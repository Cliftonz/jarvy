"use strict";

/**
 * Postinstall: download the platform-native jarvy release binary.
 *
 * Mirrors dist/scripts/install.sh:
 *   1. Resolve the release triple for this platform.
 *   2. Download jarvy-v<version>-<triple>.{tar.gz,zip} from GitHub Releases.
 *   3. Verify the archive against the release SHA256SUMS.txt BEFORE
 *      extracting. A mismatch aborts the install. A missing sums file
 *      (very old releases) warns but proceeds so legacy tags stay
 *      installable. JARVY_SKIP_CHECKSUM=1 opts out.
 *   4. Extract into <package>/vendor/ and chmod +x.
 *
 * Env vars:
 *   JARVY_NPM_SKIP_DOWNLOAD=1  - skip entirely (CI / offline installs;
 *                                the bin shims will fall back to a
 *                                system-wide `jarvy` on PATH)
 *   JARVY_SKIP_CHECKSUM=1      - skip SHA256 verification (NOT recommended)
 *   JARVY_VERSION              - override the release to download
 *                                (default: this package's version)
 */

const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const {
  resolveTriple,
  vendorDir,
  binaryPath,
  archiveName,
  assetUrl,
  checksumsUrl,
} = require("../lib/platform");

const pkg = require("../package.json");

function log(msg) {
  console.log(`[jarvy-mcp] ${msg}`);
}

function warn(msg) {
  console.warn(`[jarvy-mcp] WARN: ${msg}`);
}

function fail(msg) {
  console.error(`[jarvy-mcp] ERROR: ${msg}`);
  process.exit(1);
}

async function download(url) {
  const res = await fetch(url, { redirect: "follow" });
  if (!res.ok) {
    throw new Error(`GET ${url} -> HTTP ${res.status}`);
  }
  return Buffer.from(await res.arrayBuffer());
}

/**
 * Extract the expected SHA256 for `name` from SHA256SUMS.txt content.
 * Lines are "<hex>  [./]<path/to/filename>" (the `./` prefix comes from
 * release.yml's `sha256sum ./jarvy*`). Match on exact basename, like
 * install.sh's fetch_expected_sha.
 */
function expectedShaFor(sums, name) {
  for (const line of sums.split("\n")) {
    const parts = line.trim().split(/\s+/);
    if (parts.length < 2) continue;
    let file = parts[parts.length - 1].replace(/^\.\//, "");
    file = file.split("/").pop();
    if (file === name) return parts[0].toLowerCase();
  }
  return null;
}

function sha256(buf) {
  return crypto.createHash("sha256").update(buf).digest("hex");
}

function extract(archivePath, destDir, ext) {
  // tar.gz on unix; on Windows the bundled bsdtar (Windows 10+) also
  // extracts .zip, so a single `tar` invocation covers both formats.
  const args =
    ext === "zip" ? ["-xf", archivePath, "-C", destDir] : ["-xzf", archivePath, "-C", destDir];
  const res = spawnSync("tar", args, { stdio: "inherit" });
  if (res.error || res.status !== 0) {
    throw new Error(
      `failed to extract ${archivePath}: ${res.error ? res.error.message : `tar exited ${res.status}`}`
    );
  }
}

async function main() {
  if (process.env.JARVY_NPM_SKIP_DOWNLOAD === "1") {
    log("JARVY_NPM_SKIP_DOWNLOAD=1 set - skipping binary download.");
    return;
  }

  let plat;
  try {
    plat = resolveTriple();
  } catch (err) {
    fail(err.message);
  }

  const version = (process.env.JARVY_VERSION || pkg.version).replace(/^v/, "");
  const name = archiveName(version, plat);
  const url = assetUrl(version, name);

  log(`Downloading jarvy v${version} (${plat.triple})...`);
  log(url);

  let archive;
  try {
    archive = await download(url);
  } catch (err) {
    fail(
      `download failed: ${err.message}\n` +
        "Check https://github.com/Cliftonz/jarvy/releases for available versions, " +
        "or install another way: https://github.com/Cliftonz/jarvy#installation"
    );
  }

  // Integrity check before we ever extract the downloaded bytes.
  if (process.env.JARVY_SKIP_CHECKSUM === "1") {
    warn("JARVY_SKIP_CHECKSUM=1 set - skipping integrity verification.");
  } else {
    let sums = null;
    try {
      sums = (await download(checksumsUrl(version))).toString("utf8");
    } catch {
      // fall through to the warn below
    }
    const expected = sums ? expectedShaFor(sums, name) : null;
    if (expected) {
      const actual = sha256(archive);
      if (actual !== expected) {
        fail(
          `checksum verification failed for ${name}!\n` +
            `  Expected: ${expected}\n` +
            `  Actual:   ${actual}\n` +
            "Refusing to install."
        );
      }
      log("Checksum verified.");
    } else {
      warn(`SHA256SUMS.txt entry not found for v${version} - skipping integrity check.`);
      warn("Set JARVY_SKIP_CHECKSUM=1 to silence, or verify the download manually.");
    }
  }

  // Extract to a temp dir, then move the binary into vendor/.
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "jarvy-npm-"));
  try {
    const archivePath = path.join(tmpDir, name);
    fs.writeFileSync(archivePath, archive);
    extract(archivePath, tmpDir, plat.ext);

    const binName = process.platform === "win32" ? "jarvy.exe" : "jarvy";
    const extracted = path.join(tmpDir, binName);
    if (!fs.existsSync(extracted)) {
      throw new Error(`archive did not contain expected binary '${binName}'`);
    }

    fs.mkdirSync(vendorDir(), { recursive: true });
    const dest = binaryPath();
    fs.copyFileSync(extracted, dest);
    if (process.platform !== "win32") {
      fs.chmodSync(dest, 0o755);
    }
    log(`Installed ${dest}`);
    log("Run the MCP server with: npx jarvy-mcp   (or: jarvy mcp)");
  } catch (err) {
    fail(err.message);
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

if (require.main === module) {
  main().catch((err) => fail(err.message));
}

module.exports = { expectedShaFor, sha256 };
