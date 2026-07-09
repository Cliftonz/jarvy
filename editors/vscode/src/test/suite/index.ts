// Mocha runner executed inside the VS Code extension host. Discovers every
// compiled *.test.js in this directory and runs it. (Uses fs.readdirSync
// rather than glob to avoid a dependency and glob's cross-version API churn.)
import * as fs from "fs";
import * as path from "path";
import Mocha from "mocha";

export function run(): Promise<void> {
  const mocha = new Mocha({ ui: "tdd", color: true, timeout: 60_000 });
  const suiteDir = __dirname;

  for (const file of fs.readdirSync(suiteDir)) {
    if (file.endsWith(".test.js")) {
      mocha.addFile(path.resolve(suiteDir, file));
    }
  }

  return new Promise((resolve, reject) => {
    try {
      mocha.run((failures) => {
        if (failures > 0) {
          reject(new Error(`${failures} test(s) failed.`));
        } else {
          resolve();
        }
      });
    } catch (err) {
      reject(err instanceof Error ? err : new Error(String(err)));
    }
  });
}
