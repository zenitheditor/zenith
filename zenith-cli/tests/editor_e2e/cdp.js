// A zero-dependency Chrome DevTools Protocol client: launch headless
// Chromium, open a page, and drive it. Uses Node's built-in WebSocket.

import { spawn } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export class Browser {
  /**
   * Launch `chromium` headless and connect to it. `ZENITH_E2E_NO_SANDBOX=1`
   * adds `--no-sandbox` for hosts that block user namespaces (CI containers).
   */
  static async launch(chromium) {
    const profile = mkdtempSync(path.join(tmpdir(), "zenith-e2e-chromium-"));
    const child = spawn(
      chromium,
      [
        "--headless=new",
        "--remote-debugging-port=0",
        `--user-data-dir=${profile}`,
        "--no-first-run",
        "--no-default-browser-check",
        "--disable-gpu",
        "--disable-extensions",
        "--disable-background-networking",
        "--disable-sync",
        "--hide-scrollbars",
        "--mute-audio",
        ...(process.env.ZENITH_E2E_NO_SANDBOX ? ["--no-sandbox"] : []),
        "about:blank",
      ],
      { stdio: ["ignore", "ignore", "pipe"] },
    );
    const url = await new Promise((resolve, reject) => {
      let err = "";
      const timer = setTimeout(() => reject(new Error(`chromium did not start: ${err}`)), 30000);
      child.stderr.on("data", (chunk) => {
        err += chunk;
        const m = /DevTools listening on (ws:\/\/\S+)/.exec(err);
        if (m) {
          clearTimeout(timer);
          resolve(m[1]);
        }
      });
      child.on("exit", (code) => reject(new Error(`chromium exited ${code}: ${err}`)));
    });
    const browser = new Browser(child, profile);
    await browser.connect(url);
    return browser;
  }

  constructor(child, profile) {
    this.child = child;
    this.profile = profile;
    this.id = 0;
    this.pending = new Map();
    this.listeners = new Map();
  }

  connect(url) {
    return new Promise((resolve, reject) => {
      this.ws = new WebSocket(url);
      this.ws.onopen = () => resolve();
      this.ws.onerror = (e) => reject(new Error(`websocket: ${e.message ?? e.type}`));
      this.ws.onmessage = (e) => this.onMessage(JSON.parse(e.data));
    });
  }

  onMessage(msg) {
    if (msg.id !== undefined) {
      const p = this.pending.get(msg.id);
      if (!p) return;
      this.pending.delete(msg.id);
      if (msg.error) p.reject(new Error(`${p.method}: ${msg.error.message}`));
      else p.resolve(msg.result);
      return;
    }
    const key = `${msg.sessionId ?? ""}:${msg.method}`;
    for (const fn of this.listeners.get(key) ?? []) fn(msg.params);
  }

  send(method, params = {}, sessionId) {
    const id = ++this.id;
    const msg = { id, method, params };
    if (sessionId) msg.sessionId = sessionId;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject, method });
      this.ws.send(JSON.stringify(msg));
    });
  }

  on(sessionId, method, fn) {
    const key = `${sessionId ?? ""}:${method}`;
    if (!this.listeners.has(key)) this.listeners.set(key, []);
    this.listeners.get(key).push(fn);
  }

  async newPage() {
    const { targetId } = await this.send("Target.createTarget", { url: "about:blank" });
    const { sessionId } = await this.send("Target.attachToTarget", { targetId, flatten: true });
    const page = new Page(this, sessionId);
    await page.init();
    return page;
  }

  async close() {
    try {
      await this.send("Browser.close");
    } catch {
      // The socket closes as the browser exits.
    }
    this.child.kill();
    await sleep(200);
    rmSync(this.profile, { recursive: true, force: true });
  }
}

export class Page {
  constructor(browser, sessionId) {
    this.browser = browser;
    this.sessionId = sessionId;
    this.errors = [];
    this.dialogs = [];
  }

  send(method, params) {
    return this.browser.send(method, params, this.sessionId);
  }

  async init() {
    this.browser.on(this.sessionId, "Runtime.exceptionThrown", (p) => {
      this.errors.push(p.exceptionDetails?.exception?.description ?? p.exceptionDetails?.text);
    });
    this.browser.on(this.sessionId, "Runtime.consoleAPICalled", (p) => {
      if (p.type === "error") {
        this.errors.push(p.args.map((a) => a.value ?? a.description).join(" "));
      }
    });
    this.browser.on(this.sessionId, "Log.entryAdded", (p) => {
      if (p.entry.level === "error" && !/favicon/.test(p.entry.url ?? "")) {
        this.errors.push(`${p.entry.source}: ${p.entry.text} ${p.entry.url ?? ""}`);
      }
    });
    this.browser.on(this.sessionId, "Page.javascriptDialogOpening", (p) => {
      this.dialogs.push(p);
      this.send("Page.handleJavaScriptDialog", { accept: true });
    });
    await this.send("Page.enable");
    await this.send("Runtime.enable");
    await this.send("Log.enable");
  }

  async viewport(width, height, mobile = false, deviceScaleFactor = 1) {
    await this.send("Emulation.setDeviceMetricsOverride", {
      width,
      height,
      deviceScaleFactor,
      mobile,
    });
  }

  async media(scheme) {
    await this.send("Emulation.setEmulatedMedia", {
      features: [{ name: "prefers-color-scheme", value: scheme }],
    });
  }

  async goto(url) {
    const loaded = new Promise((resolve) =>
      this.browser.on(this.sessionId, "Page.loadEventFired", resolve),
    );
    await this.send("Page.navigate", { url });
    await loaded;
  }

  async reload() {
    const loaded = new Promise((resolve) =>
      this.browser.on(this.sessionId, "Page.loadEventFired", resolve),
    );
    await this.send("Page.reload", { ignoreCache: false });
    await loaded;
  }

  /** Evaluate `expr` (an expression, awaited) and return its value. */
  async eval(expr) {
    const r = await this.send("Runtime.evaluate", {
      expression: expr,
      awaitPromise: true,
      returnByValue: true,
    });
    if (r.exceptionDetails) {
      throw new Error(`eval failed: ${r.exceptionDetails.exception?.description ?? r.exceptionDetails.text}\n${expr}`);
    }
    return r.result.value;
  }

  /** Wait until `expr` is truthy; returns its value. */
  async waitFor(expr, what = expr, timeout = 15000) {
    const end = Date.now() + timeout;
    let last;
    while (Date.now() < end) {
      try {
        last = await this.eval(expr);
        if (last) return last;
      } catch (err) {
        last = err.message;
      }
      await sleep(50);
    }
    throw new Error(`timed out waiting for ${what} (last: ${JSON.stringify(last)})`);
  }

  /** The center of the element matched by `selector`, in CSS px. */
  async center(selector) {
    const r = await this.eval(
      `(() => { const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect(); return {x: r.left + r.width / 2, y: r.top + r.height / 2, w: r.width, h: r.height, left: r.left, top: r.top}; })()`,
    );
    return r;
  }

  async mouse(type, x, y, extra = {}) {
    await this.send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1, ...extra });
  }

  async click(x, y, extra = {}) {
    await this.mouse("mouseMoved", x, y, { button: "none", ...extra });
    await this.mouse("mousePressed", x, y, extra);
    await this.mouse("mouseReleased", x, y, extra);
  }

  async clickSelector(selector) {
    const c = await this.center(selector);
    await this.click(c.x, c.y);
  }

  async drag(from, to, steps = 8, extra = {}) {
    await this.mouse("mouseMoved", from.x, from.y, { button: "none" });
    await this.mouse("mousePressed", from.x, from.y, extra);
    for (let i = 1; i <= steps; i++) {
      const x = from.x + ((to.x - from.x) * i) / steps;
      const y = from.y + ((to.y - from.y) * i) / steps;
      await this.mouse("mouseMoved", x, y, { buttons: extra.button === "middle" ? 4 : 1, ...extra });
    }
    await this.mouse("mouseReleased", to.x, to.y, extra);
  }

  async wheel(x, y, deltaX, deltaY, modifiers = 0) {
    await this.send("Input.dispatchMouseEvent", { type: "mouseWheel", x, y, deltaX, deltaY, modifiers });
  }

  /** Press `key` (DOM key name) with `modifiers` (1 Alt, 2 Ctrl, 4 Meta, 8 Shift). */
  async key(key, modifiers = 0, code = undefined) {
    const named = {
      Escape: [27, "Escape"],
      Enter: [13, "Enter"],
      ArrowLeft: [37, "ArrowLeft"],
      ArrowRight: [39, "ArrowRight"],
      ArrowUp: [38, "ArrowUp"],
      ArrowDown: [40, "ArrowDown"],
      Home: [36, "Home"],
      End: [35, "End"],
      Backspace: [8, "Backspace"],
      Delete: [46, "Delete"],
      Tab: [9, "Tab"],
    };
    const [vk, keyCode] = named[key] ?? [key.toUpperCase().charCodeAt(0), code ?? `Key${key.toUpperCase()}`];
    // Enter types "\r" as a real keyboard does (it commits a text field).
    const text = key === "Enter" && !(modifiers & 6) ? "\r" : named[key] || modifiers & 6 ? undefined : key;
    const base = { key, code: keyCode, windowsVirtualKeyCode: vk, modifiers };
    await this.send("Input.dispatchKeyEvent", { type: text ? "keyDown" : "rawKeyDown", text, ...base });
    await this.send("Input.dispatchKeyEvent", { type: "keyUp", ...base });
  }

  async type(text) {
    await this.send("Input.insertText", { text });
  }

  async screenshot(file) {
    const { data } = await this.send("Page.captureScreenshot", { format: "png" });
    writeFileSync(file, Buffer.from(data, "base64"));
  }
}
