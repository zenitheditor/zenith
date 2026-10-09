// HttpEngine: the Engine interface (see `index.js`) over the `zenith edit`
// HTTP API. The page cookie set by `/?token=` authenticates every request.

import { EngineError, EngineUnavailable } from "./errors.js";

const RETRY_MIN_MS = 500;
const RETRY_MAX_MS = 8000;
const EVENTS = ["state", "session", "external_change", "saved", "shutdown"];

/** A random client id: 16 characters of `[A-Za-z0-9]`. */
function newClientId() {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => alphabet[b % alphabet.length]).join("");
}

export class HttpEngine {
  constructor(base = "") {
    this.base = base;
    this.kind = "http";
    this.clientId = `page-${newClientId()}`;
  }

  async run(command, params = {}, { version } = {}) {
    const body = { command, params };
    if (version !== undefined && version !== null) body.version = version;
    return this.post("/api/cmd", body, command);
  }

  async save({ version, overwrite = false } = {}) {
    const body = { overwrite };
    if (version !== undefined && version !== null) body.version = version;
    return this.post("/api/save", body, "file.save");
  }

  async state({ text = false } = {}) {
    const res = await this.fetch(`/api/state${text ? "?text=1" : ""}`, {});
    const json = await readJson(res);
    if (!res.ok || !json.ok) {
      const e = json.error || {};
      throw new EngineError(e.code || `http.${res.status}`, e.message || res.statusText);
    }
    return json;
  }

  async image(image) {
    const res = await this.fetch(image.url, {});
    if (!res.ok) {
      const json = await readJson(res);
      const e = json.error || {};
      throw new EngineError(e.code || `http.${res.status}`, e.message || res.statusText);
    }
    return res.blob();
  }

  subscribe({ open, event, lost, stopped }) {
    let source = null;
    let timer = null;
    let delay = RETRY_MIN_MS;
    let ended = false;

    const connect = () => {
      timer = null;
      if (ended) return;
      source = new EventSource(`${this.base}/api/events`);
      source.onopen = () => {
        delay = RETRY_MIN_MS;
        open?.();
      };
      for (const name of EVENTS) {
        source.addEventListener(name, (e) => {
          let data = {};
          try {
            data = JSON.parse(e.data);
          } catch {
            data = {};
          }
          event?.(name, data);
          if (name === "shutdown") {
            ended = true;
            source.close();
            stopped?.();
          }
        });
      }
      source.onerror = () => {
        // The browser retries on a fixed delay. Close it and retry with a
        // growing delay instead.
        source.close();
        if (ended) return;
        lost?.(delay);
        timer = setTimeout(connect, delay);
        delay = Math.min(delay * 2, RETRY_MAX_MS);
      };
    };
    connect();
    return () => {
      ended = true;
      if (timer !== null) clearTimeout(timer);
      source?.close();
    };
  }

  async post(path, body, command) {
    const res = await this.fetch(path, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Zenith-Client": this.clientId,
      },
      body: JSON.stringify(body),
    });
    const json = await readJson(res);
    if (res.ok && typeof json.ok === "boolean") return json;
    // HTTP-level refusal (auth, limits, busy): same shape as an engine error.
    const e = json.error || {};
    return {
      ok: false,
      command,
      error: {
        code: e.code || `http.${res.status}`,
        message: e.message || `the server answered ${res.status} ${res.statusText}`,
      },
    };
  }

  async fetch(path, init) {
    try {
      return await fetch(`${this.base}${path}`, { credentials: "same-origin", ...init });
    } catch (err) {
      throw new EngineUnavailable(`Cannot reach zenith edit (${err.message})`);
    }
  }
}

async function readJson(res) {
  try {
    return await res.json();
  } catch {
    return {};
  }
}
