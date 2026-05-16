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
  VerifyResultDocProvider,
} from "./verifyResult";

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
 * Register `fsm.verify` + `fsm.baseline`. Additive call site, the EXACT
 * shape of `registerCommands` — pushes the two command disposables (plus
 * the result-doc provider + the reachability `DiagnosticCollection`) onto
 * `context.subscriptions`. Does NOT touch V1's client-spawn / the V3
 * command set / the V4 diagram (disjoint additive surface).
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

  context.subscriptions.push(
    vscode.commands.registerCommand("fsm.verify", (arg) =>
      runVerify(cmdDeps, vDeps, arg),
    ),
    vscode.commands.registerCommand("fsm.baseline", (arg) =>
      runBaseline(cmdDeps, vDeps, arg),
    ),
  );
}
