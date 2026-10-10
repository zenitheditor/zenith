// HttpEngine: the Engine interface (see `index.js`) over the `zenith edit`
// HTTP API. Every request carries `Authorization: Bearer <token>`. The
// token comes from the URL fragment (`main.js` reads it), so no cookie
// exists for other local services to receive. `EventSource` cannot send a
// header, so the event stream is read with `fetch`.

import { EngineError, EngineUnavailable } from "./errors.js";

const RETRY_MIN_MS = 500;
const RETRY_MAX_MS = 8000;
const EVENTS = new Set(["state", "session", "external_change", "saved", "stopping", "shutdown"]);

/** A random client id: 16 characters of `[A-Za-z0-9]`. */
function newClientId() {
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => alphabet[b % alphabet.length]).join("");
}

export class HttpEngine {
  constructor(token, base = "") {
    this.base = base;
    this.kind = "http";
    this.clientId = `page-${newClientId()}`;
    this.token = token;
  }

  /** The header that proves this page holds the token. */
  authHeaders() {
    return { Authorization: `Bearer ${this.token}` };
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
    let timer = null;
    let delay = RETRY_MIN_MS;
    let ended = false;
    let abort = null;

    const retry = () => {
      if (ended) return;
      lost?.(delay);
      timer = setTimeout(connect, delay);
      delay = Math.min(delay * 2, RETRY_MAX_MS);
    };

    const dispatch = (name, raw) => {
      if (!EVENTS.has(name)) return;
      let data = {};
      try {
        data = JSON.parse(raw);
      } catch {
        data = {};
      }
      event?.(name, data);
      if (name === "shutdown") {
        ended = true;
        abort?.abort();
        stopped?.();
      }
    };

    const connect = async () => {
      timer = null;
      if (ended) return;
      abort = new AbortController();
      let res;
      try {
        res = await fetch(`${this.base}/api/events`, {
          headers: this.authHeaders(),
          cache: "no-store",
          signal: abort.signal,
        });
      } catch {
        retry();
        return;
      }
      if (!res.ok || !res.body) {
        retry();
        return;
      }
      delay = RETRY_MIN_MS;
      open?.();
      try {
        await readEvents(res.body, dispatch);
      } catch {
        // A dropped stream ends the read. The retry below covers it.
      }
      retry();
    };
    connect();
    return () => {
      ended = true;
      if (timer !== null) clearTimeout(timer);
      abort?.abort();
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
    const headers = { ...(init.headers ?? {}), ...this.authHeaders() };
    try {
      return await fetch(`${this.base}${path}`, { ...init, headers, credentials: "omit" });
    } catch (err) {
      throw new EngineUnavailable(`Cannot reach zenith edit (${err.message})`);
    }
  }
}

/**
 * Read a `text/event-stream` body and call `dispatch(name, data)` for each
 * record. Comment lines (`: ping`) and ids are skipped. Resolves when the
 * stream ends.
 */
export async function readEvents(body, dispatch) {
  const reader = body.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = "";
  for (;;) {
    const { value, done } = await reader.read();
    if (done) return;
    buffer += value;
    let end;
    while ((end = buffer.indexOf("\n\n")) >= 0) {
      const record = buffer.slice(0, end);
      buffer = buffer.slice(end + 2);
      let name = "message";
      const data = [];
      for (const line of record.split("\n")) {
        if (line.startsWith(":")) continue;
        const colon = line.indexOf(":");
        const field = colon < 0 ? line : line.slice(0, colon);
        const text = colon < 0 ? "" : line.slice(colon + 1).replace(/^ /, "");
        if (field === "event") name = text;
        else if (field === "data") data.push(text);
      }
      if (data.length) dispatch(name, data.join("\n"));
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
