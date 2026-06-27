const { download } = require("./util");

const [url, dest] = process.argv.slice(2);

download(url, dest).catch((error) => {
  console.error(error.message);
  process.exit(1);
});
