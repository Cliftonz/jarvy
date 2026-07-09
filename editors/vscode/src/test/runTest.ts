// Entry point for the headless integration test. Downloads a VS Code build
// (@vscode/test-electron), then launches it with this extension loaded and a
// fixture workspace open, running the Mocha suite inside the extension host.
//
// The extension shells out to `jarvy`; put the built binary on PATH before
// running (CI: `cargo build` then prepend target/debug). If `jarvy` is not on
// PATH the diagnostics test is skipped rather than failing spuriously.
import * as path from "path";
import { runTests } from "@vscode/test-electron";

async function main(): Promise<void> {
  try {
    // editors/vscode/ (the extension root that holds package.json).
    const extensionDevelopmentPath = path.resolve(__dirname, "../../");
    // Compiled suite entry (out/test/suite/index.js).
    const extensionTestsPath = path.resolve(__dirname, "./suite/index");
    // Fixture workspace with a jarvy.toml so the extension activates.
    const workspace = path.resolve(
      __dirname,
      "../../src/test/fixtures/workspace",
    );

    await runTests({
      extensionDevelopmentPath,
      extensionTestsPath,
      launchArgs: [workspace, "--disable-extensions"],
    });
  } catch (err) {
    console.error("Failed to run integration tests:", err);
    process.exit(1);
  }
}

void main();
