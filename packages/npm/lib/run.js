"use strict";

/**
 * Shared launcher for the bin shims. Prefers the vendored binary that
 * postinstall downloaded; falls back to a system-wide `jarvy` on PATH
 * (covers JARVY_NPM_SKIP_DOWNLOAD=1 installs).
 */

const fs = require("node:fs");
const { spawn } = require("node:child_process");

const { binaryPath } = require("./platform");

function resolveBinary() {
  const vendored = binaryPath();
  if (fs.existsSync(vendored)) {
    return vendored;
  }
  // Fall back to PATH lookup; spawn resolves it.
  return process.platform === "win32" ? "jarvy.exe" : "jarvy";
}

/** Spawn jarvy with the given args, inheriting stdio (MCP runs on stdio). */
function run(args) {
  const bin = resolveBinary();
  const child = spawn(bin, args, { stdio: "inherit" });
  child.on("error", (err) => {
    if (err.code === "ENOENT") {
      console.error(
        "[jarvy-mcp] ERROR: jarvy binary not found. The postinstall download " +
          "may have been skipped or failed.\n" +
          "Re-run it with:  npm rebuild jarvy-mcp\n" +
          "Or install jarvy another way: https://github.com/Cliftonz/jarvy#installation"
      );
      process.exit(127);
    }
    console.error(`[jarvy-mcp] ERROR: failed to launch jarvy: ${err.message}`);
    process.exit(1);
  });
  child.on("exit", (code, signal) => {
    if (signal) {
      process.kill(process.pid, signal);
      return;
    }
    process.exit(code === null ? 1 : code);
  });
}

module.exports = { run, resolveBinary };
