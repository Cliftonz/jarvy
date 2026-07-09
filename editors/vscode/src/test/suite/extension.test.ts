// Headless integration tests: drive the real extension inside a VS Code
// host against the real `jarvy` binary and assert observable behavior
// (activation + diagnostics), not internals.
import * as assert from "assert";
import { execFileSync } from "child_process";
import * as path from "path";
import * as vscode from "vscode";

const EXTENSION_ID = "jarvy.jarvy-vscode";

/** True when a `jarvy` binary is resolvable on PATH. */
function jarvyOnPath(): boolean {
  try {
    execFileSync("jarvy", ["--version"], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

async function waitFor(
  predicate: () => boolean,
  timeoutMs: number,
  intervalMs = 250,
): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) {
      return true;
    }
    await new Promise((r) => setTimeout(r, intervalMs));
  }
  return predicate();
}

suite("Jarvy extension — integration", () => {
  suiteSetup(async () => {
    const ext = vscode.extensions.getExtension(EXTENSION_ID);
    assert.ok(ext, `extension ${EXTENSION_ID} should be present`);
    await ext!.activate();
  });

  test("activates and contributes its commands", async () => {
    const ext = vscode.extensions.getExtension(EXTENSION_ID);
    assert.ok(ext?.isActive, "extension should be active");
    const commands = await vscode.commands.getCommands(true);
    for (const id of ["jarvy.validate", "jarvy.setup", "jarvy.doctor"]) {
      assert.ok(commands.includes(id), `command ${id} should be registered`);
    }
  });

  test("publishes an error diagnostic for an unknown tool", async function () {
    if (!jarvyOnPath()) {
      // The whole point is to test against the real binary. Skip cleanly on
      // machines that haven't built jarvy rather than fail spuriously; CI
      // guarantees it's on PATH.
      this.skip();
    }

    // The fixture workspace root holds a jarvy.toml with an unknown tool.
    const folders = vscode.workspace.workspaceFolders;
    assert.ok(folders && folders.length > 0, "a workspace folder is expected");
    const tomlUri = vscode.Uri.file(
      path.join(folders![0].uri.fsPath, "jarvy.toml"),
    );

    const doc = await vscode.workspace.openTextDocument(tomlUri);
    await vscode.window.showTextDocument(doc);
    // Force a validation pass (also fires on open, but be deterministic).
    await vscode.commands.executeCommand("jarvy.validate");

    const appeared = await waitFor(
      () => vscode.languages.getDiagnostics(tomlUri).length > 0,
      30_000,
    );
    const diags = vscode.languages.getDiagnostics(tomlUri);
    assert.ok(appeared && diags.length > 0, "expected at least one diagnostic");

    const hasUnknownTool = diags.some(
      (d) =>
        d.severity === vscode.DiagnosticSeverity.Error &&
        /unknown tool/i.test(d.message),
    );
    assert.ok(
      hasUnknownTool,
      `expected an 'Unknown tool' error; got: ${diags
        .map((d) => `[${d.severity}] ${d.message}`)
        .join(" | ")}`,
    );
  });
});
