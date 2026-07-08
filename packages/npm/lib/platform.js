"use strict";

/**
 * Platform resolution + vendored-binary paths for the jarvy npm wrapper.
 *
 * The triple table mirrors what .github/workflows/release.yml actually
 * publishes (and what dist/scripts/install.sh requests):
 *   - macOS ships Apple Silicon only (Intel users: `cargo install jarvy`
 *     or `brew install Cliftonz/tap/jarvy`, both compile from source).
 *   - Linux x86_64 is the static musl build (runs on glibc AND musl).
 *   - Linux aarch64 is gnu; armv7 is gnueabihf.
 *   - Windows is x86_64 MSVC, shipped as a .zip.
 */

const path = require("node:path");

const REPO = "Cliftonz/jarvy";

/**
 * Map (process.platform, process.arch) to the published release triple.
 * Returns `{ triple, ext }` or throws with an actionable message.
 */
function resolveTriple(platform = process.platform, arch = process.arch) {
  if (platform === "darwin") {
    if (arch === "arm64") {
      return { triple: "aarch64-apple-darwin", ext: "tar.gz" };
    }
    throw new Error(
      "jarvy does not ship a prebuilt Intel macOS binary. " +
        "Install from source instead:\n" +
        "    cargo install jarvy\n" +
        "or\n" +
        "    brew install Cliftonz/tap/jarvy"
    );
  }
  if (platform === "linux") {
    if (arch === "x64") {
      return { triple: "x86_64-unknown-linux-musl", ext: "tar.gz" };
    }
    if (arch === "arm64") {
      return { triple: "aarch64-unknown-linux-gnu", ext: "tar.gz" };
    }
    if (arch === "arm") {
      return { triple: "armv7-unknown-linux-gnueabihf", ext: "tar.gz" };
    }
    throw new Error(`jarvy has no prebuilt Linux binary for arch '${arch}'.`);
  }
  if (platform === "win32") {
    if (arch === "x64") {
      return { triple: "x86_64-pc-windows-msvc", ext: "zip" };
    }
    throw new Error(`jarvy has no prebuilt Windows binary for arch '${arch}'.`);
  }
  throw new Error(`jarvy has no prebuilt binary for platform '${platform}'.`);
}

/** Directory the postinstall script vendors the binary into. */
function vendorDir() {
  return path.join(__dirname, "..", "vendor");
}

/** Absolute path of the vendored jarvy binary for this platform. */
function binaryPath(platform = process.platform) {
  const name = platform === "win32" ? "jarvy.exe" : "jarvy";
  return path.join(vendorDir(), name);
}

/** Release asset name for a version + triple, matching release.yml. */
function archiveName(version, { triple, ext }) {
  return `jarvy-v${version}-${triple}.${ext}`;
}

/** Download URL for a release asset. */
function assetUrl(version, name) {
  return `https://github.com/${REPO}/releases/download/v${version}/${name}`;
}

/** URL of the release's SHA256SUMS.txt. */
function checksumsUrl(version) {
  return `https://github.com/${REPO}/releases/download/v${version}/SHA256SUMS.txt`;
}

module.exports = {
  REPO,
  resolveTriple,
  vendorDir,
  binaryPath,
  archiveName,
  assetUrl,
  checksumsUrl,
};
