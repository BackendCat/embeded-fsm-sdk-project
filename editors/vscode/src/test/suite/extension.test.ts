// V1 behavioural-acceptance gate — Doc 28 §5 (NOT optional, NOT
// symbol-presence). The extension-layer analogue of the LSP
// `lsp_client_acceptance.rs` harness: a REAL headless VS Code Extension
// Host + the REAL `fsm-lang-server` binary, asserting OBSERVABLE editor
// state (`vscode.languages.getDiagnostics`) byte-equal to the established
// `fsm check --json` CLI oracle for the identical source.
//
// The four required assertions (Doc 28 §5 (a)-(d)):
//   (a) known-broken .fsm  -> getDiagnostics has the EXACT FSM-Exxxx code
//       AND the EXACT Range `fsm check --json` reports (R-15 fixture: no
//       fsm.toml [compiler] allow/deny in scope — staged in an OS temp dir
//       so the CLI's upward walk finds none).
//   (b) edit-to-fix        -> diagnostics clear.
//   (c) positionEncoding negotiates CORRECTLY for the official client.
//       V1-SPINE FINDING: Doc 27 §2.3 / R-4 expected UTF-8; the official
//       vscode-languageclient (v8/v9) hardcodes ['utf-16'] and throws on
//       anything else, so the server's documented UTF-16 fallback is what
//       runs (correct; loses only the perf fast-path). (c) asserts the
//       negotiation is the correct UTF-16 (not a throw/failure); the
//       surviving substantive R-4 guard is (d), run under that UTF-16.
//   (d) first error AFTER a non-ASCII line -> Range correct (Doc 26 §4.1
//       defect-class guard, end-to-end through the client, under UTF-16).
//
// Manifest-presence / "the .ts compiles" / unit-testing JS internals is
// explicitly NOT acceptance here.

import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import * as vscode from "vscode";

import { resolveRealBinaries } from "./binaries";
import {
  OracleDiag,
  assertAsciiColumnInvariant,
  cliOracle,
} from "./oracle";

const FIXTURE_SRC = path.resolve(__dirname, "../fixtures");

interface FsmExtensionApi {
  readonly serverStarted: boolean;
  readonly negotiatedPositionEncoding: string | undefined;
  readonly binarySource: "compilerPath" | "bundled" | "none";
}

/** Poll `getDiagnostics(uri)` until `predicate` holds or the deadline. */
async function waitForDiagnostics(
  uri: vscode.Uri,
  predicate: (d: readonly vscode.Diagnostic[]) => boolean,
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
        `timed out waiting for diagnostics on ${uri.fsPath}; ` +
          `last = ${JSON.stringify(diags)}`,
      );
    }
    await new Promise((r) => setTimeout(r, 150));
  }
}

function assertRangeMatchesOracle(
  got: vscode.Diagnostic,
  want: OracleDiag,
  label: string,
): void {
  assert.strictEqual(
    got.code,
    want.code,
    `[${label}] diagnostic code must equal the CLI oracle`,
  );
  assert.strictEqual(
    got.range.start.line,
    want.startLine,
    `[${label}] start.line must byte-match fsm check --json`,
  );
  assert.strictEqual(
    got.range.start.character,
    want.startChar,
    `[${label}] start.character must byte-match fsm check --json`,
  );
  assert.strictEqual(
    got.range.end.line,
    want.endLine,
    `[${label}] end.line must byte-match fsm check --json`,
  );
  assert.strictEqual(
    got.range.end.character,
    want.endChar,
    `[${label}] end.character must byte-match fsm check --json`,
  );
}

suite("FSM Studio V1 — Extension-Host behavioural acceptance", () => {
  let api: FsmExtensionApi;
  let cliBinary: string;
  let tmpDir: string;

  suiteSetup(async function () {
    this.timeout(600_000); // a cold cargo build can be slow on a busy box.

    // 1. The REAL binaries on the pinned toolchain (build from repo root).
    const bins = resolveRealBinaries();
    cliBinary = bins.cli;

    // 2. R-15 isolation: stage fixtures in an OS temp dir (NOT under the
    //    repo) so the CLI's upward fsm.toml walk terminates at / with none
    //    found — apply_allow_deny is a guaranteed no-op.
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-v1-accept-"));
    for (const f of ["broken.fsm", "clean.fsm", "non_ascii_doc.fsm"]) {
      fs.copyFileSync(
        path.join(FIXTURE_SRC, f),
        path.join(tmpDir, f),
      );
    }
    // Hard assert no fsm.toml is reachable up-tree from the staged dir.
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

    // 3. Point the extension's resolver at the real server (Rule 1,
    //    fsmLang.compilerPath verbatim — V1 ships host-only, no bundled
    //    bin/ yet, so the explicit-path rule is the V1 launch path).
    await vscode.workspace
      .getConfiguration("fsmLang")
      .update(
        "compilerPath",
        bins.server,
        vscode.ConfigurationTarget.Global,
      );

    // 4. Activate the extension and capture its test API.
    const ext = vscode.extensions.getExtension("fsmstudio.fsm-lang");
    assert.ok(ext, "extension fsmstudio.fsm-lang must be present");
    api = (await ext.activate()) as FsmExtensionApi;

    // Wait for the language client to actually reach Running.
    const startDeadline = Date.now() + 30_000;
    while (!api.serverStarted) {
      if (Date.now() > startDeadline) {
        throw new Error("language client never reached Running state");
      }
      await new Promise((r) => setTimeout(r, 150));
    }
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

  // (c) — the positionEncoding negotiation seam (Doc 28 §5 (c) / R-4).
  //
  // V1-SPINE FINDING (this is exactly what the depth-first gate is for):
  // Doc 27 §2.3 / R-4 expected UTF-8 to be negotiable. VERIFIED FALSE
  // against vscode-languageclient v8.0.2/v8.1.0/v9.0.1 — ALL hardcode
  // `positionEncodings = ['utf-16']` and THROW on a non-UTF-16 server
  // response (lib/common/client.js). The official client is intrinsically
  // UTF-16; UTF-8 is unreachable through it by any in-scope means. The
  // shipped server's documented FALLBACK (offer only utf-16 -> negotiate
  // UTF-16, server.rs:216-226) runs and is CORRECT (Doc 26 §4.1: loses the
  // fast-path, NOT correctness). The surviving substantive R-4 guard — a
  // suppressed-capability / wrong-encoding regression corrupting multibyte
  // ranges — is enforced END-TO-END by assertion (d) under this exact
  // UTF-16 negotiation. So (c) asserts the negotiation is the CORRECT one
  // for the official client (UTF-16, not a silent failure/throw), and (d)
  // proves the defect class is still guarded under it.
  test("(c) positionEncoding negotiates correctly for the official client (UTF-16)", () => {
    assert.strictEqual(
      api.binarySource,
      "compilerPath",
      "the test must launch the real server via fsmLang.compilerPath",
    );
    // Not undefined => the initialize round-trip completed and the client
    // did NOT throw `Unsupported position encoding` (it would have, had
    // the server negotiated anything the official client rejects). UTF-16
    // is the only value vscode-languageclient >= 8 accepts; the server
    // negotiating it on a utf-16-only offer is the CORRECT R-4 fallback.
    assert.strictEqual(
      api.negotiatedPositionEncoding,
      "utf-16",
      "the official vscode-languageclient offers only ['utf-16'] and " +
        "throws on anything else; the shipped server must negotiate the " +
        "documented UTF-16 fallback (R-4 'else UTF-16') — UTF-8 is NOT " +
        "reachable through the official client (V1-spine finding vs " +
        "Doc 27 §2.3 / Doc 28 R-4)",
    );
  });

  // (a) — known-broken .fsm: exact FSM-Exxxx code AND exact Range equal to
  // the `fsm check --json` oracle for the identical source.
  test("(a) known-broken .fsm surfaces the exact CLI-oracle code + Range", async () => {
    const fixture = path.join(tmpDir, "broken.fsm");
    const source = fs.readFileSync(fixture, "utf8");
    const oracle = cliOracle(cliBinary, fixture);
    assertAsciiColumnInvariant(source, oracle);
    assert.strictEqual(
      oracle.length,
      1,
      `oracle precondition: broken.fsm has exactly one diagnostic, ` +
        `got ${JSON.stringify(oracle)}`,
    );
    assert.strictEqual(oracle[0].code, "FSM-E0107");

    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);

    const diags = await waitForDiagnostics(
      doc.uri,
      (d) => d.length === 1,
    );
    assertRangeMatchesOracle(diags[0], oracle[0], "broken.fsm");
  });

  // (b) — edit-to-fix → diagnostics clear.
  test("(b) editing the file to fix the error clears diagnostics", async () => {
    const fixture = path.join(tmpDir, "broken.fsm");
    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    const editor = await vscode.window.showTextDocument(doc);

    await waitForDiagnostics(doc.uri, (d) => d.length === 1);

    // Oracle sanity (mirrors the LSP did_change_fixing invariant assert):
    // the replacement source really IS clean via the same CLI pipeline, so
    // a subsequent non-empty publish would be a real regression, not a bad
    // fixture. Check it on an isolated temp copy (still R-15-isolated).
    const cleanSource = fs.readFileSync(
      path.join(tmpDir, "clean.fsm"),
      "utf8",
    );
    const sanityPath = path.join(tmpDir, "_b_sanity.fsm");
    fs.writeFileSync(sanityPath, cleanSource);
    assert.strictEqual(
      cliOracle(cliBinary, sanityPath).length,
      0,
      "test invariant: the replacement buffer must be clean per fsm check",
    );
    fs.rmSync(sanityPath, { force: true });

    // Apply the fix as an in-memory edit — the client forwards a
    // didChange; the server re-analyzes the NEW buffer (not the on-disk
    // file) and must republish empty.
    await editor.edit((eb) => {
      const full = new vscode.Range(
        doc.positionAt(0),
        doc.positionAt(doc.getText().length),
      );
      eb.replace(full, cleanSource);
    });

    const cleared = await waitForDiagnostics(
      doc.uri,
      (d) => d.length === 0,
    );
    assert.strictEqual(
      cleared.length,
      0,
      "diagnostics must clear after the fixing edit (live didChange)",
    );

    // The fixing edit was in-memory only (never saved), so the on-disk
    // fixture is already untouched. Discard the dirty buffer cleanly so a
    // later test reopening it sees pristine content (revert via the
    // stable file-revert command, then close without save).
    await vscode.commands.executeCommand("workbench.action.files.revert");
    await vscode.commands.executeCommand(
      "workbench.action.closeActiveEditor",
    );
  });

  // (d) — first error AFTER a non-ASCII line: Range correct (Doc 26 §4.1
  // defect-class guard, end-to-end through the client). The astral emoji
  // (🚀, 4 bytes / 2 UTF-16) + Cyrillic sit on line 3; the error is on a
  // later line — a broken LineIndex line-start table would offset it.
  test("(d) error after a non-ASCII line has the exact CLI-oracle Range", async () => {
    const fixture = path.join(tmpDir, "non_ascii_doc.fsm");
    const source = fs.readFileSync(fixture, "utf8");
    const oracle = cliOracle(cliBinary, fixture);
    assertAsciiColumnInvariant(source, oracle);
    assert.ok(
      oracle.length >= 1,
      `oracle precondition: non_ascii_doc.fsm has >= 1 diagnostic`,
    );
    // The FIRST error is the one whose Range the §5(d) guard pins.
    const first = oracle[0];
    assert.strictEqual(
      first.code,
      "FSM-E0100",
      "first diagnostic is the unknown-state-reference error",
    );
    // It must genuinely be below the multibyte doc-comment line (line 3,
    // 0-based 2) — otherwise the guard would be vacuous.
    assert.ok(
      first.startLine > 2,
      `the guarded error must be AFTER the non-ASCII line ` +
        `(got 0-based startLine ${first.startLine})`,
    );

    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);

    const diags = await waitForDiagnostics(
      doc.uri,
      (d) => d.some((x) => x.code === "FSM-E0100"),
    );
    const got = diags.find((d) => d.code === "FSM-E0100");
    assert.ok(got, "the FSM-E0100 diagnostic must be present");
    assertRangeMatchesOracle(got, first, "non_ascii_doc.fsm");
  });
});
