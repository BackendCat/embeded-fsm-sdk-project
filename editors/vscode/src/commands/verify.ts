// `fsm.verify` / `fsm.baseline` — Doc 31 §1 W-A1: surface the SHIPPED v1.4
// verification in VS Code by SPAWNING the same `fsm` binary the CI/factory
// runs (the proven `cliRunner.ts` / `cliBinary.ts` seam — the IDENTICAL
// pattern `fsm.checkFile` / `fsm.generateC99` use) and rendering its
// `--json` output.
//
// THE CARDINAL INVARIANT (Doc 31 §2 — read it; this is THE rule): there is
// **NO** verification / reachability / deadlock / transition-selection /
// guard-eval logic in this handler. It does EXACTLY four things:
//   1. resolve the target `.fsm` (the shared `resolveTargetFsm` helper);
//   2. resolve the `fsm` CLI via the EXISTING `resolveCliBinary` (no new
//      binary-resolution machinery — the GT-3 seam, reused verbatim);
//   3. `runCli(cli.command, ["verify"|"baseline", …, "--json"])` (the
//      EXISTING `runCli` — no new spawn machinery);
//   4. parse the `fsm-verify/v1` / `fsm-trace-diff/v1` JSON envelope and
//      render it (`verifyResult.ts`).
// A1 holds the keystone-in-UI invariant (Doc 31 §2) TRIVIALLY: it is
// literally the CLI binary the CI calls. We never re-implement, re-derive,
// or "recompute from the witness" anything — the `verdict` the CLI emits
// is the verdict shown; the bound stats / witness / reachability line+col
// are read VERBATIM from the JSON. A second verifier (even partial, even
// "just parsing the witness ourselves") is the exact P0-1 / v1.4-keystone
// regression this epic guards against.
//
// HONESTY (Doc 31 §1 W-A1 (3); the schema's "exit-2 = explicitly NOT
// verified"): the CLI verify exit codes are a VERDICT contract, NOT a
// command-failure signal — 0 verified / 1 property-violated / 2
// INCONCLUSIVE / 3 IO / 4 not-analyzable. We surface 0/1/2 as the CLI's
// own verdict (the report renders INCONCLUSIVE as inconclusive, NEVER as
// "verified" — the cardinal verification-UI sin is a false "verified" on a
// bound-hit). Only 3/4 and a genuine spawn failure are surfaced as a
// command problem; even then the CLI's own stderr is shown verbatim (the
// cardinal-sin bar — never a fabricated clean verdict).

import * as path from "path";

import * as vscode from "vscode";
import { State } from "vscode-languageclient/node";

import { resolveTargetFsm } from "./activeFsm";
import { CommandDeps } from "./index";
import { noCliBinaryMessage, resolveCliBinary } from "./cliBinary";
import { runCli } from "./cliRunner";
import { error, info, infoWithLog, warn, warnWithLog } from "./notify";
import {
  parseBaselineJson,
  parseVerifyJson,
  publishReachabilityDiagnostics,
  renderBaselineReport,
  renderVerifyReport,
  verdictLabel,
  VerifyJson,
  VerifyResultDocProvider,
} from "./verifyResult";

// ── v1.5 W-A2 — the LSP-LIVE verify surface (Doc 31 §1 W-A2 / §2 the
// keystone-in-UI invariant). This COEXISTS with A1's CLI-spawn surface:
// both are honest frontends of the SAME `fsm-verify` — A1 spawns the `fsm`
// binary the CI runs, A2 round-trips the `fsm/verify` LSP request that
// calls the IDENTICAL `fsm_verify::{verify,reachability_diagnostics}` the
// CLI calls (server-side; see `crates/fsm-lsp/src/capabilities/verify.rs`).
// A2 ADDS this surface; it does NOT fork A1. The verdict/witness/
// reachability are read VERBATIM from the LSP result (the SAME
// `fsm-verify/v1` envelope A1 renders), reusing `verifyResult.ts` — there
// is NO verification logic in this file (the cardinal-sin bar; a second
// verifier — even a "fast in-editor" one — is the P0-1/v1.4-keystone
// regression the epic guards against).

/** The `fsm/verify` LSP request shape (Doc 31 §1 W-A2). Server contract in
 * `crates/fsm-lsp/src/capabilities/verify.rs::VerifyRequestParams`. */
interface FsmVerifyLiveParams {
  readonly uri: string;
  readonly text: string;
  readonly machine?: string;
  readonly maxStates?: number;
  readonly maxSteps?: number;
}

/** The `fsm/verify` LSP response. `verifyJson` is the `fsm-verify/v1`
 * object byte-equal to `fsm verify --json` (the differential oracle);
 * `null` iff the model did not analyse (then `error` carries the honest
 * exit-4 reason). `exitCode` is the CLI's own verdict bucket. */
interface FsmVerifyLiveResult {
  readonly verifyJson: VerifyJson | null;
  readonly exitCode: number;
  readonly error?: string;
}

/**
 * The client-side LARGE-FSM CEILING (Doc 31 §1 W-A2 (2)). `fsm-verify` is
 * itself bounded-by-construction (it returns INCONCLUSIVE on a hit bound,
 * never a hang), but auto/explicit verify of a very large buffer would
 * still be a poor in-editor round-trip. Above this line count the live
 * command short-circuits to an HONEST "not auto-verified — run the
 * CLI-spawn `FSM Studio: Verify` (`fsm.verify`) explicitly" state rather
 * than churning. This is a *trigger* guard (the verifier is unchanged);
 * the threshold is deliberately generous (a realistic embedded `.fsm` is
 * tens–low-hundreds of lines; 4000 is far past hand-authored size yet
 * still bounds the worst in-editor latency).
 */
const LARGE_FSM_LINE_CEILING = 4000;

/**
 * The client-side DEBOUNCE for the live request (Doc 31 §1 W-A2 (2); the
 * SAME ~200ms posture the LSP's own `publishDiagnostics` debounce uses,
 * server.rs DEBOUNCE). The command is *explicitly triggered*, never
 * auto-on-keystroke; the debounce additionally collapses a rapid
 * re-invocation burst (e.g. a held keybinding / repeated palette runs) to
 * one in-flight verify per document so the server is not flooded.
 */
const VERIFY_LIVE_DEBOUNCE_MS = 200;

/** The verify-report doc provider + the reachability diagnostics
 * collection, owned by the extension and passed to the handlers. Created
 * once in `extension.ts` (the additive call site — see `registerVerify`). */
export interface VerifyDeps {
  readonly resultDocs: VerifyResultDocProvider;
  readonly reachDiagnostics: vscode.DiagnosticCollection;
}

/** Build the virtual-document URI for a given `.fsm`'s verify report. The
 * path component carries the file basename so the tab title is readable;
 * the query carries the full fsPath so two files never collide. */
function reportUri(fsmPath: string, kind: "verify" | "baseline"): vscode.Uri {
  const base = path.basename(fsmPath);
  return vscode.Uri.from({
    scheme: VerifyResultDocProvider.scheme,
    path: `/${kind}/${base}.txt`,
    query: `f=${encodeURIComponent(fsmPath)}&k=${kind}`,
  });
}

/** Open (or refresh) the read-only result document beside the editor. */
async function showReport(
  deps: VerifyDeps,
  uri: vscode.Uri,
  body: string,
): Promise<void> {
  deps.resultDocs.set(uri, body);
  const doc = await vscode.workspace.openTextDocument(uri);
  await vscode.languages.setTextDocumentLanguage(doc, "plaintext");
  await vscode.window.showTextDocument(doc, {
    viewColumn: vscode.ViewColumn.Beside,
    preview: true,
    preserveFocus: false,
  });
}

async function runVerify(
  cmdDeps: CommandDeps,
  vDeps: VerifyDeps,
  arg: unknown,
): Promise<void> {
  const fsmPath = resolveTargetFsm(arg);
  if (!fsmPath) {
    warn("FSM Studio: open a .fsm file to verify.");
    return;
  }

  const cli = resolveCliBinary(
    cmdDeps.extensionPath,
    vscode.workspace.getConfiguration("fsmLang"),
  );
  if (!cli) {
    // No silent no-op — the verbatim missing-CLI message (the cardinal-sin
    // bar at the command boundary; identical to checkFile/generate).
    error(noCliBinaryMessage(`${process.platform}-${process.arch}`));
    return;
  }

  const args = ["verify", "--json", fsmPath];
  cmdDeps.outputChannel.appendLine(
    `[fsm] fsm.verify: ${cli.command} ${args.join(" ")}`,
  );
  const res = await runCli(cli.command, args, {
    cwd: path.dirname(fsmPath),
  });

  if (res.spawnError) {
    cmdDeps.outputChannel.appendLine(
      `[fsm] verify could not spawn the CLI: ${res.stderr}`,
    );
    error(`FSM Studio: could not run fsm verify — ${res.stderr.trim()}`);
    return;
  }

  // Echo the CLI's raw streams into the log (the same pipeline the CI
  // runs — full transparency).
  if (res.stdout.trim().length > 0) {
    cmdDeps.outputChannel.appendLine(res.stdout.trimEnd());
  }
  if (res.stderr.trim().length > 0) {
    cmdDeps.outputChannel.appendLine(res.stderr.trimEnd());
  }

  // Exit 3 (IO) / 4 (not-analyzable) are NOT a verdict — surface the
  // CLI's own error verbatim, render no fake summary (the cardinal-sin
  // bar). Exit 0/1/2 ARE verdicts (verified / property-violated /
  // INCONCLUSIVE) and flow to the honest render below.
  if (res.code === 3 || res.code === 4) {
    const detail =
      res.stderr.trim().split("\n")[0] ||
      `fsm verify exited with code ${res.code}`;
    error(`FSM Studio: cannot verify — ${detail}`);
    return;
  }

  const parsed = parseVerifyJson(res.stdout);
  const body = renderVerifyReport(parsed ?? {}, fsmPath, res.stdout);
  const uri = reportUri(fsmPath, "verify");
  await showReport(vDeps, uri, body);

  // Reachability FSM-E0400/FSM-W0602 → the dedicated DiagnosticCollection
  // (line/col straight from the JSON — VS Code's native click→source).
  if (parsed) {
    publishReachabilityDiagnostics(
      vDeps.reachDiagnostics,
      vscode.Uri.file(fsmPath),
      parsed,
    );
  }

  // A non-blocking toast mirroring the CLI's OWN verdict (never a verdict
  // we re-derived). INCONCLUSIVE is announced as inconclusive — never as a
  // pass (Doc 31 §1 W-A1 (3)).
  if (parsed && parsed.schema?.startsWith("fsm-verify/")) {
    const { word } = verdictLabel(parsed);
    if (parsed.verdict === "verified") {
      info(`FSM Studio: verification — ${word}.`);
    } else if (parsed.verdict === "inconclusive") {
      warnWithLog(
        `FSM Studio: verification — ${word} (a bound was hit; ` +
          "NOT a proof). See the result view.",
        cmdDeps.outputChannel,
      );
    } else {
      warnWithLog(
        `FSM Studio: verification — ${word}. See the result view.`,
        cmdDeps.outputChannel,
      );
    }
  } else {
    // The CLI ran but did not emit the expected envelope — the report
    // shows the raw output verbatim; the toast says so honestly.
    warnWithLog(
      "FSM Studio: fsm verify produced no fsm-verify/v1 JSON — see " +
        "the result view for the raw CLI output.",
      cmdDeps.outputChannel,
    );
  }
}

async function runBaseline(
  cmdDeps: CommandDeps,
  vDeps: VerifyDeps,
  arg: unknown,
): Promise<void> {
  const fsmPath = resolveTargetFsm(arg);
  if (!fsmPath) {
    warn("FSM Studio: open a .fsm file to run a baseline check.");
    return;
  }

  const cli = resolveCliBinary(
    cmdDeps.extensionPath,
    vscode.workspace.getConfiguration("fsmLang"),
  );
  if (!cli) {
    error(noCliBinaryMessage(`${process.platform}-${process.arch}`));
    return;
  }

  // `fsm baseline` walks a SUITE directory; from a single .fsm the
  // sensible suite is its containing directory (the CLI walks it
  // recursively). Default mode (no --record/--check) is a drift CHECK
  // against the corpus — the read-only, factory-equivalent invocation.
  const suiteDir = path.dirname(fsmPath);
  const args = ["baseline", "--json", suiteDir];
  cmdDeps.outputChannel.appendLine(
    `[fsm] fsm.baseline: ${cli.command} ${args.join(" ")}`,
  );
  const res = await runCli(cli.command, args, { cwd: suiteDir });

  if (res.spawnError) {
    cmdDeps.outputChannel.appendLine(
      `[fsm] baseline could not spawn the CLI: ${res.stderr}`,
    );
    error(`FSM Studio: could not run fsm baseline — ${res.stderr.trim()}`);
    return;
  }
  if (res.stdout.trim().length > 0) {
    cmdDeps.outputChannel.appendLine(res.stdout.trimEnd());
  }
  if (res.stderr.trim().length > 0) {
    cmdDeps.outputChannel.appendLine(res.stderr.trimEnd());
  }

  // Mirror the verify exit-code discipline: 3 (IO) / 4 (out-of-scope)
  // are not a verdict — surface verbatim, no fake summary. 0/1/2 are
  // verdicts (no-drift / drift / INCONCLUSIVE) and render honestly.
  if (res.code === 3 || res.code === 4) {
    const detail =
      res.stderr.trim().split("\n")[0] ||
      `fsm baseline exited with code ${res.code}`;
    error(`FSM Studio: baseline could not run — ${detail}`);
    return;
  }

  const parsed = parseBaselineJson(res.stdout);
  const body = renderBaselineReport(parsed ?? {}, res.stdout);
  const uri = reportUri(fsmPath, "baseline");
  await showReport(vDeps, uri, body);

  if (parsed && parsed.schema?.startsWith("fsm-trace-diff/")) {
    if (parsed.verdict === "no-drift") {
      infoWithLog(
        "FSM Studio: baseline — NO-DRIFT.",
        cmdDeps.outputChannel,
      );
    } else if (parsed.verdict === "inconclusive") {
      warnWithLog(
        "FSM Studio: baseline — INCONCLUSIVE (corpus absent/unreadable; " +
          "NOT a clean no-drift). See the result view.",
        cmdDeps.outputChannel,
      );
    } else {
      warnWithLog(
        `FSM Studio: baseline — ${(parsed.verdict ?? "?").toUpperCase()}. ` +
          "See the result view.",
        cmdDeps.outputChannel,
      );
    }
  } else {
    warnWithLog(
      "FSM Studio: fsm baseline produced no fsm-trace-diff/v1 JSON — " +
        "see the result view for the raw CLI output.",
      cmdDeps.outputChannel,
    );
  }
}

/**
 * `fsm.verifyLive` — verify via the LANGUAGE SERVER (Doc 31 §1 W-A2 / §2).
 *
 * Flow (the cardinal-sin bar: NO verification logic here — every datum is
 * the LSP result, the SAME `fsm-verify/v1` envelope A1 renders from the
 * CLI; the server-side `fsm/verify` calls the IDENTICAL
 * `fsm_verify::{verify,reachability_diagnostics}` the CLI calls):
 *
 *  1. resolve the active `.fsm` editor document (its IN-MEMORY buffer is
 *     the authoritative content — the live advantage over A1's on-disk
 *     CLI-spawn);
 *  2. the LARGE-FSM CEILING — above `LARGE_FSM_LINE_CEILING` lines,
 *     short-circuit to an HONEST "run `fsm.verify` (CLI-spawn) explicitly"
 *     message (never silently churn the editor — Doc 31 §1 W-A2 (2));
 *  3. require the language client `State.Running` (honest degrade
 *     otherwise — never a silent no-op, never a fabricated verdict);
 *  4. the DEBOUNCE — collapse a rapid re-invocation burst per document to
 *     one in-flight `fsm/verify` (the trigger is explicit, never
 *     auto-on-keystroke; the debounce just prevents a flood);
 *  5. `client.sendRequest("fsm/verify", { uri, text })` and RENDER the
 *     result through `verifyResult.ts` (the SAME virtual-doc +
 *     reachability `DiagnosticCollection` A1 uses — one surface, two
 *     honest frontends). INCONCLUSIVE renders as inconclusive, NEVER as
 *     "verified"; a not-analyzable model (`verifyJson:null`, exit 4) shows
 *     the honest CLI-parity reason, never a fake clean verdict.
 */
async function runVerifyLive(
  cmdDeps: CommandDeps,
  vDeps: VerifyDeps,
  inflight: Map<string, number>,
  arg: unknown,
): Promise<void> {
  const fsmPath = resolveTargetFsm(arg);
  if (!fsmPath) {
    warn("FSM Studio: open a .fsm file to verify (live).");
    return;
  }

  // The authoritative content is the IN-EDITOR buffer (the live edge over
  // A1's on-disk CLI-spawn). Find the open document for this path; fall
  // back to reading it if it is not open in an editor.
  let doc = vscode.workspace.textDocuments.find(
    (d) => d.uri.fsPath === fsmPath,
  );
  if (!doc) {
    try {
      doc = await vscode.workspace.openTextDocument(vscode.Uri.file(fsmPath));
    } catch {
      error(`FSM Studio: could not open ${fsmPath} for live verification.`);
      return;
    }
  }
  const text = doc.getText();

  // (2) THE large-FSM ceiling — an honest short-circuit, NOT churn. The
  // verifier is bounded anyway; this guards the in-editor round-trip UX.
  if (doc.lineCount > LARGE_FSM_LINE_CEILING) {
    warnWithLog(
      `FSM Studio: this model is large (${doc.lineCount} lines > ` +
        `${LARGE_FSM_LINE_CEILING}); live verification is not run ` +
        "automatically to keep the editor responsive. Run " +
        "`FSM Studio: Verify` (the CLI-spawn command) explicitly for a " +
        "full bounded verification.",
      cmdDeps.outputChannel,
    );
    return;
  }

  // (3) the language client must be running (the LSP is the data source —
  // honest degrade, never a silent no-op or a fabricated verdict).
  const client = cmdDeps.getClient();
  if (!client || client.state !== State.Running) {
    error(
      "FSM Studio: the language server is not running — live verification " +
        "is unavailable. Use `FSM Studio: Verify` (CLI-spawn) instead, or " +
        "restart the server (`FSM Studio: Restart Language Server`).",
    );
    return;
  }

  // (4) the debounce — collapse a burst per document. A monotonically
  // increasing token per URI; only the latest run renders (a superseded
  // run is silently dropped — exactly the LSP's own generation-counter
  // debounce shape, server.rs `schedule_analyze`).
  const key = doc.uri.toString();
  const token = (inflight.get(key) ?? 0) + 1;
  inflight.set(key, token);
  await new Promise((r) => setTimeout(r, VERIFY_LIVE_DEBOUNCE_MS));
  if (inflight.get(key) !== token) {
    // A newer invocation superseded this one — it owns the render.
    return;
  }

  const params: FsmVerifyLiveParams = { uri: doc.uri.toString(), text };
  cmdDeps.outputChannel.appendLine(
    `[fsm] fsm.verifyLive: LSP request fsm/verify ${doc.uri.toString()}`,
  );

  let result: FsmVerifyLiveResult;
  try {
    result = await client.sendRequest<FsmVerifyLiveResult>(
      "fsm/verify",
      params,
    );
  } catch (e) {
    // A JSON-RPC error (e.g. invalid-params) or a transport failure —
    // surfaced verbatim, NEVER a fabricated clean verdict (cardinal-sin
    // bar at the request boundary).
    const msg = e instanceof Error ? e.message : String(e);
    cmdDeps.outputChannel.appendLine(
      `[fsm] fsm/verify request failed: ${msg}`,
    );
    error(`FSM Studio: live verification request failed — ${msg}`);
    return;
  }
  // A later invocation may have superseded us during the round-trip.
  if (inflight.get(key) !== token) {
    return;
  }

  // A not-analyzable model: the honest exit-4 reason, NEVER a fake clean
  // verdict (mirrors the CLI's own "cannot verify what won't compile";
  // identical posture to A1's exit-3/4 branch).
  if (result.verifyJson === null) {
    const detail = result.error?.trim() || "the model does not analyse";
    error(`FSM Studio: cannot verify (live) — ${detail}`);
    return;
  }

  // Render through the SAME `verifyResult.ts` A1 uses — the result is the
  // SAME `fsm-verify/v1` envelope (the LSP marshals it byte-equal to
  // `fsm verify --json`). One surface, two honest frontends; A2 does NOT
  // fork A1's render. The report tab is tagged "verify" so re-running on
  // the same file refreshes in place.
  const v = result.verifyJson;
  const body = renderVerifyReport(v, fsmPath, JSON.stringify(v));
  const uri = reportUri(fsmPath, "verify");
  await showReport(vDeps, uri, body);

  // Reachability FSM-E0400/FSM-W0602 → the SAME dedicated
  // `DiagnosticCollection` A1 publishes to (line/col straight from the
  // envelope — VS Code's native click→source; no recompute).
  publishReachabilityDiagnostics(
    vDeps.reachDiagnostics,
    vscode.Uri.file(fsmPath),
    v,
  );

  // A non-blocking toast mirroring the LSP/CLI's OWN verdict (never
  // re-derived). INCONCLUSIVE is announced as inconclusive — never a pass
  // (Doc 31 §1 W-A2; the false-proven guard).
  const { word } = verdictLabel(v);
  if (v.verdict === "verified") {
    info(`FSM Studio: live verification — ${word}.`);
  } else if (v.verdict === "inconclusive") {
    warnWithLog(
      `FSM Studio: live verification — ${word} (a bound was hit; ` +
        "NOT a proof). See the result view.",
      cmdDeps.outputChannel,
    );
  } else {
    warnWithLog(
      `FSM Studio: live verification — ${word}. See the result view.`,
      cmdDeps.outputChannel,
    );
  }
}

/**
 * Register `fsm.verify` + `fsm.baseline` (A1, CLI-spawn) + `fsm.verifyLive`
 * (A2, LSP-embed). Additive call site, the EXACT shape of
 * `registerCommands` — pushes the command disposables (plus the
 * result-doc provider + the reachability `DiagnosticCollection`, SHARED by
 * the A1 and A2 surfaces) onto `context.subscriptions`. Does NOT touch
 * V1's client-spawn / the V3 command set / the V4 diagram (disjoint
 * additive surface). A2's `fsm.verifyLive` reuses A1's `VerifyDeps` +
 * `verifyResult.ts` render — they are two honest frontends of the SAME
 * `fsm-verify` (the keystone-in-UI invariant, Doc 31 §2), not a fork.
 */
export function registerVerify(
  context: vscode.ExtensionContext,
  cmdDeps: CommandDeps,
): void {
  const resultDocs = new VerifyResultDocProvider();
  const reachDiagnostics =
    vscode.languages.createDiagnosticCollection("fsm-verify");
  context.subscriptions.push(
    resultDocs,
    reachDiagnostics,
    vscode.workspace.registerTextDocumentContentProvider(
      VerifyResultDocProvider.scheme,
      resultDocs,
    ),
  );
  const vDeps: VerifyDeps = { resultDocs, reachDiagnostics };
  // Per-document debounce/supersede token map for the live command (the
  // client-side generation-counter debounce — Doc 31 §1 W-A2 (2)).
  const liveInflight = new Map<string, number>();

  context.subscriptions.push(
    vscode.commands.registerCommand("fsm.verify", (arg) =>
      runVerify(cmdDeps, vDeps, arg),
    ),
    vscode.commands.registerCommand("fsm.baseline", (arg) =>
      runBaseline(cmdDeps, vDeps, arg),
    ),
    vscode.commands.registerCommand("fsm.verifyLive", (arg) =>
      runVerifyLive(cmdDeps, vDeps, liveInflight, arg),
    ),
  );
}
