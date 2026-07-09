// Unit tests for parseValidationResult — the extension's contract with
// `jarvy validate --format json`. Pure logic (no vscode import), so it runs
// under node's built-in test runner without a headless editor.
//
// Run: npm run compile && npm test  (imports the compiled out/types.js).

const { test } = require("node:test");
const assert = require("node:assert/strict");
const { parseValidationResult } = require("../out/types.js");

test("parses a real jarvy validate payload (invalid config)", () => {
  // Shape captured from `jarvy validate --strict --format json` on a config
  // with an unknown tool.
  const raw = {
    path: "/tmp/jarvy.toml",
    valid: false,
    error_count: 1,
    warning_count: 0,
    issues: [
      {
        severity: "error",
        message: "Unknown tool: 'notarealtool'",
        suggestion: "Did you mean 'dotnet'?",
      },
    ],
  };
  const r = parseValidationResult(raw);
  assert.ok(r, "should parse");
  assert.equal(r.valid, false);
  assert.equal(r.error_count, 1);
  assert.equal(r.issues.length, 1);
  assert.equal(r.issues[0].severity, "error");
  assert.equal(r.issues[0].suggestion, "Did you mean 'dotnet'?");
  // No line info in the payload → undefined, not null/0.
  assert.equal(r.issues[0].line, undefined);
});

test("parses a valid (empty-issues) payload", () => {
  const r = parseValidationResult({
    path: "./jarvy.toml",
    valid: true,
    error_count: 0,
    warning_count: 0,
    issues: [],
  });
  assert.ok(r);
  assert.equal(r.valid, true);
  assert.equal(r.issues.length, 0);
});

test("keeps a 1-based line number when present", () => {
  const r = parseValidationResult({
    valid: false,
    issues: [{ severity: "warning", message: "x", line: 5 }],
  });
  assert.equal(r.issues[0].line, 5);
});

test("drops issues with an unknown severity", () => {
  const r = parseValidationResult({
    valid: false,
    issues: [
      { severity: "fatal", message: "dropped" },
      { severity: "info", message: "kept" },
    ],
  });
  assert.equal(r.issues.length, 1);
  assert.equal(r.issues[0].message, "kept");
});

test("falls back to a default message when message is missing/non-string", () => {
  const r = parseValidationResult({
    valid: false,
    issues: [{ severity: "error" }],
  });
  assert.equal(r.issues[0].message, "Validation issue");
});

test("returns undefined for malformed top-level shapes", () => {
  for (const bad of [
    null,
    undefined,
    42,
    "string",
    {},                                   // missing valid + issues
    { valid: true },                      // missing issues
    { valid: "yes", issues: [] },         // valid not boolean
    { valid: true, issues: "nope" },      // issues not array
  ]) {
    assert.equal(parseValidationResult(bad), undefined, `should reject ${JSON.stringify(bad)}`);
  }
});
