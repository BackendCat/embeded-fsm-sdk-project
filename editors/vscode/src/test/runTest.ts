// @vscode/test-electron entry — downloads a real headless VS Code and
// launches the Extension Host with this extension (Doc 28 §5 / §2.2: the
// mandated §5.4 analogue — a real editor + the real server, asserting
// observable editor state, NOT manifest/symbol presence).
//
// DISK (Doc 28 §2 Option A): @vscode/test-electron's default cache is
// out-of-tree at `~/.vscode-test`. We pin it explicitly to that out-of-tree
// dir (NEVER under /root/dev) so the VS Code download never co-locates with
// the cargo-target trough.

import * as os from "os";
import * as path from "path";

import { runTests } from "@vscode/test-electron";

async function main(): Promise<void> {
  try {
    const extensionDevelopmentPath = path.resolve(__dirname, "../../");
    const extensionTestsPath = path.resolve(__dirname, "./suite/index");

    // Out-of-tree VS Code download cache (Doc 28 §2 Option A). Honoured by
    // @vscode/test-electron via the `cachePath` option.
    const cachePath = path.join(os.homedir(), ".vscode-test");

    await runTests({
      extensionDevelopmentPath,
      extensionTestsPath,
      // Isolate from the host VS Code profile; open no workspace folder so
      // the R-15 fixture dir (an OS temp dir, no fsm.toml up-tree) is the
      // only path scope the CLI oracle ever walks. `--no-sandbox` is
      // required when running Electron headless as root in CI (the
      // Chromium sandbox cannot be set up there and otherwise SIGKILLs
      // spawned child processes — the language server included).
      launchArgs: ["--disable-extensions", "--disable-gpu", "--no-sandbox"],
      cachePath,
    });
  } catch (err) {
    console.error("Failed to run Extension-Host tests:", err);
    process.exit(1);
  }
}

void main();
