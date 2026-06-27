const fs = require("node:fs");
const https = require("node:https");

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const request = https.get(
      url,
      {
        headers: {
          "User-Agent": "@zenitheditor/zenith-mcp npm installer",
        },
      },
      (response) => {
        if (
          response.statusCode >= 300 &&
          response.statusCode < 400 &&
          response.headers.location
        ) {
          response.resume();
          download(response.headers.location, dest).then(resolve, reject);
          return;
        }

        if (response.statusCode !== 200) {
          response.resume();
          reject(new Error(`download failed (${response.statusCode}): ${url}`));
          return;
        }

        const file = fs.createWriteStream(dest, { mode: 0o755 });
        response.pipe(file);
        file.on("finish", () => file.close(resolve));
        file.on("error", reject);
      }
    );

    request.on("error", reject);
  });
}

module.exports = { download };
