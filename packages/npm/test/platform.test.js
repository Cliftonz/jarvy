"use strict";

const assert = require("node:assert/strict");
const { test } = require("node:test");

const {
  resolveTriple,
  archiveName,
  assetUrl,
  checksumsUrl,
} = require("../lib/platform");
const { expectedShaFor, sha256 } = require("../scripts/postinstall");

test("darwin/arm64 resolves to aarch64-apple-darwin tar.gz", () => {
  assert.deepEqual(resolveTriple("darwin", "arm64"), {
    triple: "aarch64-apple-darwin",
    ext: "tar.gz",
  });
});

test("darwin/x64 (Intel mac) throws with cargo/brew guidance", () => {
  assert.throws(() => resolveTriple("darwin", "x64"), /cargo install jarvy/);
});

test("linux x64 resolves to the static musl triple", () => {
  assert.equal(resolveTriple("linux", "x64").triple, "x86_64-unknown-linux-musl");
});

test("linux arm64 resolves to gnu; arm to gnueabihf", () => {
  assert.equal(resolveTriple("linux", "arm64").triple, "aarch64-unknown-linux-gnu");
  assert.equal(resolveTriple("linux", "arm").triple, "armv7-unknown-linux-gnueabihf");
});

test("win32/x64 resolves to msvc zip", () => {
  assert.deepEqual(resolveTriple("win32", "x64"), {
    triple: "x86_64-pc-windows-msvc",
    ext: "zip",
  });
});

test("unsupported platforms throw", () => {
  assert.throws(() => resolveTriple("freebsd", "x64"));
  assert.throws(() => resolveTriple("linux", "ppc64"));
  assert.throws(() => resolveTriple("win32", "arm64"));
});

test("archive name + URLs match the release.yml contract", () => {
  const plat = resolveTriple("linux", "x64");
  const name = archiveName("0.5.2", plat);
  assert.equal(name, "jarvy-v0.5.2-x86_64-unknown-linux-musl.tar.gz");
  assert.equal(
    assetUrl("0.5.2", name),
    "https://github.com/Cliftonz/jarvy/releases/download/v0.5.2/jarvy-v0.5.2-x86_64-unknown-linux-musl.tar.gz"
  );
  assert.equal(
    checksumsUrl("0.5.2"),
    "https://github.com/Cliftonz/jarvy/releases/download/v0.5.2/SHA256SUMS.txt"
  );
});

test("expectedShaFor matches basenames with ./ and path prefixes", () => {
  const sums = [
    "aaaa  ./jarvy-v0.5.2-aarch64-apple-darwin.tar.gz",
    "BBBB  ./some/dir/jarvy-v0.5.2-x86_64-unknown-linux-musl.tar.gz",
    "cccc  jarvy-v0.5.2-x86_64-pc-windows-msvc.zip",
    "",
    "not-a-sums-line",
  ].join("\n");

  assert.equal(expectedShaFor(sums, "jarvy-v0.5.2-aarch64-apple-darwin.tar.gz"), "aaaa");
  // lowercased on the way out
  assert.equal(
    expectedShaFor(sums, "jarvy-v0.5.2-x86_64-unknown-linux-musl.tar.gz"),
    "bbbb"
  );
  assert.equal(expectedShaFor(sums, "jarvy-v0.5.2-x86_64-pc-windows-msvc.zip"), "cccc");
  assert.equal(expectedShaFor(sums, "missing.tar.gz"), null);
});

test("sha256 produces the expected digest", () => {
  // sha256 of the empty string is a well-known constant.
  assert.equal(
    sha256(Buffer.from("")),
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  );
});
