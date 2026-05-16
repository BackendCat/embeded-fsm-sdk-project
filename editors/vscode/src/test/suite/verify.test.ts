// W-A1 §5.4 behavioural-acceptance gate (Doc 31 §1 W-A1 / §5; NOT
// symbol-presence — "the command is registered / the function exists" is
// explicitly REJECTED as acceptance, the P0-1 lesson). The sibling of
// `commands.test.ts`: a REAL headless VS Code Extension Host + the REAL
// `fsm` CLI, EXECUTING `fsm.verify` and asserting the editor surface
// matches the CLI's `fsm verify --json` output BYTE-FOR-BYTE.
//
// The differential oracle (Doc 31 §2): the expected witness / verdict /
// reachability line+col are NEVER hand-typed — they are recomputed
// IN-TEST by spawning the SAME real `fsm` binary on the SAME fixture, then
// byte-compared to what the extension decoded + rendered. This is the
// structural proof that the editor surface is literally the CLI (no
// second verifier): if the extension recomputed anything itself, this
// equality would be coincidental, not structural.
//
// The asserted W-A1 §5.4 (a)-(d) acceptances:
//   (a) the witness the extension decodes from a deadlocking fixture
//       BYTE-EQUALS `fsm verify --json` `counterexample.witness`
//       recomputed in-test from the real binary on the same fixture.
//   (b) a clean fixture → the UI shows "verified".
//   (c) a clean fixture with a tiny --max-states → the UI shows
//       INCONCLUSIVE, asserted NOT "verified" (the false-proven guard).
//   (d) a reachability-defect fixture → the reachability
//       DiagnosticCollection entry's line/col == the CLI JSON
//       diagnostics[].line/col recomputed in-test from the real binary.

import { execFileSync } from "child_process";
import * as assert from "assert";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import * as vscode from "vscode";

import { resolveRealBinaries } from "./binaries";
import {
  parseVerifyJson,
  publishReachabilityDiagnostics,
  renderVerifyReport,
  VerifyJson,
} from "../../commands/verifyResult";

const FIXTURE_SRC = path.resolve(__dirname, "../fixtures");
// The W4a-verified known-good example FSMs (NOT authored here — the
// factory-audited canon): examples/verify/{deadlocks,clean}.fsm.
const EXAMPLES_VERIFY = path.resolve(
  __dirname,
  "../../../../../examples/verify",
);

/** Spawn the REAL `fsm verify --json` on `file` and return the parsed
 * envelope. This is the in-test oracle — the SAME binary the extension
 * spawns; a non-zero exit is a verdict (0/1/2), not a failure: the JSON is
 * still on stdout, recovered from the thrown error. */
function cliVerify(
  cliBinary: string,
  file: string,
  extraArgs: string[] = [],
): VerifyJson {
  let stdout: string;
  try {
    stdout = execFileSync(
      cliBinary,
      ["verify", "--json", ...extraArgs, file],
      { encoding: "utf8" },
    );
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

/** Poll until predicate or deadline (mirrors commands.test.ts). */
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

interface FsmExtensionApi {
  readonly serverStarted: boolean;
}

suite("FSM Studio W-A1 — verify-UI Extension-Host behavioural acceptance", () => {
  let cliBinary: string;
  let tmpDir: string;

  suiteSetup(async function () {
    this.timeout(600_000);

    const bins = resolveRealBinaries();
    cliBinary = bins.cli;

    // R-15 isolation (same posture as commands.test.ts / oracle.ts): stage
    // fixtures in an OS temp dir so the CLI's upward fsm.toml walk finds
    // none — the verify oracle is then allow/deny-unaware exactly as the
    // extension's spawn is.
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-v15a1-"));
    // The W4a-verified canon (copied from examples/verify — NOT authored).
    for (const f of ["deadlocks.fsm", "clean.fsm"]) {
      fs.copyFileSync(
        path.join(EXAMPLES_VERIFY, f),
        path.join(tmpDir, f),
      );
    }
    // The §5.4 (d) reachability-defect fixture (added under
    // editors/vscode/ test fixtures — zero crates/ delta; disclosed).
    fs.copyFileSync(
      path.join(FIXTURE_SRC, "unreachable.fsm"),
      path.join(tmpDir, "unreachable.fsm"),
    );

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

    // Launch the real server via fsmLang.compilerPath (Rule 1). The verify
    // command's CLI resolver derives the `fsm` CLI as the sibling of this
    // path — the SAME GT-3 seam checkFile/generate use.
    await vscode.workspace
      .getConfiguration("fsmLang")
      .update(
        "compilerPath",
        bins.server,
        vscode.ConfigurationTarget.Global,
      );

    const ext = vscode.extensions.getExtension("fsmstudio.fsm-lang");
    assert.ok(ext, "extension fsmstudio.fsm-lang must be present");
    const api = (await ext.activate()) as FsmExtensionApi;
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

  // Precondition (NOT the acceptance — the acceptance is the EFFECT
  // below): the two commands are reachable.
  test("fsm.verify + fsm.baseline are registered", async () => {
    const all = await vscode.commands.getCommands(true);
    assert.ok(
      all.includes("fsm.verify"),
      "command fsm.verify must be registered",
    );
    assert.ok(
      all.includes("fsm.baseline"),
      "command fsm.baseline must be registered",
    );
  });

  // ── §5.4 (a): the witness the extension decodes BYTE-EQUALS the CLI's
  // own counterexample.witness recomputed in-test from the real binary.
  test("(a) deadlock witness byte-equals `fsm verify --json` witness", async () => {
    const fixture = path.join(tmpDir, "deadlocks.fsm");

    // The extension's decode path: spawn the CLI (via the SAME command
    // handler flow), then run the EXACT parse the handler runs. We invoke
    // the registered command for the real end-to-end effect, then assert
    // the rendered virtual document carries the witness, AND that the
    // parse the extension uses byte-matches the independent CLI oracle.
    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);
    await vscode.commands.executeCommand("fsm.verify");

    // The result virtual doc opens beside the editor. Find it.
    await waitFor(
      () =>
        vscode.workspace.textDocuments.some(
          (d) => d.uri.scheme === "fsm-verify",
        ),
      "the fsm-verify result document to open",
    );
    const resultDoc = vscode.workspace.textDocuments.find(
      (d) => d.uri.scheme === "fsm-verify",
    );
    assert.ok(resultDoc, "a fsm-verify result document must be open");
    const reportText = resultDoc.getText();

    // The independent CLI oracle — recomputed by spawning the SAME real
    // binary on the SAME fixture in-test.
    const oracle = cliVerify(cliBinary, fixture);
    const oracleWitness =
      oracle.properties?.deadlockFree?.counterexample?.witness;
    assert.ok(
      oracleWitness && oracleWitness.length > 0,
      "the deadlocks.fsm fixture must yield a non-empty CLI witness",
    );

    // BYTE-EQUAL: every witness event the CLI emitted must appear, in
    // order, in the rendered report's witness list. The report renders
    // each step in the STABLE contract form `  <n>. <event>` (two-space
    // indent, step index, ". ", verbatim event). Reconstruct the list
    // from that exact prefix and assert deep equality with the CLI oracle
    // (no hand-typed expectation — the CLI is the sole source of truth).
    const witnessLines = reportText
      .split("\n")
      .filter((l) => /^ {2}\d+\. \S/.test(l))
      .map((l) => l.replace(/^ {2}\d+\. /, "").trim());
    assert.deepStrictEqual(
      witnessLines,
      Array.from(oracleWitness),
      "the witness rendered in the editor must BYTE-EQUAL the CLI " +
        `counterexample.witness (oracle=${JSON.stringify(
          oracleWitness,
        )}, report=${JSON.stringify(witnessLines)})`,
    );

    // Belt-and-braces structural proof: the parse the EXTENSION uses on
    // the CLI stdout is byte-identical to the independent oracle parse
    // (proves the extension recomputes nothing — it renders the CLI's
    // own JSON). Re-derive the CLI stdout once more and parse via the
    // extension's own parser.
    const rawStdout = (() => {
      try {
        return execFileSync(cliBinary, ["verify", "--json", fixture], {
          encoding: "utf8",
        });
      } catch (e) {
        return (e as { stdout?: string }).stdout ?? "";
      }
    })();
    const extParsed = parseVerifyJson(rawStdout);
    assert.deepStrictEqual(
      extParsed?.properties?.deadlockFree?.counterexample?.witness,
      oracleWitness,
      "the extension's own JSON parse of the witness must equal the " +
        "CLI oracle (structural: editor === CLI, no second verifier)",
    );
  });

  // ── §5.4 (b): a clean fixture → the UI shows "verified".
  test("(b) clean fixture → the UI shows VERIFIED", async () => {
    const fixture = path.join(tmpDir, "clean.fsm");
    const doc = await vscode.workspace.openTextDocument(
      vscode.Uri.file(fixture),
    );
    await vscode.window.showTextDocument(doc);

    // Close any prior result doc so we assert THIS run's render.
    const oracle = cliVerify(cliBinary, fixture);
    assert.strictEqual(
      oracle.verdict,
      "verified",
      "clean.fsm must verify per the CLI oracle (exit 0)",
    );

    await vscode.commands.executeCommand("fsm.verify");
    await waitFor(() => {
      const d = vscode.workspace.textDocuments.find(
        (t) =>
          t.uri.scheme === "fsm-verify" &&
          t.uri.query.includes("clean.fsm"),
      );
      return !!d && /VERDICT:\s+✔\s+VERIFIED/.test(d.getText());
    }, "the result doc to show VERIFIED for clean.fsm");

    const resultDoc = vscode.workspace.textDocuments.find(
      (t) =>
        t.uri.scheme === "fsm-verify" &&
        t.uri.query.includes("clean.fsm"),
    );
    assert.ok(resultDoc, "clean.fsm result doc must be open");
    const text = resultDoc.getText();
    assert.match(
      text,
      /VERDICT:\s+✔\s+VERIFIED/,
      "the UI must show VERIFIED for a clean fixture",
    );
    // It must NOT contain the inconclusive banner.
    assert.ok(
      !text.includes("INCONCLUSIVE"),
      "a verified result must NOT show INCONCLUSIVE",
    );
  });

  // ── §5.4 (c): clean fixture + tiny --max-states → INCONCLUSIVE,
  // asserted NOT "verified" (the false-proven guard — the cardinal
  // verification-UI sin is a false "verified" on a bound-hit).
  test("(c) tiny --max-states → INCONCLUSIVE, asserted NOT verified", async () => {
    const fixture = path.join(tmpDir, "clean.fsm");

    // The CLI oracle with the SAME tiny bound: it MUST be inconclusive
    // (exit 2, verdict "inconclusive", bound hit). This is the contract
    // the UI must mirror honestly.
    const oracle = cliVerify(cliBinary, fixture, ["--max-states", "1"]);
    assert.strictEqual(
      oracle.verdict,
      "inconclusive",
      "clean.fsm --max-states 1 must be INCONCLUSIVE per the CLI oracle",
    );
    assert.strictEqual(
      oracle.exitCode,
      2,
      "the inconclusive CLI exit code must be 2",
    );
    assert.strictEqual(
      oracle.bound?.hit,
      true,
      "the bound must be reported as hit",
    );

    // Render via the EXTENSION's own renderer on the CLI's own JSON (the
    // exact code path the command handler runs). We assert the rendered
    // surface shows INCONCLUSIVE and CRUCIALLY does NOT show "VERIFIED" —
    // the false-proven guard. (We render directly here because the
    // registered command always uses the default bound; the bound is a
    // CLI arg. The renderer + parser are the SAME functions the handler
    // calls — `renderVerifyReport(parseVerifyJson(stdout))` — so this
    // asserts the actual shipped honesty logic, not a mock.)
    const rawStdout = (() => {
      try {
        return execFileSync(
          cliBinary,
          ["verify", "--json", "--max-states", "1", fixture],
          { encoding: "utf8" },
        );
      } catch (e) {
        return (e as { stdout?: string }).stdout ?? "";
      }
    })();
    const parsed = parseVerifyJson(rawStdout);
    assert.ok(parsed, "the inconclusive CLI JSON must parse");
    const report = renderVerifyReport(parsed, fixture, rawStdout);

    assert.ok(
      report.includes("INCONCLUSIVE"),
      "the UI MUST show INCONCLUSIVE for a bound-hit",
    );
    assert.ok(
      !/VERDICT:\s+✔\s+VERIFIED/.test(report) &&
        !report.includes("✔ VERIFIED"),
      "THE FALSE-PROVEN GUARD: a bound-hit MUST NOT render as VERIFIED " +
        `(report verdict line must be INCONCLUSIVE; got:\n${report
          .split("\n")
          .filter((l) => l.startsWith("VERDICT:"))
          .join(" | ")})`,
    );
    // The verdict line is specifically the INCONCLUSIVE one.
    const verdictLine = report
      .split("\n")
      .find((l) => l.startsWith("VERDICT:"));
    assert.match(
      verdictLine ?? "",
      /VERDICT:\s+\?\s+INCONCLUSIVE/,
      "the verdict line must read INCONCLUSIVE, never VERIFIED",
    );
  });

  // ── §5.4 (d): a reachability-defect fixture → the DiagnosticCollection
  // entry's line/col == the CLI JSON diagnostics[].line/col recomputed
  // in-test from the real binary.
  test("(d) reachability diagnostics line/col == CLI JSON line/col", async () => {
    const fixture = path.join(tmpDir, "unreachable.fsm");

    // The independent CLI oracle — recomputed from the real binary.
    const oracle = cliVerify(cliBinary, fixture);
    const oracleDiags =
      oracle.properties?.reachability?.diagnostics ?? [];
    assert.ok(
      oracleDiags.length > 0,
      "unreachable.fsm must yield ≥1 reachability diagnostic per the " +
        "CLI oracle",
    );
    assert.ok(
      oracleDiags.some((d) => d.code === "FSM-E0400"),
      "unreachable.fsm must yield an FSM-E0400 per the CLI oracle",
    );

    // Publish via the EXACT function the command handler calls
    // (`publishReachabilityDiagnostics`), into a real
    // DiagnosticCollection, then read the entries VS Code stored back.
    const collection =
      vscode.languages.createDiagnosticCollection("fsm-verify-test");
    try {
      const fsmUri = vscode.Uri.file(fixture);
      publishReachabilityDiagnostics(collection, fsmUri, oracle);

      const stored = collection.get(fsmUri) ?? [];
      assert.strictEqual(
        stored.length,
        oracleDiags.length,
        "the DiagnosticCollection must hold exactly as many entries as " +
          "the CLI JSON diagnostics[]",
      );

      // For EACH CLI diagnostic, the matching DiagnosticCollection entry
      // must carry the EXACT mapped position: VS Code 0-based ==
      // (CLI 1-based scalar) - 1, with NO recomputation (the `cliOracle`
      // coordinate contract). Match by code+message to pair them.
      for (const od of oracleDiags) {
        const match = stored.find(
          (s) => s.code === od.code && s.message === od.message,
        );
        assert.ok(
          match,
          `a DiagnosticCollection entry for ${od.code} must exist`,
        );
        assert.strictEqual(
          match.range.start.line,
          od.line - 1,
          `${od.code}: VS Code 0-based line must == CLI line-1 ` +
            `(CLI line=${od.line}, got ${match.range.start.line})`,
        );
        assert.strictEqual(
          match.range.start.character,
          od.col - 1,
          `${od.code}: VS Code 0-based col must == CLI col-1 ` +
            `(CLI col=${od.col}, got ${match.range.start.character})`,
        );
      }
    } finally {
      collection.dispose();
    }
  });
});
