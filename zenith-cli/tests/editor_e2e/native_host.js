// The native engine behind `zenith edit`, for the e2e runs: start the
// server on a document and talk to its HTTP API from Node.

import { spawn } from "node:child_process";

export async function startServer(zenith, doc, data) {
  const child = spawn(zenith, ["edit", doc, "--no-open", "--json"], {
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, ZENITH_DATA_DIR: data },
  });
  let stderr = "";
  child.stderr.on("data", (d) => (stderr += d));
  const line = await new Promise((resolve, reject) => {
    let buf = "";
    const timer = setTimeout(() => reject(new Error(`zenith edit did not start: ${stderr}`)), 30000);
    child.stdout.on("data", (d) => {
      buf += d;
      const nl = buf.indexOf("\n");
      if (nl >= 0) {
        clearTimeout(timer);
        resolve(buf.slice(0, nl));
      }
    });
    child.on("exit", (code) => reject(new Error(`zenith edit exited ${code}: ${stderr}`)));
  });
  const start = JSON.parse(line);
  return { child, url: start.url, stderr: () => stderr };
}

/**
 * A client of a started server: takes the token from the URL fragment and
 * sends it as a bearer header. `run(command, params, version)` resolves the
 * envelope.
 */
export async function connect(server) {
  const url = new URL(server.url);
  const token = new URLSearchParams(url.hash.slice(1)).get("token");
  const auth = { Authorization: `Bearer ${token}` };
  const base = url.origin;
  const call = async (path, body) => {
    const res = await fetch(`${base}${path}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Zenith-Client": "node-e2e", ...auth },
      body: JSON.stringify(body),
    });
    return res.json();
  };
  return {
    run: (command, params = {}, version) => call("/api/cmd", { command, params, ...(version === undefined ? {} : { version }) }),
    state: async () => (await fetch(`${base}/api/state?text=1`, { headers: auth })).json(),
  };
}
