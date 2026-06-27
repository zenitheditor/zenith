#!/usr/bin/env node

const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const exe = process.platform === "win32" ? "zenith.exe" : "zenith";
const binPath = path.join(root, "vendor", exe);

if (!fs.existsSync(binPath)) {
  const { install } = require("../scripts/install");
  install({ root });
}

const args = process.argv.length > 2 ? process.argv.slice(2) : ["mcp"];
const result = spawnSync(binPath, args, { stdio: "inherit" });

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status === null ? 1 : result.status);
