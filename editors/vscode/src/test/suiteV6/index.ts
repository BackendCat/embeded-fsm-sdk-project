// Mocha runner for the V6 acceptance suite, loaded inside a DEDICATED
// Extension Host (separate from the V1–V5 `suite/` host) so the extension
// activates with `fsmLang.compilerPath` UNSET — the precondition that
// forces V1's Doc 22 §2.2 Rule-2 ("bundled host-triple") path (Doc 28
// §3-V6 / Doc 27 §8-V6). Mirrors `suite/index.ts`; globs only suiteV6/.

import * as path from "path";

import { glob } from "glob";
import Mocha from "mocha";

export async function run(): Promise<void> {
  const mocha = new Mocha({
    ui: "tdd",
    color: true,
    // Cold release build + vsce package can be slow on a busy box.
    timeout: 120_000,
  });

  const testsRoot = path.resolve(__dirname, "..");
  const files = await glob("suiteV6/**/*.test.js", { cwd: testsRoot });
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
