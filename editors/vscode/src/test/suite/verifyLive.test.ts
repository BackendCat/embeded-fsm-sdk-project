// W-C2 (C1-F1 + folded C1-F9) §5.4 behavioural-acceptance gate
// (Doc 31 §1 W-C2 / §5; the v1.5 C1-audit `fix-in-v1.5` headline). NOT
// symbol-presence — "the command is registered / the function exists" is
// explicitly REJECTED as acceptance (the P0-1 lesson; the CellWar R6
// "the host is the only truth" standard the C1 audit invokes). The
// sibling of `verify.test.ts`: a REAL headless VS Code Extension Host +
// the REAL `fsm-lang-server` + the REAL `fsm` CLI, EXECUTING the
// `fsm.verifyLive` command / driving the real `fsm/verify` LSP request
// path end-to-end and asserting the editor surface against the CLI's
// `fsm verify --json` output BYTE-FOR-BYTE.
//
// WHY this test exists (the C1-F1 finding): A1's CLI-spawn `fsm.verify`
// is host-tested to byte-equality by `verify.test.ts`; the A2 server seam
// is differential-tested at the Rust layer by
// `crates/fsm-lsp/tests/verify_lsp_acceptance.rs` — but NOTHING drove the
// registered `fsm.verifyLive` command / the A2 LSP-live verification path
// THROUGH A REAL EDITOR. The debounce / large-FSM-ceiling /
// degrade-to-CLI branches in `commands/verify.ts::runVerifyLive` were
// host-untested. This test closes exactly that R6 gap (Doc 31 §1 W-C2 —
// "the host verify-UX E2E that hardens A1/A2"), folding C1-F9 by driving
// the INCONCLUSIVE-not-VERIFIED honesty guard through the real A2 LSP
// seam (the data source the live command renders), not only the renderer.
//
// The differential oracle (Doc 31 §2 — IDENTICAL to verify.test.ts): the
// expected witness / verdict are NEVER hand-typed — they are recomputed
// IN-TEST by spawning the SAME real `fsm` binary on the SAME fixture,
// then byte-compared to what the live path decoded + rendered. This is
// the structural proof that the editor's live surface IS the CLI (no
// second verifier): if the LSP-embedded path recomputed anything itself,
// this equality would be coincidental, not structural.
//
// The asserted W-C2 §5.4 (a)-(e) acceptances:
//   (a) a deadlocking fixture → the live path renders the correct
//       witness list, BYTE-cross-checked against `fsm verify --json`
//       `counterexample.witness` recomputed in-test from the real binary
//       on the same fixture (the differential-oracle method).
//   (b) a clean fixture → the live path shows VERIFIED.
//   (c) a tiny-bound case → INCONCLUSIVE asserted NOT "verified" (the
//       cardinal-sin guard; folds C1-F9), driven through the REAL A2
//       `fsm/verify` LSP request (maxStates:1) so the honesty guard is
//       asserted at the live verification seam users hit, not only the
//       renderer — strictly above `verify.test.ts`'s renderer-only (c).
//   (d) the DEBOUNCE coalesces a rapid re-invocation burst on one
//       document (the supersede-token guard: earlier invocations return
//       before rendering; only the last settles, exactly once, correct).
//   (e) the LARGE-FSM CEILING short-circuits to the honest "run
//       explicitly" state within a bounded time — no LSP request, no
//       render, no editor hang.

import { execFileSync } from "child_process";
import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import * as vscode from "vscode";
import { LanguageClient } from "vscode-languageclient/node";

import { resolveRealBinaries } from "./binaries";
import { parseVerifyJson, VerifyJson } from "../../commands/verifyResult";

const FIXTURE_SRC = path.resolve(__dirname, "../fixtures");
// The W4a-verified known-good example FSMs (NOT authored here — the
// factory-audited canon, the SAME set verify.test.ts reuses):
// examples/verify/{deadlocks,clean}.fsm.
const EXAMPLES_VERIFY = path.resolve(__dirname, "../../../../../examples/verify");

/** Spawn the REAL `fsm verify --json` on `file` and return the parsed
 * envelope — the in-test differential oracle (the SAME binary the LSP
 * server embeds; a non-zero exit is a verdict 0/1/2, not a failure: the
 * JSON is still on stdout, recovered from the thrown error). IDENTICAL to
 * verify.test.ts's `cliVerify` — the same oracle, the A2 surface. */
function cliVerify(cliBinary: string, file: string, extraArgs: string[] = []): VerifyJson {
  let stdout: string;
  try {
    stdout = execFileSync(cliBinary, ["verify", "--json", ...extraArgs, file], {
      encoding: "utf8",
    });
  } catch (e) {
    const err = e as { status?: number; stdout?: string };
    if (typeof err.stdout === "string" && err.stdout.length > 0) {
      stdout = err.stdout;
    } else {
      throw new Error(`oracle: fsm verify produced no JSON: ${String(e)}`);
    }
  }
  const v = parseVerifyJson(stdout);
  assert.ok(v, "oracle: fsm verify --json must parse into an envelope");
  return v;
}

/** Poll until predicate or deadline (mirrors verify.test.ts). */
async function waitFor(predicate: () => boolean, what: string, timeoutMs = 20_000): Promise<void> {
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

/** Assert a predicate STAYS false for `windowMs` (a stability assertion —
 * used to prove the ceiling/debounce did NOT render prematurely; the
 * honest short-circuit / supersede contract). */
async function assertStaysFalse(
  predicate: () => boolean,
  what: string,
  windowMs: number,
): Promise<void> {
  const deadline = Date.now() + windowMs;
  for (;;) {
    assert.ok(!predicate(), `expected to stay false within window: ${what}`);
    if (Date.now() > deadline) {
      return;
    }
    await new Promise((r) => setTimeout(r, 50));
  }
}

/** The `fsm-verify` result virtual doc for a given fixture basename, or
 * undefined. The report URI carries the fsPath in its query
 * (`f=<encoded>`), so a per-fixture lookup is unambiguous. */
function resultDocFor(base: string): vscode.TextDocument | undefined {
  return vscode.workspace.textDocuments.find(
    (d) => d.uri.scheme === "fsm-verify" && d.uri.query.includes(encodeURIComponent(base)),
  );
}

/** Close every open `fsm-verify` result editor so a subsequent assertion
 * observes only THIS run's render (not a stale prior tab). */
async function closeAllResultDocs(): Promise<void> {
  for (const ed of vscode.window.visibleTextEditors) {
    if (ed.document.uri.scheme === "fsm-verify") {
      await vscode.window.showTextDocument(ed.document, {
        preview: false,
        preserveFocus: false,
      });
      await vscode.commands.executeCommand("workbench.action.closeActiveEditor");
    }
  }
}

interface FsmExtensionApi {
  readonly serverStarted: boolean;
}

suite("FSM Studio W-C2 (C1-F1+F9) — fsm.verifyLive Extension-Host acceptance", () => {
  let cliBinary: string;
  let serverBinary: string;
  let tmpDir: string;

  suiteSetup(async function () {
    this.timeout(600_000);

    const bins = resolveRealBinaries();
    cliBinary = bins.cli;
    serverBinary = bins.server;

    // R-15 isolation (the SAME posture as verify.test.ts / oracle.ts):
    // stage fixtures in an OS temp dir so the CLI's upward fsm.toml walk
    // finds none — the verify oracle is then allow/deny-unaware exactly
    // as the LSP server's containment boundary is.
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-v15c2-"));
    // The W4a-verified canon (copied from examples/verify — NOT
    // authored; the SAME fixtures verify.test.ts reuses). `clean_d.fsm`
    // is a SECOND copy of the W4a clean.fsm dedicated to the (d)
    // debounce test: its result doc must be observably absent BEFORE
    // that test's burst, and (b)/(c) both render the shared clean.fsm
    // (a `TextDocument` lingers in `workspace.textDocuments` after its
    // editor closes — Mocha shares one Extension Host across the
    // suite), so (d) uses its own untouched fixture for a clean
    // precondition. Same bytes ⇒ the SAME W4a-verified verdict.
    for (const f of ["deadlocks.fsm", "clean.fsm"]) {
      fs.copyFileSync(path.join(EXAMPLES_VERIFY, f), path.join(tmpDir, f));
    }
    fs.copyFileSync(path.join(EXAMPLES_VERIFY, "clean.fsm"), path.join(tmpDir, "clean_d.fsm"));
    // The reachability fixture (already under editors/vscode/ test
    // fixtures from W-A1 — reused, zero new .fsm under crates/).
    fs.copyFileSync(
      path.join(FIXTURE_SRC, "unreachable.fsm"),
      path.join(tmpDir, "unreachable.fsm"),
    );
    // A synthetic > LARGE_FSM_LINE_CEILING (4000-line) buffer for the
    // ceiling acceptance (e). NOT a verifiable model — it never reaches
    // the LSP request (the ceiling short-circuits first by line count),
    // so its content is irrelevant; padding the W4a clean.fsm with
    // comment lines keeps it a plausible `.fsm` while exceeding 4000
    // lines. Authored under the OS temp dir, NOT under crates/ (the A1
    // zero-crates-delta posture preserved; disclosed judgment call).
    const cleanSrc = fs.readFileSync(path.join(EXAMPLES_VERIFY, "clean.fsm"), "utf8");
    const padding = Array.from(
      { length: 4200 },
      (_v, i) => `// ceiling-padding line ${i + 1}`,
    ).join("\n");
    fs.writeFileSync(path.join(tmpDir, "huge.fsm"), `${padding}\n${cleanSrc}\n`);

    // Assert the R-15 invariant actually holds up-tree.
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

    // Launch the real server via fsmLang.compilerPath (Rule 1) — the
    // SAME GT-3 seam. The live path's `fsm/verify` request is served by
    // THIS real `fsm-lang-server`; its embedded `fsm_verify::verify` is
    // the IDENTICAL function `fsm` (the oracle binary) calls.
    await vscode.workspace
      .getConfiguration("fsmLang")
      .update("compilerPath", serverBinary, vscode.ConfigurationTarget.Global);

    const ext = vscode.extensions.getExtension("fsmstudio.fsm-lang");
    assert.ok(ext, "extension fsmstudio.fsm-lang must be present");
    const api = (await ext.activate()) as FsmExtensionApi;
    await waitFor(() => api.serverStarted, "language client to reach Running", 30_000);
  });

  suiteTeardown(async () => {
    await vscode.workspace
      .getConfiguration("fsmLang")
      .update("compilerPath", undefined, vscode.ConfigurationTarget.Global);
    if (tmpDir && fs.existsSync(tmpDir)) {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  // Precondition (NOT the acceptance — the acceptance is the EFFECT
  // below): the live command is reachable.
  test("fsm.verifyLive is registered", async () => {
    const all = await vscode.commands.getCommands(true);
    assert.ok(all.includes("fsm.verifyLive"), "command fsm.verifyLive must be registered");
  });

  // ── §5.4 (a): a deadlocking fixture → the live path renders the
  // correct witness list, BYTE-cross-checked vs `fsm verify --json`.
  test("(a) live deadlock witness byte-equals `fsm verify --json`", async () => {
    await closeAllResultDocs();
    const fixture = path.join(tmpDir, "deadlocks.fsm");

    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(fixture));
    await vscode.window.showTextDocument(doc);
    // Drive the REGISTERED live command end-to-end. The Uri arg is the
    // explorer-context path `resolveTargetFsm` honours first — it pins
    // the target deterministically (no activeTextEditor focus race),
    // exercising the SAME `runVerifyLive` handler a palette invocation
    // does (it then hits the LSP `fsm/verify` request path).
    await vscode.commands.executeCommand("fsm.verifyLive", vscode.Uri.file(fixture));

    await waitFor(
      () => !!resultDocFor("deadlocks.fsm"),
      "the live fsm-verify result document to open",
    );
    const resultDoc = resultDocFor("deadlocks.fsm");
    assert.ok(resultDoc, "a live fsm-verify result document must be open");
    const reportText = resultDoc.getText();

    // The independent CLI oracle — recomputed by spawning the SAME real
    // binary on the SAME fixture. The LSP server embeds the IDENTICAL
    // `fsm_verify::verify`; the live render MUST match it byte-for-byte.
    const oracle = cliVerify(cliBinary, fixture);
    const oracleWitness = oracle.properties?.deadlockFree?.counterexample?.witness;
    assert.ok(
      oracleWitness && oracleWitness.length > 0,
      "the deadlocks.fsm fixture must yield a non-empty CLI witness",
    );

    // BYTE-EQUAL: reconstruct the witness list from the report's STABLE
    // `  <n>. <event>` contract form and deep-equal it with the CLI
    // oracle (no hand-typed expectation — the CLI is the sole truth;
    // the SAME assertion shape verify.test.ts uses for A1, here proving
    // the A2 LIVE path === the CLI).
    const witnessLines = reportText
      .split("\n")
      .filter((l) => /^ {2}\d+\. \S/.test(l))
      .map((l) => l.replace(/^ {2}\d+\. /, "").trim());
    assert.deepStrictEqual(
      witnessLines,
      Array.from(oracleWitness),
      "the witness rendered by the LIVE path must BYTE-EQUAL the CLI " +
        `counterexample.witness (oracle=${JSON.stringify(
          oracleWitness,
        )}, report=${JSON.stringify(witnessLines)})`,
    );
  });

  // ── §5.4 (b): a clean fixture → the live path shows VERIFIED.
  test("(b) clean fixture → the live path shows VERIFIED", async () => {
    await closeAllResultDocs();
    const fixture = path.join(tmpDir, "clean.fsm");

    const oracle = cliVerify(cliBinary, fixture);
    assert.strictEqual(
      oracle.verdict,
      "verified",
      "clean.fsm must verify per the CLI oracle (exit 0)",
    );

    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(fixture));
    await vscode.window.showTextDocument(doc);
    await vscode.commands.executeCommand("fsm.verifyLive", vscode.Uri.file(fixture));

    await waitFor(() => {
      const d = resultDocFor("clean.fsm");
      return !!d && /VERDICT:\s+✔\s+VERIFIED/.test(d.getText());
    }, "the live result doc to show VERIFIED for clean.fsm");

    const resultDoc = resultDocFor("clean.fsm");
    assert.ok(resultDoc, "clean.fsm live result doc must be open");
    const text = resultDoc.getText();
    assert.match(
      text,
      /VERDICT:\s+✔\s+VERIFIED/,
      "the live path must show VERIFIED for a clean fixture",
    );
    assert.ok(!text.includes("INCONCLUSIVE"), "a verified live result must NOT show INCONCLUSIVE");
  });

  // ── §5.4 (c): a tiny bound → INCONCLUSIVE asserted NOT "verified"
  // (the cardinal-sin guard; folds C1-F9). Driven through the REAL A2
  // `fsm/verify` LSP request with maxStates:1 — the registered
  // `fsm.verifyLive` command always uses the default bound (the bound
  // is an LSP-param the handler does not pass, exactly as A1's
  // registered command does not pass --max-states; verify.test.ts (c)
  // is therefore renderer-only). Here we drive the REAL LIVE seam (the
  // bundled `fsm-lang-server`'s `fsm/verify`, which the live command's
  // data source IS) so the honesty guard is asserted END-TO-END through
  // the A2 verification path — strictly above the A1 renderer-only (c).
  test("(c) tiny-bound via real fsm/verify LSP request → INCONCLUSIVE, NOT verified", async () => {
    const fixture = path.join(tmpDir, "clean.fsm");

    // The CLI oracle with the SAME tiny bound: it MUST be inconclusive
    // (exit 2, verdict "inconclusive", bound hit) — the contract the
    // live LSP seam must mirror honestly.
    const oracle = cliVerify(cliBinary, fixture, ["--max-states", "1"]);
    assert.strictEqual(
      oracle.verdict,
      "inconclusive",
      "clean.fsm --max-states 1 must be INCONCLUSIVE per the CLI oracle",
    );
    assert.strictEqual(oracle.exitCode, 2, "the inconclusive CLI exit code must be 2");
    assert.strictEqual(oracle.bound?.hit, true, "the bound must be reported as hit");

    // Drive the REAL A2 `fsm/verify` LSP request — the EXACT request
    // `runVerifyLive` sends (`client.sendRequest("fsm/verify", {uri,
    // text, maxStates})`) — against the SAME bundled `fsm-lang-server`
    // binary the live command's running client talks to. The extension
    // does not export its running V1 client, so the live SEAM is
    // exercised via a short-lived `vscode-languageclient` to the
    // IDENTICAL server binary (`serverBinary`, the same `fsmLang.
    // compilerPath` the suite configured): identical server, identical
    // embedded `fsm_verify::verify` — this is the A2 verification path
    // itself (the live command's data source), NOT a renderer / a mock.
    // The differential oracle (vs `fsm verify --json --max-states 1`)
    // holds by construction (editor LIVE ≡ CLI — Doc 31 §2).
    const text = fs.readFileSync(fixture, "utf8");
    const fsmUri = vscode.Uri.file(fixture).toString();

    interface FsmVerifyLiveResult {
      readonly verifyJson: VerifyJson | null;
      readonly exitCode: number;
      readonly error?: string;
    }
    const client = new LanguageClient(
      "fsm-verifyLive-c",
      "fsm verifyLive (c) A2-seam probe",
      {
        run: { command: serverBinary },
        debug: { command: serverBinary },
      },
      { documentSelector: [{ language: "fsm-lang" }] },
    );
    await client.start();
    try {
      const result = await client.sendRequest<FsmVerifyLiveResult>("fsm/verify", {
        uri: fsmUri,
        text,
        maxStates: 1,
      });
      assert.ok(result.verifyJson, "the live fsm/verify result must carry a verifyJson envelope");
      const v = result.verifyJson;
      // THE FALSE-PROVEN GUARD, at the LIVE seam: the verdict the A2
      // path returns MUST be inconclusive, NEVER "verified".
      assert.strictEqual(
        v.verdict,
        "inconclusive",
        "THE FALSE-PROVEN GUARD: the live A2 fsm/verify on a tiny " +
          "bound MUST return INCONCLUSIVE, never a false 'verified'",
      );
      assert.notStrictEqual(
        v.verdict,
        "verified",
        "a bound-hit live verdict must NOT be 'verified'",
      );
      assert.strictEqual(
        result.exitCode,
        2,
        "the live A2 inconclusive exit code must be 2 (CLI parity)",
      );
      assert.strictEqual(v.bound?.hit, true, "the live A2 result must report the bound as hit");
      // DIFFERENTIAL ORACLE: the live A2 envelope is byte-equal to the
      // CLI `fsm verify --json --max-states 1` (editor LIVE === CLI;
      // no second verifier — the keystone-in-UI invariant, Doc 31 §2).
      assert.deepStrictEqual(
        v.verdict,
        oracle.verdict,
        "live A2 verdict must byte-equal the CLI oracle verdict",
      );
      assert.deepStrictEqual(
        v.bound?.hit,
        oracle.bound?.hit,
        "live A2 bound.hit must byte-equal the CLI oracle",
      );
    } finally {
      await client.stop();
    }
  });

  // ── §5.4 (d): the DEBOUNCE coalesces a rapid re-invocation burst on
  // one document. `runVerifyLive` uses a per-URI monotonically
  // increasing supersede token + a VERIFY_LIVE_DEBOUNCE_MS (200ms)
  // window; every invocation but the LAST returns at the
  // `inflight.get(key) !== token` guard BEFORE sending the LSP request
  // / rendering. The observable end-to-end contract: a burst does NOT
  // produce a premature render during the window (the early invocations
  // were superseded, not run), and AFTER settle exactly one correct
  // render exists (the single surviving run). This is asserted purely
  // on observable editor state (the R6 discipline — no output-channel
  // peeking, no shipped-code test seam, exactly as verify.test.ts).
  test("(d) debounce coalesces a rapid re-invocation burst", async () => {
    await closeAllResultDocs();
    // A fixture untouched by (b)/(c) — its result doc is genuinely
    // absent before the burst, so "no premature render" is an
    // unambiguous coalesce signal (a lingering shared-clean.fsm result
    // TextDocument from a prior test would otherwise mask it).
    const fixture = path.join(tmpDir, "clean_d.fsm");
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(fixture));
    await vscode.window.showTextDocument(doc);

    assert.ok(
      !resultDocFor("clean_d.fsm"),
      "precondition: no clean_d.fsm result doc before the burst",
    );

    // Fire a rapid burst WITHOUT awaiting each (the held-keybinding /
    // repeated-palette scenario the debounce guards). Eight invocations
    // dispatched well inside the 200ms window.
    const BURST = 8;
    const burst: Thenable<unknown>[] = [];
    for (let i = 0; i < BURST; i++) {
      burst.push(vscode.commands.executeCommand("fsm.verifyLive", vscode.Uri.file(fixture)));
    }

    // COALESCE PROOF, part 1: within the debounce window the result doc
    // must NOT appear. If the invocations ran un-coalesced (no
    // supersede guard), an earlier invocation would already have
    // rendered before the last settles. Assert it stays absent for most
    // of the debounce window (150ms < 200ms — a stability assertion,
    // not a race: the supersede guard MUST hold every early invocation).
    await assertStaysFalse(
      () => !!resultDocFor("clean_d.fsm"),
      "no premature render during the debounce window (early " +
        "invocations must be superseded, not rendered)",
      150,
    );

    // COALESCE PROOF, part 2: after the window + the round-trip, the
    // single surviving run renders exactly once, correctly (the CLI
    // oracle says the W4a clean.fsm bytes verify).
    await Promise.all(burst);
    await waitFor(() => {
      const d = resultDocFor("clean_d.fsm");
      return !!d && /VERDICT:\s+✔\s+VERIFIED/.test(d.getText());
    }, "the single coalesced live run to render VERIFIED");

    const matches = vscode.workspace.textDocuments.filter(
      (d) =>
        d.uri.scheme === "fsm-verify" && d.uri.query.includes(encodeURIComponent("clean_d.fsm")),
    );
    // The report URI is keyed by fsPath, so re-renders refresh in place
    // — exactly ONE result document for the fixture, never a torn /
    // duplicated / errored state from the burst.
    assert.strictEqual(
      matches.length,
      1,
      "a coalesced burst must yield exactly ONE result document " +
        `(got ${matches.length}) — the supersede guard collapsed the ` +
        "burst to a single surviving render",
    );
    const text = matches[0].getText();
    assert.match(
      text,
      /VERDICT:\s+✔\s+VERIFIED/,
      "the coalesced run must render the correct (VERIFIED) verdict",
    );
    assert.ok(
      !text.includes("request failed") && !text.includes("INCONCLUSIVE"),
      "the coalesced burst must NOT leave a torn / errored render",
    );
  });

  // ── §5.4 (e): the LARGE-FSM CEILING short-circuits to the honest
  // "run explicitly" state within a bounded time. `runVerifyLive`
  // checks `doc.lineCount > LARGE_FSM_LINE_CEILING` (4000) and returns
  // BEFORE any LSP request — no render, no editor hang. The observable
  // end-to-end contract: the command returns FAST (far below a real
  // verify round-trip) and produces NO result doc for the huge fixture.
  test("(e) large-FSM ceiling short-circuits fast, no render, no hang", async () => {
    await closeAllResultDocs();
    const fixture = path.join(tmpDir, "huge.fsm");
    const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(fixture));
    assert.ok(
      doc.lineCount > 4000,
      `the ceiling fixture must exceed 4000 lines (got ${doc.lineCount})`,
    );
    await vscode.window.showTextDocument(doc);

    // The command must RETURN quickly — the ceiling short-circuits
    // before the LSP round-trip + debounce. A real live verify would
    // take well over the debounce (200ms) + a server round-trip; the
    // ceiling path does neither. Bound it generously at 2s (far below a
    // real round-trip, far above any plausible short-circuit cost) so
    // this asserts "did not hang / did not run the verify", not a tight
    // micro-benchmark (no flakiness on a slow CI box).
    const t0 = Date.now();
    await vscode.commands.executeCommand("fsm.verifyLive", vscode.Uri.file(fixture));
    const elapsed = Date.now() - t0;
    assert.ok(
      elapsed < 2000,
      "the large-FSM ceiling must short-circuit FAST (no LSP " +
        `round-trip, no hang); took ${elapsed}ms`,
    );

    // And it must NOT have rendered a result doc (the honest
    // short-circuit — it points the user at the explicit CLI-spawn
    // command instead; it does NOT churn the editor with a render).
    // Assert it stays absent for a window comfortably exceeding the
    // debounce + a round-trip — proving no deferred render fires.
    await assertStaysFalse(
      () => !!resultDocFor("huge.fsm"),
      "the large-FSM ceiling must NOT produce a result document " +
        "(honest short-circuit, never a churned render)",
      1500,
    );
  });
});
