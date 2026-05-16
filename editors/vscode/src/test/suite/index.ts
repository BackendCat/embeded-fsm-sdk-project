// Mocha runner loaded inside the Extension Host (Doc 28 §5 acceptance).

import * as path from "path";

import { glob } from "glob";
import Mocha from "mocha";

export async function run(): Promise<void> {
  const mocha = new Mocha({
    ui: "tdd",
    color: true,
    // The suite drives a real server over stdio with a 5 s publish window;
    // a generous per-test timeout keeps a slow CI box from a false red.
    timeout: 60_000,
  });

  const testsRoot = path.resolve(__dirname, "..");
  const files = await glob("suite/**/*.test.js", { cwd: testsRoot });
  for (const f of files) {
    mocha.addFile(path.resolve(testsRoot, f));
  }

  await new Promise<void>((resolve, reject) => {
    try {
      mocha.run((failures) => {
        if (failures > 0) {
          reject(new Error(`${failures} test(s) failed.`));
        } else {
          resolve();
        }
      });
    } catch (err) {
      reject(err as Error);
    }
  });
}
