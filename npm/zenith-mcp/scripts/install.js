const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const REPO = "zenitheditor/zenith";

function platformTarget() {
  const arch = os.arch();

  if (process.platform === "linux") {
    if (arch === "x64") return { label: "linux-x64", archive: "tar.gz", exe: "zenith" };
    if (arch === "arm64") return { label: "linux-arm64", archive: "tar.gz", exe: "zenith" };
  }

  if (process.platform === "darwin") {
    if (arch === "x64") return { label: "macos-x64", archive: "tar.gz", exe: "zenith" };
    if (arch === "arm64") return { label: "macos-arm64", archive: "tar.gz", exe: "zenith" };
  }

  if (process.platform === "win32" && arch === "x64") {
    return { label: "windows-x64", archive: "zip", exe: "zenith.exe" };
  }

  throw new Error(`unsupported platform: ${process.platform}-${arch}`);
}

function packageVersion(root) {
  const pkg = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8"));
  return pkg.version;
}

function extract(archivePath, archiveType, tmpDir) {
  if (archiveType === "zip") {
    if (process.platform === "win32") {
      execFileSync(
        "powershell.exe",
        [
          "-NoProfile",
          "-ExecutionPolicy",
          "Bypass",
          "-Command",
          `Expand-Archive -LiteralPath ${JSON.stringify(archivePath)} -DestinationPath ${JSON.stringify(tmpDir)} -Force`,
        ],
        { stdio: "inherit" }
      );
      return;
    }

    execFileSync("unzip", ["-oq", archivePath, "-d", tmpDir], { stdio: "inherit" });
    return;
  }

  execFileSync("tar", ["xzf", archivePath, "-C", tmpDir], { stdio: "inherit" });
}

function install(options = {}) {
  if (process.env.ZENITH_NPM_SKIP_DOWNLOAD === "1") {
    return;
  }

  const root = options.root || path.resolve(__dirname, "..");
  const vendorDir = path.join(root, "vendor");
  const target = platformTarget();
  const version = packageVersion(root);
  const archiveName = `zenith-${version}-${target.label}.${target.archive}`;
  const url = `https://github.com/${REPO}/releases/download/v${version}/${archiveName}`;
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "zenith-npm-"));
  const archivePath = path.join(tmpDir, archiveName);
  const outPath = path.join(vendorDir, target.exe);

  fs.mkdirSync(vendorDir, { recursive: true });

  try {
    console.log(`Downloading Zenith ${version} (${target.label})...`);
    execFileSync(process.execPath, [path.join(__dirname, "postinstall-download.js"), url, archivePath], {
      stdio: "inherit",
    });
    extract(archivePath, target.archive, tmpDir);

    const extracted = path.join(tmpDir, target.exe);
    if (!fs.existsSync(extracted)) {
      throw new Error(`archive did not contain ${target.exe}`);
    }

    fs.copyFileSync(extracted, outPath);
    if (process.platform !== "win32") {
      fs.chmodSync(outPath, 0o755);
    }
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

module.exports = { install };
