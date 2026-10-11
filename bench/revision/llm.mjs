// OpenAI-compatible chat client. The harness owns the agent loop, so this only
// sends one request and reports usage. Keys come from the environment or a
// private file outside the repo; they are never logged or written to run output.
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const BASE = process.env.ZENITH_BENCH_BASE_URL ?? "https://ollama.com/v1";

function readKey() {
  if (process.env.OLLAMA_API_KEY) return process.env.OLLAMA_API_KEY.trim();
  const file = path.join(os.homedir(), ".config", "zenith-bench", "ollama.key");
  try {
    return fs.readFileSync(file, "utf8").trim();
  } catch {
    throw new Error(`no API key: set OLLAMA_API_KEY or write it to ${file}`);
  }
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Returns { message, usage, finish }. Retries 429 and 5xx with backoff.
export async function chat({ model, messages, tools, maxTokens = 8192, temperature = 0.6, seed }) {
  const key = readKey();
  const body = { model, messages, max_tokens: maxTokens, temperature };
  if (seed !== undefined) body.seed = seed;
  if (tools?.length) body.tools = tools;
  let lastErr;
  for (let attempt = 0; attempt < 6; attempt++) {
    let res;
    try {
      res = await fetch(`${BASE}/chat/completions`, {
        method: "POST",
        headers: { "content-type": "application/json", authorization: `Bearer ${key}` },
        body: JSON.stringify(body),
      });
    } catch (err) {
      lastErr = err;
      await sleep(2000 * (attempt + 1));
      continue;
    }
    if (res.status === 429 || res.status >= 500) {
      lastErr = new Error(`HTTP ${res.status}`);
      await sleep(3000 * (attempt + 1));
      continue;
    }
    const text = await res.text();
    if (!res.ok) throw new Error(`HTTP ${res.status}: ${text.slice(0, 400)}`);
    const json = JSON.parse(text);
    const choice = json.choices?.[0];
    if (!choice) throw new Error(`no choices in response: ${text.slice(0, 400)}`);
    const u = json.usage ?? {};
    return {
      message: choice.message,
      finish: choice.finish_reason,
      usage: {
        input: u.prompt_tokens ?? 0,
        output: u.completion_tokens ?? 0,
        cached: u.prompt_tokens_details?.cached_tokens ?? 0,
      },
    };
  }
  throw new Error(`model request failed after retries: ${lastErr?.message}`);
}
