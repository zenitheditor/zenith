import { check, ready } from "./helpers.js";

const DIR = "samples/confined/";
const SOURCE = "zenith version=1 {}\n";

function response(local, manifest, outside) {
  if (local === "confinement-outside.zen") return { body: "outside" };
  if (local === `${DIR}redirect.zen` || local === `${DIR}asset-redirect.txt`) {
    return { code: 302, headers: [{ name: "Location", value: outside }] };
  }
  if (local === `${DIR}manifest.json`) return {
    body: JSON.stringify({ files: manifest }),
    headers: [{ name: "Content-Type", value: "application/json" }],
  };
  if (local === `${DIR}nested/logo%20%C3%A9.txt`) return { body: "asset" };
  if (local === `${DIR}Custom%20Design%20%C3%A9.zen`) return { body: SOURCE };
  return { code: 404 };
}

const load = (value) => `(async () => {
  const { loadSample } = await import('./js/app/samples.js');
  try {
    const { name, text, project } = await loadSample(${JSON.stringify(value)});
    return { name, text, files: project?.files };
  } catch (error) {
    return { error: error.message };
  }
})()`;

export const steps = [
  [
    "hosted sample links and manifest assets stay inside the bundle",
    async ({ page, url }) => {
      const home = new URL(url);
      home.search = "";
      await page.goto(home.href);
      await page.waitFor("!document.getElementById('welcome').hidden", "the welcome screen");
      const child = `${DIR}Custom Design é.zen`;
      const outside = new URL("confinement-outside.zen", home).pathname;
      const requests = [];
      let manifest = ["nested/logo é.txt"];
      const errors = [];
      page.browser.on(page.sessionId, "Fetch.requestPaused", ({ requestId, request }) => {
        const local = new URL(request.url).pathname.slice(home.pathname.length);
        requests.push(local);
        const { code = 200, headers = [], body = "" } = response(local, manifest, outside);
        page.send("Fetch.fulfillRequest", {
          requestId, responseCode: code, responseHeaders: headers, body: Buffer.from(body).toString("base64"),
        }).catch((error) => errors.push(error.message));
      });
      await page.send("Fetch.enable", {
        patterns: [
          { urlPattern: `*${DIR}*` },
          { urlPattern: "*confinement-outside.zen" },
        ],
      });
      page.expected.push(/(?:net::ERR_FAILED|redirect mode).*(?:redirect\.zen|asset-redirect\.txt|confinement-outside\.zen)/i);
      try {
        const opened = await page.eval(load(child));
        check(opened.name === "Custom Design é.zen" && opened.text === SOURCE
          && opened.files?.["nested/logo é.txt"] === "YXNzZXQ=", "a nested fixture lost its name or asset");

        requests.length = 0;
        for (const path of [
          "", "assets/confinement-outside.zen", "/samples/confinement-outside.zen",
          "samples/../confinement-outside.zen", "samples/./confinement-outside.zen",
          "samples/%2e%2e/confinement-outside.zen", "samples\\..\\confinement-outside.zen",
          "//host.invalid/confinement-outside.zen", "https://host.invalid/confinement-outside.zen",
          "samples/confined/readme.txt",
        ]) {
          const result = await page.eval(load(path));
          check(result.error?.includes("Choose a .zen"), `unsafe document accepted: ${path}`);
        }
        check(requests.length === 0, `unsafe document requests: ${requests}`);

        for (const asset of [
          "../../confinement-outside.zen", "/confinement-outside.zen", "../other.txt",
          "%2e%2e/confinement-outside.zen", "nested\\..\\other.txt", "https://host.invalid/outside.txt",
        ]) {
          requests.length = 0;
          manifest = ["nested/logo é.txt", asset];
          const result = await page.eval(load(child));
          check(result.error?.includes("invalid asset path"), `unsafe asset accepted: ${asset}`);
          check(requests.length === 2 && requests[1] === `${DIR}manifest.json`, `unsafe manifest started asset requests: ${requests}`);
        }

        manifest = ["asset-redirect.txt"];
        for (const path of [`${DIR}redirect.zen`, child]) {
          requests.length = 0;
          check((await page.eval(load(path))).error, `redirect accepted for ${path}`);
          check(!requests.includes("confinement-outside.zen"), `redirect fetched outside bundle: ${requests}`);
        }
        check(errors.length === 0, `fixture interception errors: ${errors}`);
      } finally {
        await page.send("Fetch.disable");
      }
      await page.goto(url);
      await ready(page);
      return { nested: child };
    },
  ],
];
