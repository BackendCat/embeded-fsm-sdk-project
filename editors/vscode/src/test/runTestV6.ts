// @vscode/test-electron entry for the V6 acceptance suite — a DEDICATED
// headless VS Code launch, separate from `runTest.ts` (the V1–V5 host),
// for one reason that is load-bearing to the V6 gate:
//
//   The V6 §5.4-analogue proof requires the extension to activate down
//   V1's Doc 22 §2.2 Rule-2 ("bundled host-triple") path. That path is
//   chosen ONLY when `fsmLang.compilerPath` is empty. The V1 suite
//   (`runTest.ts`) deliberately SETS `compilerPath` (its Rule-1 launch),
//   and the extension's `binarySource` is captured once at activation. To
//   observe Rule-2 we therefore need a pristine Host where compilerPath
//   was NEVER written — a separate launch with its own throwaway
//   `--user-data-dir` (so no global setting can leak in) and
//   `--disable-extensions` (same isolation posture as runTest.ts).
//
// DISK (Doc 28 §2 Option A): the @vscode/test-electron download cache is
// pinned OUT-OF-TREE at `~/.vscode-test` (shared with runTest.ts — same
// cachePath, so no second multi-hundred-MB download); the throwaway
// user-data-dir is an OS temp dir, never under /root/dev.

import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import { runTests } from "@vscode/test-electron";

async function main(): Promise<void> {
  let userDataDir: string | undefined;
  try {
    const extensionDevelopmentPath = path.resolve(__dirname, "../../");
    const extensionTestsPath = path.resolve(__dirname, "./suiteV6/index");
    const cachePath = path.join(os.homedir(), ".vscode-test");

    // A throwaway, pristine VS Code user profile so `fsmLang.compilerPath`
    // is GUARANTEED unset — the precondition that forces V1's Rule-2.
    userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-v6-userdata-"));

    await runTests({
      extensionDevelopmentPath,
      extensionTestsPath,
      launchArgs: [
        "--disable-extensions",
        "--disable-gpu",
        "--no-sandbox",
        "--user-data-dir",
        userDataDir,
      ],
      cachePath,
    });
  } catch (err) {
    console.error("Failed to run V6 Extension-Host tests:", err);
    process.exitCode = 1;
  } finally {
    if (userDataDir && fs.existsSync(userDataDir)) {
      fs.rmSync(userDataDir, { recursive: true, force: true });
    }
  }
}

void main();
