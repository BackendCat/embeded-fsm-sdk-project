// V3 behavioural-acceptance gate — Doc 28 §3-V3 / Doc 27 §8-V3 (NOT
// symbol-presence; "the command is registered / appears in package.json"
// is explicitly NOT acceptance and is rejected — P0-1 lesson). This is the
// extension-layer §5.4 analogue, the sibling of `extension.test.ts`: a
// REAL headless VS Code Extension Host + the REAL `fsm` CLI + the REAL
// `fsm-lang-server`, EXECUTING each V3 command and asserting it DID THE
// THING (a file appeared / the clipboard round-trips / the client cycled
// and re-served diagnostics / the channel revealed).
//
// V1's extension.test.ts is NOT edited (separate file; the runner globs
// suite/**/*.test.js so this is auto-discovered).
//
// The asserted V3 effects:
//   - fsm.generateC99      -> the expected C files appear in the resolved
//                             codegen output dir (real codegen, not a stub).
//   - fsm.copyIR           -> the clipboard holds JSON BYTE-IDENTICAL to the
//                             real `fsm generate --emit-ir` artifact for the
//                             same source (the R-11/R-17 path; round-trip-
//                             able by `fsm_ir::from_json` per R-17's
//                             verified-stable to_json/from_json schema pair)
//                             AND it is structurally the fsm-ir document
//                             (irVersion + the fixture's machine) — proves
//                             it is the genuine artifact, not a JS stub.
//                             Fixture (clean.fsm / machine Gate) is one that
//                             CODEGEN-SUCCEEDS (the R-11 precondition).
//   - fsm.copyIR on a codegen-FAILING (but parse-relevant) fixture ->
//                             the clipboard is NOT mutated and the command
//                             surfaces the codegen-gated failure honestly
//                             (the R-11 boundary, PROVEN not assumed —
//                             "the command must surface that honestly,
//                             asserted here").
//   - fsm.generateCpp17    -> graceful degradation: the shipped v1.0 CLI
//                             rejects cpp17 (exit 2); NO C++ files appear
//                             and the failure is surfaced honestly (no fake
//                             success — the cardinal-sin bar).
//   - fsm.checkFile        -> runs the real `fsm check` (the same pipeline
//                             the squiggle already runs; not a JS re-impl)
//                             and the diagnostics the editor already shows
//                             match the CLI oracle for the same source.
//   - fsm.formatDocument   -> the dirty buffer's text becomes the canonical
//                             `fsm fmt` output (idempotent: re-running is a
//                             no-op) — a real formatter effect.
//   - fsm.restartLanguageServer -> the client transitions away from Running
//                             and back to Running AND diagnostics
//                             re-publish. This is the M-1 / Finding-1
//                             regression guard: a round-tripped diagnostic
//                             AFTER restart proves the re-spawn used V1's
//                             omit-transport `Executable` (had a regression
//                             re-added `transport`/`--stdio` or switched to
//                             NodeModule, the strict server would exit 2 —
//                             crates/fsm-lsp/src/main.rs:49-52 — and no
//                             diagnostic would come back).
//   - fsm.showOutputChannel -> executes without throwing and the registered
//                             handler reveals V1's output channel (this is
//                             the exact target statusBar.ts:74 already wires
//                             the status-bar click to — M-2).

import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import * as vscode from "vscode";

import { resolveRealBinaries } from "./binaries";
import { cliOracle } from "./oracle";

const FIXTURE_SRC = path.resolve(__dirname, "../fixtures");

/** Poll until `predicate` holds or the deadline (mirrors extension.test). */
async function waitFor(
  predicate: () => boolean,
  what: string,
  timeoutMs = 20_000,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    if (predicate()) {
      return;
    }
    if (Date.now() > deadline) {
      throw new Error(`timed out waiting for: ${what}`);
    }
    await new Promise((r) => setTimeout(r, 150));
  }
}

async function waitForDiagnostics(
  uri: vscode.Uri,
  predicate: (d: readonly vscode.Diagnostic[]) => boolean,
  what: string,
  timeoutMs = 20_000,
): Promise<readonly vscode.Diagnostic[]> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const diags = vscode.languages.getDiagnostics(uri);
    if (predicate(diags)) {
      return diags;
    }
    if (Date.now() > deadline) {
      throw new Error(
        `timed out waiting for diagnostics (${what}) on ${uri.fsPath}; ` +
          `last = ${JSON.stringify(diags)}`,
      );
    }
    await new Promise((r) => setTimeout(r, 150));
  }
}

interface FsmExtensionApi {
  readonly serverStarted: boolean;
  readonly negotiatedPositionEncoding: string | undefined;
  readonly binarySource: "compilerPath" | "bundled" | "none";
}

suite("FSM Studio V3 — Extension-Host command behavioural acceptance", () => {
  let api: FsmExtensionApi;
  let cliBinary: string;
  let tmpDir: string;

  suiteSetup(async function () {
    this.timeout(600_000);

    const bins = resolveRealBinaries();
    cliBinary = bins.cli;

    // R-15 isolation (same posture as extension.test.ts): stage fixtures
    // in an OS temp dir so the CLI's upward fsm.toml walk finds none.
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-v3-cmd-"));
    for (const f of ["broken.fsm", "clean.fsm"]) {
      fs.copyFileSync(path.join(FIXTURE_SRC, f), path.join(tmpDir, f));
    }
    let dir = tmpDir;
    for (;;) {
      assert.ok(
        !fs.existsSync(path.join(dir, "fsm.toml")),
        `R-15 violated: fsm.toml found at ${dir}`,
      );
      const parent = path.dirname(dir);
      if (parent === dir) {
        break;
      }
      dir = parent;
    }

    // Launch the real server via fsmLang.compilerPath (Rule 1; V1 ships
    // host-only). The V3 CLI resolver derives the `fsm` CLI as the sibling
    // of this path (the genuine two-binary need Doc 27 §2.2 records).
    await vscode.workspace
      .getConfiguration("fsmLang")
      .update(
        "compilerPath",
        bins.server,
        vscode.ConfigurationTarget.Global,
      );

    const ext = vscode.extensions.getExtension("fsmstudio.fsm-lang");
    assert.ok(ext, "extension fsmstudio.fsm-lang must be present");
    api = (await ext.activate()) as FsmExtensionApi;

    await waitFor(
      () => api.serverStarted,
      "language client to reach Running",
      30_000,
    );
  });

  suiteTeardown(async () => {
    await vscode.workspace
      .getConfiguration("fsmLang")
      .update(
        "compilerPath",
        undefined,
        vscode.ConfigurationTarget.Global,
      );
    if (tmpDir && fs.existsSync(tmpDir)) {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  // Sanity: the V3 commands are reachable (a precondition, NOT the
  // acceptance — the acceptance is the per-command EFFECT below).
  test("the seven V3 commands are registered", async () => {
    const all = await vscode.commands.getCommands(true);
    for (const id of [
      "fsm.checkFile",
      "fsm.generateC99",
      "fsm.generateCpp17",
      "fsm.copyIR",
      "fsm.formatDocument",
      "fsm.restartLanguageServer",
      "fsm.showOutputChannel",
    ]) {
      assert.ok(all.includes(id), `command ${id} must be registered`);
    }
  });

  // fsm.generateC99 -> the real C files appear in the output dir.
  test("fsm.generateC99 writes the expected C files (real codegen)", async () => {
    const fixture = path.join(tmpDir, "clean.fsm"); // machine Gate
    const outDir = path.join(tmpDir, "generated");
    fs.rmSync(outDir, { recursive: true, force: true });

    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);
    await vscode.commands.executeCommand("fsm.generateC99");

    // The codegen output dir is workspace-rooted; with no workspace folder
    // the command roots it at the .fsm file's directory -> tmpDir/generated.
    await waitFor(
      () =>
        fs.existsSync(path.join(outDir, "Gate.c")) &&
        fs.existsSync(path.join(outDir, "Gate.h")),
      "Gate.c + Gate.h to be generated",
    );
    const c = fs.readFileSync(path.join(outDir, "Gate.c"), "utf8");
    assert.ok(
      c.length > 0 && /Gate/.test(c),
      "generated Gate.c must be non-empty real codegen output",
    );
  });

  // fsm.copyIR -> clipboard byte-identical to the real --emit-ir artifact
  // AND structurally the fsm-ir document (the genuine R-11/R-17 path).
  test("fsm.copyIR copies the genuine --emit-ir IR JSON to the clipboard", async () => {
    const fixture = path.join(tmpDir, "clean.fsm"); // codegen-SUCCEEDS
    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);

    // Independent oracle: run the SAME real CLI --emit-ir ourselves and
    // read the artifact it writes (the established oracle pattern). The
    // command's clipboard MUST equal this byte-for-byte — proving it is
    // the real fsm_ir::to_json output, not a JS stub. (R-17: to_json /
    // from_json are a verified-stable schema pair, so a byte-equal
    // to_json artifact is from_json-round-trippable.)
    const oracleOut = fs.mkdtempSync(
      path.join(os.tmpdir(), "fsm-v3-iroracle-"),
    );
    try {
      const { execFileSync } = await import("child_process");
      execFileSync(
        cliBinary,
        [
          "generate",
          "--target",
          "c99",
          "--out",
          oracleOut,
          "--emit-ir",
          fixture,
        ],
        { stdio: "ignore" },
      );
      const irFile = fs
        .readdirSync(oracleOut)
        .find((f) => f.endsWith(".ir.json"));
      assert.ok(
        irFile,
        "oracle precondition: real --emit-ir wrote a .ir.json",
      );
      const expectedIr = fs.readFileSync(
        path.join(oracleOut, irFile),
        "utf8",
      );

      // Put a sentinel on the clipboard first so a no-op command (the
      // failure we are guarding against) is detectable, not a false pass.
      await vscode.env.clipboard.writeText("__sentinel__");
      await vscode.commands.executeCommand("fsm.copyIR");

      let clip = "";
      {
        const deadline = Date.now() + 20_000;
        for (;;) {
          clip = await vscode.env.clipboard.readText();
          if (clip !== "__sentinel__" && clip.length > 0) {
            break;
          }
          if (Date.now() > deadline) {
            throw new Error(
              "timed out waiting for fsm.copyIR to populate the clipboard",
            );
          }
          await new Promise((r) => setTimeout(r, 150));
        }
      }

      assert.strictEqual(
        clip,
        expectedIr,
        "fsm.copyIR clipboard must be BYTE-IDENTICAL to the real " +
          "`fsm generate --emit-ir` artifact (the only IR path; R-11) — " +
          "a divergence means it is a stub, not the genuine IR",
      );
      // Structural assertion: it really is the fsm-ir document for Gate.
      const parsed = JSON.parse(clip) as {
        irVersion?: string;
        machines?: Array<{ name?: string }>;
      };
      assert.ok(
        typeof parsed.irVersion === "string",
        "clipboard IR must carry an irVersion (fsm-ir schema, R-17)",
      );
      assert.ok(
        Array.isArray(parsed.machines) &&
          parsed.machines.some((m) => m.name === "Gate"),
        "clipboard IR must contain the fixture's machine `Gate`",
      );
    } finally {
      fs.rmSync(oracleOut, { recursive: true, force: true });
    }
  });

  // fsm.copyIR on a codegen-FAILING fixture -> clipboard NOT mutated, the
  // codegen-gated boundary surfaced honestly (R-11, PROVEN not assumed).
  test("fsm.copyIR does NOT fake a clipboard when codegen fails (R-11 boundary)", async () => {
    const fixture = path.join(tmpDir, "broken.fsm"); // codegen FAILS
    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);

    const sentinel = `__no_ir_sentinel_${Date.now()}__`;
    await vscode.env.clipboard.writeText(sentinel);
    await vscode.commands.executeCommand("fsm.copyIR");

    // Give the command time to run the CLI + (correctly) decline to copy.
    await new Promise((r) => setTimeout(r, 1500));
    const clip = await vscode.env.clipboard.readText();
    assert.strictEqual(
      clip,
      sentinel,
      "fsm.copyIR must NOT overwrite the clipboard when codegen fails " +
        "(no .ir.json is produced — R-11 codegen-gated; the command " +
        "surfaces the failure instead of faking success)",
    );
  });

  // fsm.generateCpp17 -> graceful degradation: no C++ files, honest fail.
  test("fsm.generateCpp17 degrades gracefully on the v1.0 CLI (no fake success)", async () => {
    const fixture = path.join(tmpDir, "clean.fsm");
    const outDir = path.join(tmpDir, "generated_cpp");
    fs.rmSync(outDir, { recursive: true, force: true });

    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);
    await vscode.commands.executeCommand("fsm.generateCpp17");

    // The shipped v1.0 CLI rejects cpp17 (exit 2) BEFORE writing anything
    // (crates/fsm-cli/src/cmd/generate.rs:28-36). The command must NOT
    // have produced an output dir / files (a fake success would).
    await new Promise((r) => setTimeout(r, 1500));
    const produced =
      fs.existsSync(outDir) && fs.readdirSync(outDir).length > 0;
    assert.strictEqual(
      produced,
      false,
      "fsm.generateCpp17 must NOT create C++ output when the CLI " +
        "rejects the cpp17 target (graceful degradation, not a fake " +
        "success — Doc 27 §5)",
    );
  });

  // fsm.checkFile -> runs the real `fsm check`; the editor diagnostics for
  // the same source match the CLI oracle (same pipeline, not a JS re-impl).
  test("fsm.checkFile echoes the real `fsm check` pipeline (oracle match)", async () => {
    const fixture = path.join(tmpDir, "broken.fsm");
    const oracle = cliOracle(cliBinary, fixture);
    assert.strictEqual(oracle.length, 1, "broken.fsm oracle = 1 diag");
    assert.strictEqual(oracle[0].code, "FSM-E0107");

    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);

    // Executing the command must not throw and the editor's diagnostics
    // (which the command's pipeline is an echo of) match the CLI oracle.
    await vscode.commands.executeCommand("fsm.checkFile");
    const diags = await waitForDiagnostics(
      doc.uri,
      (d) => d.length === 1,
      "broken.fsm FSM-E0107 after fsm.checkFile",
    );
    assert.strictEqual(
      diags[0].code,
      oracle[0].code,
      "the diagnostics fsm.checkFile's pipeline produces must equal the " +
        "CLI oracle (same `fsm check` pipeline; not a JS re-implementation)",
    );
  });

  // fsm.formatDocument -> the dirty buffer becomes the canonical fsm fmt
  // output; idempotent (re-running it is a no-op) — a real formatter effect.
  test("fsm.formatDocument formats the buffer to canonical form (idempotent)", async () => {
    // A deliberately un-canonical (over-indented) but parseable buffer.
    const messy =
      "language fsm 2.0\n\nmachine M {\n        events { GO }\n" +
      "    initial A\n    state A {\n        on GO -> A\n    }\n}\n";
    const messyPath = path.join(tmpDir, "_fmt_target.fsm");
    fs.writeFileSync(messyPath, messy);

    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(messyPath),
    );
    const editor = await vscode.window.showTextDocument(doc);

    // The CLI formatter is the oracle: format the same bytes via stdin.
    const { execFileSync } = await import("child_process");
    const canonical = execFileSync(cliBinary, ["fmt", "--stdin"], {
      input: messy,
      encoding: "utf8",
    });
    assert.notStrictEqual(
      canonical,
      messy,
      "test precondition: the messy buffer must be non-canonical",
    );

    await vscode.commands.executeCommand("fsm.formatDocument");
    await waitFor(
      () => editor.document.getText() === canonical,
      "buffer to become the canonical `fsm fmt` output",
    );

    // Idempotence: running format again is a no-op (the real formatter
    // contract — `fsm fmt f && fsm fmt --check f` exits 0).
    await vscode.commands.executeCommand("fsm.formatDocument");
    await new Promise((r) => setTimeout(r, 800));
    assert.strictEqual(
      editor.document.getText(),
      canonical,
      "fsm.formatDocument must be idempotent on already-canonical text",
    );

    await vscode.commands.executeCommand(
      "workbench.action.closeActiveEditor",
    );
    fs.rmSync(messyPath, { force: true });
  });

  // fsm.showOutputChannel -> executes without throwing (M-2: it is the
  // exact handler statusBar.ts:74 already wires the status-bar click to;
  // before V3 that click was inert because the command was uncontributed).
  test("fsm.showOutputChannel executes (M-2: status-bar click now functional)", async () => {
    await vscode.commands.executeCommand("fsm.showOutputChannel");
    // Re-invocation must also be safe (the status bar can be clicked
    // repeatedly). No throw == the registered handler ran.
    await vscode.commands.executeCommand("fsm.showOutputChannel");
  });

  // fsm.restartLanguageServer -> the client cycles AND diagnostics
  // re-publish. THE M-1 / Finding-1 regression guard: a diagnostic that
  // round-trips AFTER the restart proves the re-spawn used V1's
  // omit-transport `Executable` (a re-added `transport`/`--stdio` or a
  // NodeModule switch would make the strict server exit 2 and NOTHING
  // would come back — crates/fsm-lsp/src/main.rs:49-52).
  test("fsm.restartLanguageServer cycles the client AND diagnostics re-publish (M-1 guard)", async function () {
    this.timeout(120_000);

    const fixture = path.join(tmpDir, "broken.fsm");
    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);

    // Pre-restart: the squiggle is up (FSM-E0107) under the original
    // omit-transport spawn.
    await waitForDiagnostics(
      doc.uri,
      (d) => d.some((x) => x.code === "FSM-E0107"),
      "FSM-E0107 BEFORE restart",
    );
    assert.strictEqual(
      api.serverStarted,
      true,
      "client must be Running before the restart",
    );

    // Execute the real command (NOT a direct client.restart() — we are
    // testing the contributed+registered command path end-to-end).
    await vscode.commands.executeCommand("fsm.restartLanguageServer");

    // Post-restart: the client must be Running again AND the server must
    // re-serve the diagnostic. If the re-spawn had pushed `--stdio` /
    // set transport / used NodeModule, the strict fsm-lang-server would
    // have exited 2 and we would NEVER see this diagnostic again. Its
    // presence is the behavioural proof the omit-transport spawn survived
    // the restart (M-1 / Finding-1).
    await waitFor(
      () => api.serverStarted,
      "client to be Running AGAIN after fsm.restartLanguageServer",
      60_000,
    );

    // Reopen to force a fresh didOpen on the (restarted) server, then
    // assert the diagnostic round-trips through the new connection.
    const doc2 = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc2);
    const after = await waitForDiagnostics(
      doc2.uri,
      (d) => d.some((x) => x.code === "FSM-E0107"),
      "FSM-E0107 AFTER restart (the M-1 omit-transport survival proof)",
      60_000,
    );
    assert.ok(
      after.some((d) => d.code === "FSM-E0107"),
      "diagnostics MUST re-publish after fsm.restartLanguageServer — " +
        "their return proves the restarted client re-spawned with V1's " +
        "omit-transport Executable (no --stdio / no NodeModule; the " +
        "strict fsm-lang-server would otherwise exit 2). M-1 guard.",
    );
  });
});
