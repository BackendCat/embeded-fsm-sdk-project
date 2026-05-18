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

    // Phase-6.0-W4 (Doc 32 §W4 deliverable (2)): the EXISTING
    // `xvfb-run @vscode/test-electron` host is wrapped by `c8` (GT-11 —
    // the extension's OWN coverage, not a new harness). `c8` sets
    // `NODE_V8_COVERAGE` to a temp dir in OUR (launcher) process env, but
    // the extension code (`out/**` ← sourcemap ← `src/**`) actually
    // executes in the spawned Extension-Host (Electron/Node) process.
    // V8's `NODE_V8_COVERAGE` is per-process, so we MUST forward it into
    // the Extension Host via `extensionTestsEnv` (test-electron merges it:
    // `Object.assign({}, process.env, testRunnerEnv)`), or c8 would see an
    // empty profile and the gate would be vacuously green. Forward it ONLY
    // when c8 set it (normal `npm test` runs are unaffected — zero
    // behavioural change when not under coverage).
    const v8CovDir = process.env.NODE_V8_COVERAGE;
    const extensionTestsEnv = v8CovDir ? { NODE_V8_COVERAGE: v8CovDir } : undefined;

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
      // Forward the c8 V8-coverage sink into the Extension Host so the
      // extension's own executed code is profiled (Doc 32 §W4 (2)).
      extensionTestsEnv,
    });
  } catch (err) {
    console.error("Failed to run Extension-Host tests:", err);
    process.exit(1);
  }
}

void main();
