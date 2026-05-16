// The verification result-summary SURFACE for `fsm.verify` / `fsm.baseline`
// (Doc 31 §1 W-A1 deliverable (2)/(3)/(4)/(5); Doc 31 §2 the keystone-in-UI
// invariant).
//
// THE CARDINAL INVARIANT (Doc 31 §2 — the load-bearing rule this whole
// epic guards): A1 surfaces verification by SPAWNING the same `fsm` binary
// the CI/factory uses (the proven `cliRunner.ts`/`cliBinary.ts` seam) and
// RENDERING its `--json` output. There is **NO** verification /
// reachability / deadlock / transition-selection / guard-eval logic in
// this file (or anywhere in `editors/vscode/**`). This module ONLY:
//   (1) declares the `fsm-verify/v1` / `fsm-trace-diff/v1` JSON envelope
//       shapes EXACTLY as `crates/fsm-cli/src/cmd/{verify,baseline}.rs`
//       define them (the contract; we are a consumer, not a re-deriver);
//   (2) parses that JSON (a JSON-object read — unknown keys tolerated, per
//       the schema's own `fsm-verify/vN` consumer policy);
//   (3) renders it as a skimmable read-only virtual document.
// A second verifier — even "just parsing the witness ourselves to
// recompute something" — is the exact P0-1 / v1.4-keystone regression. We
// recompute NOTHING: every datum rendered is read verbatim from the CLI's
// JSON. The `verdict` string the CLI emits is the verdict we show; we
// never re-derive it from `bound.hit` or anything else.
//
// SURFACE CHOICE (a disclosed senior-UX judgment call — Doc 31 §1 W-A1
// "pick the lighter surface, disclose + justify"): a READ-ONLY VIRTUAL
// DOCUMENT (`TextDocumentContentProvider`) + a `DiagnosticCollection`, NOT
// a Webview. Rationale: the verify result is fundamentally skimmable text
// (a verdict line, bound stats, an ordered witness list). The deadlock
// `counterexample.witness` is an EVENT-NAME SEQUENCE (`["ARM"]`) — NOT a
// witness graph; the JSON carries NO per-event `SourceLocation` (only
// `properties.reachability.diagnostics[]` carries `line`/`col`). A
// witness-graph Webview would be over-built for an event list and would
// add a CSP/nonce/postMessage attack surface for zero navigational gain.
// The virtual doc renders the verdict + stats + witness cleanly; the
// reachability diagnostics — the ONLY datum the contract gives a location
// for — are surfaced as a dedicated `DiagnosticCollection`, which IS VS
// Code's first-class click→source primitive (the Problems panel makes each
// entry navigable natively — the exact mechanism the diagram's
// click→source reveal and the V3 `cliOracle` coordinate contract use:
// JSON 1-based scalar line/col → VS Code 0-based). We do NOT fork the
// diagram's reveal; we use the same VS-Code-native reveal a `Diagnostic`
// provides, with line/col taken STRAIGHT from the JSON (no position
// recomputation — Doc 31 §1 W-A1 (5)).

import * as vscode from "vscode";

// ── The `fsm-verify/v1` JSON envelope (DEFINED by
// `crates/fsm-cli/src/cmd/verify.rs` module doc — mirrored here as the
// consumer's view of the contract; we read it, we do not define it). Per
// the schema's own consumer policy ("branch on the `fsm-verify/v1` major,
// tolerate unknown keys") every field is optional-on-read: a missing key
// is rendered honestly as "absent", never invented.

/** A reachability diagnostic — `FSM-E0400` / `FSM-W0602`. `line`/`col` are
 * 1-based Unicode-scalar (the CLI's coordinate contract — the SAME one
 * `cliOracle` maps with `(line-1, col-1)`). */
export interface VerifyReachDiag {
  readonly code: string;
  readonly severity: string;
  readonly message: string;
  readonly line: number;
  readonly col: number;
}

export interface VerifyJson {
  readonly schema?: string;
  readonly machine?: string;
  /** "verified" | "property-violated" | "inconclusive" — the CLI's verdict,
   * rendered VERBATIM. Never re-derived here. */
  readonly verdict?: string;
  readonly exitCode?: number;
  readonly properties?: {
    readonly deadlockFree?: {
      readonly result?: string;
      readonly counterexample?: {
        readonly config?: readonly string[];
        /** The event-sequence witness from the initial configuration. */
        readonly witness?: readonly string[];
      };
    };
    readonly reachability?: {
      readonly result?: string;
      readonly reachableStates?: readonly string[];
      readonly unreachableStates?: readonly string[];
      readonly diagnostics?: readonly VerifyReachDiag[];
    };
  };
  readonly bound?: {
    readonly maxStates?: number;
    readonly maxSteps?: number;
    readonly configsVisited?: number;
    readonly edgesExplored?: number;
    readonly hit?: boolean;
    readonly stopReason?: string;
  };
}

/** The `fsm-trace-diff/v1` envelope (DEFINED by
 * `crates/fsm-cli/src/cmd/baseline.rs` module doc). Consumer view only. */
export interface BaselineJson {
  readonly schema?: string;
  readonly mode?: string;
  readonly verdict?: string;
  readonly exitCode?: number;
  readonly corpus?: string;
  readonly fsms?: ReadonlyArray<{
    readonly fsm?: string;
    readonly machine?: string;
    readonly result?: string;
    readonly firstMismatch?: {
      readonly step?: number;
      readonly expected?: unknown;
      readonly actual?: unknown;
    };
    readonly reason?: string;
  }>;
}

/** Parse a CLI `--json` stdout string. Returns `undefined` (NOT a faked
 * empty result) if it is not the expected JSON object — the caller then
 * surfaces the raw CLI output honestly (the cardinal-sin bar: never
 * present an unparseable run as a clean verdict). */
export function parseVerifyJson(stdout: string): VerifyJson | undefined {
  try {
    const v = JSON.parse(stdout) as unknown;
    if (v && typeof v === "object" && !Array.isArray(v)) {
      return v as VerifyJson;
    }
  } catch {
    /* fall through — not JSON */
  }
  return undefined;
}

export function parseBaselineJson(stdout: string): BaselineJson | undefined {
  try {
    const v = JSON.parse(stdout) as unknown;
    if (v && typeof v === "object" && !Array.isArray(v)) {
      return v as BaselineJson;
    }
  } catch {
    /* fall through */
  }
  return undefined;
}

/**
 * Map the CLI verdict string to a HONEST one-word label + a glyph.
 *
 * This is the load-bearing honesty point (Doc 31 §1 W-A1 (3); the schema's
 * "exit-2 = explicitly NOT verified"): `verdict:"inconclusive"` /
 * `exitCode:2` MUST read as INCONCLUSIVE, never as "verified". We do NOT
 * compute this from `bound.hit` — we render the CLI's OWN `verdict` field
 * verbatim (mirroring the CLI's own honesty; a false "verified" on a
 * bound-hit is the cardinal verification-UI sin). An unrecognised verdict
 * string is shown verbatim with a neutral glyph — never silently coerced
 * to "verified".
 */
export function verdictLabel(v: VerifyJson): {
  readonly word: string;
  readonly glyph: string;
} {
  switch (v.verdict) {
    case "verified":
      return { word: "VERIFIED", glyph: "✔" };
    case "property-violated":
      return { word: "PROPERTY-VIOLATED", glyph: "✗" };
    case "inconclusive":
      // The honest bound-hit outcome. NEVER "verified".
      return { word: "INCONCLUSIVE", glyph: "?" };
    default:
      // An unknown verdict is surfaced as-is — we never default to a
      // reassuring word the CLI did not emit.
      return {
        word: (v.verdict ?? "(no verdict in CLI output)").toUpperCase(),
        glyph: "•",
      };
  }
}

/** True iff the CLI verdict is exactly "verified" (the ONLY string that
 * may render as a pass). Used by the §5.4 acceptance to assert the
 * false-proven guard: an inconclusive result must make this false. */
export function isVerified(v: VerifyJson): boolean {
  return v.verdict === "verified";
}

/**
 * Render the `fsm-verify/v1` JSON as a skimmable plain-text report (the
 * virtual-document body). Pure formatting of CLI-provided data — every
 * value is read verbatim from `v`; nothing is recomputed.
 */
export function renderVerifyReport(
  v: VerifyJson,
  fsmPath: string,
  rawStdout: string,
): string {
  // If the CLI output did not parse into the expected envelope, show the
  // raw CLI text verbatim (honest) rather than a fabricated summary.
  if (!v.schema || !v.schema.startsWith("fsm-verify/")) {
    return [
      "FSM Studio — Verification result",
      "═".repeat(48),
      "",
      "⚠ The fsm CLI did not emit a recognised fsm-verify/v1 JSON",
      "  envelope. The raw CLI output is shown verbatim below (the",
      "  result is NOT interpreted — never a fabricated verdict):",
      "",
      rawStdout.trimEnd() || "(no stdout)",
      "",
    ].join("\n");
  }

  const { word, glyph } = verdictLabel(v);
  const lines: string[] = [];
  lines.push("FSM Studio — Verification result");
  lines.push("═".repeat(48));
  lines.push("");
  lines.push(`File:    ${fsmPath}`);
  lines.push(`Machine: ${v.machine ?? "(unknown)"}`);
  lines.push(`Schema:  ${v.schema}`);
  lines.push("");
  lines.push(`VERDICT: ${glyph} ${word}`);
  if (typeof v.exitCode === "number") {
    lines.push(`         (CLI exit code ${v.exitCode})`);
  }
  if (v.verdict === "inconclusive") {
    // Make the INCONCLUSIVE state unmissable (Doc 31 §1 W-A1: "the
    // INCONCLUSIVE state unmissable"). This is NOT "verified".
    lines.push("");
    lines.push(
      "  ⚠ INCONCLUSIVE — a search bound was hit; the reachable",
    );
    lines.push(
      "    state space was NOT fully explored. This is NOT a proof",
    );
    lines.push(
      "    of correctness. Re-run with a larger --max-states /",
    );
    lines.push("    --max-steps to attempt a conclusive verdict.");
  }
  lines.push("");

  // ── Properties.
  const df = v.properties?.deadlockFree;
  lines.push("─ Deadlock-freedom ─────────────────────────────");
  lines.push(`  result: ${df?.result ?? "(absent)"}`);
  const witness = df?.counterexample?.witness;
  const config = df?.counterexample?.config;
  if (df?.result === "violated") {
    lines.push("");
    lines.push(
      `  Deadlocked configuration: ${
        config && config.length > 0 ? config.join(", ") : "(none reported)"
      }`,
    );
    lines.push("");
    if (witness && witness.length > 0) {
      lines.push(
        `  Counterexample witness (${witness.length} event${
          witness.length === 1 ? "" : "s"
        } from the initial configuration):`,
      );
      // The witness as a NAVIGABLE, ordered list (Doc 31 §1 W-A1 (4)).
      // Each step is one numbered line in a STABLE, unambiguously
      // parseable form: `  <n>. <event>` (a two-space indent, the step
      // index, ". ", then the verbatim event name). The JSON gives event
      // NAMES only (no per-event SourceLocation in the contract —
      // verify.rs's `counterexample` is `{config, witness}` with no
      // line/col), so we render the exact ordered event sequence the CLI
      // emitted. We do NOT fabricate locations the contract does not
      // provide (that would be recomputing — the cardinal sin). The
      // navigable reveal the contract DOES support — the reachability
      // diagnostics' line/col — is wired through the DiagnosticCollection
      // below (VS Code's native click→source, the same reveal the diagram
      // panel uses). The fixed `  <n>. ` prefix is the stable contract the
      // §5.4 (a) acceptance parses the rendered witness back from.
      witness.forEach((ev, i) => {
        lines.push(`  ${i + 1}. ${ev}`);
      });
    } else {
      lines.push(
        "  Counterexample witness: (empty — the initial configuration",
      );
      lines.push("    is itself the deadlock)");
    }
  }
  lines.push("");

  // ── Reachability.
  const reach = v.properties?.reachability;
  lines.push("─ Reachability ─────────────────────────────────");
  lines.push(`  result: ${reach?.result ?? "(absent)"}`);
  const reachable = reach?.reachableStates ?? [];
  const unreachable = reach?.unreachableStates ?? [];
  lines.push(`  reachable states:   ${reachable.length}`);
  lines.push(`  unreachable states: ${unreachable.length}`);
  if (unreachable.length > 0) {
    lines.push(`    ${unreachable.join(", ")}`);
  }
  const diags = reach?.diagnostics ?? [];
  if (diags.length > 0) {
    lines.push("");
    lines.push(
      `  Reachability diagnostics (${diags.length}) — also in the`,
    );
    lines.push(
      "  Problems panel, click to jump to source:",
    );
    for (const d of diags) {
      lines.push(
        `    ${d.code} [${d.severity}] ${d.message} ` +
          `(line ${d.line}, col ${d.col})`,
      );
    }
  }
  lines.push("");

  // ── Bound.
  const b = v.bound;
  lines.push("─ Search bound ─────────────────────────────────");
  if (b) {
    lines.push(`  maxStates:      ${b.maxStates ?? "(absent)"}`);
    lines.push(`  maxSteps:       ${b.maxSteps ?? "(absent)"}`);
    lines.push(`  configsVisited: ${b.configsVisited ?? "(absent)"}`);
    lines.push(`  edgesExplored:  ${b.edgesExplored ?? "(absent)"}`);
    lines.push(`  bound hit:      ${b.hit === true ? "YES" : "no"}`);
    lines.push(`  stopReason:     ${b.stopReason ?? "(absent)"}`);
  } else {
    lines.push("  (no bound reported)");
  }
  lines.push("");
  lines.push("─".repeat(48));
  lines.push(
    "This report is the verbatim render of `fsm verify --json`",
  );
  lines.push(
    "(the same `fsm` binary the CI/factory runs). No verification",
  );
  lines.push("logic runs in the editor — it spawns the CLI and renders.");
  lines.push("");
  return lines.join("\n");
}

/**
 * Render the `fsm-trace-diff/v1` (baseline) JSON as a skimmable report.
 * Pure formatting; the `verdict` is the CLI's own, rendered verbatim
 * (no-drift / drift / inconclusive — "inconclusive" is NEVER shown as
 * "no-drift", mirroring the baseline schema's own honesty).
 */
export function renderBaselineReport(
  b: BaselineJson,
  rawStdout: string,
): string {
  if (!b.schema || !b.schema.startsWith("fsm-trace-diff/")) {
    return [
      "FSM Studio — Baseline (regression-replay) result",
      "═".repeat(48),
      "",
      "⚠ The fsm CLI did not emit a recognised fsm-trace-diff/v1 JSON",
      "  envelope. Raw CLI output (verbatim, not interpreted):",
      "",
      rawStdout.trimEnd() || "(no stdout)",
      "",
    ].join("\n");
  }

  const lines: string[] = [];
  lines.push("FSM Studio — Baseline (regression-replay) result");
  lines.push("═".repeat(48));
  lines.push("");
  lines.push(`Mode:   ${b.mode ?? "(unknown)"}`);
  lines.push(`Corpus: ${b.corpus ?? "(unknown)"}`);
  lines.push("");
  const word = (b.verdict ?? "(no verdict)").toUpperCase();
  const glyph =
    b.verdict === "no-drift"
      ? "✔"
      : b.verdict === "drift"
        ? "✗"
        : b.verdict === "inconclusive"
          ? "?"
          : "•";
  lines.push(`VERDICT: ${glyph} ${word}`);
  if (typeof b.exitCode === "number") {
    lines.push(`         (CLI exit code ${b.exitCode})`);
  }
  if (b.verdict === "inconclusive") {
    lines.push("");
    lines.push(
      "  ⚠ INCONCLUSIVE — the baseline corpus was absent / unreadable /",
    );
    lines.push(
      "    not fsm-trace/v1, so drift could be neither confirmed nor",
    );
    lines.push("    denied. This is NOT a clean 'no-drift' result.");
  }
  lines.push("");
  const fsms = b.fsms ?? [];
  lines.push(`─ Per-FSM (${fsms.length}) ─────────────────────────────`);
  for (const f of fsms) {
    lines.push(
      `  ${f.result ?? "?"}  ${f.fsm ?? "(unknown)"}` +
        (f.machine ? ` [${f.machine}]` : ""),
    );
    if (f.result === "drift" && f.firstMismatch) {
      lines.push(
        `      first mismatch at step ${
          f.firstMismatch.step ?? "?"
        }`,
      );
    }
    if (f.result === "inconclusive" && f.reason) {
      lines.push(`      reason: ${f.reason}`);
    }
  }
  lines.push("");
  lines.push("─".repeat(48));
  lines.push(
    "Verbatim render of `fsm baseline --json` (the CI/factory binary).",
  );
  lines.push("");
  return lines.join("\n");
}

/**
 * The read-only virtual-document provider for the verification report.
 * One scheme (`fsm-verify`); the body is set per-URI and the provider just
 * echoes it. Read-only by construction (no `TextDocumentContentProvider`
 * write path exists) — the result is a report, never an editable buffer.
 */
export class VerifyResultDocProvider
  implements vscode.TextDocumentContentProvider
{
  public static readonly scheme = "fsm-verify";
  private readonly bodies = new Map<string, string>();
  private readonly emitter = new vscode.EventEmitter<vscode.Uri>();
  public readonly onDidChange = this.emitter.event;

  /** Set the report text for `uri` and fire a change so an already-open
   * editor refreshes (re-running verify on the same file updates in
   * place rather than opening a second tab). */
  public set(uri: vscode.Uri, body: string): void {
    this.bodies.set(uri.toString(), body);
    this.emitter.fire(uri);
  }

  public provideTextDocumentContent(uri: vscode.Uri): string {
    return (
      this.bodies.get(uri.toString()) ??
      "FSM Studio: (no verification result for this document)"
    );
  }

  public dispose(): void {
    this.emitter.dispose();
    this.bodies.clear();
  }
}

/**
 * Publish the reachability `FSM-E0400`/`FSM-W0602` diagnostics into a
 * dedicated `DiagnosticCollection` (Doc 31 §1 W-A1 (5)). This is VS Code's
 * native click→source primitive — the Problems panel makes each entry
 * jump-to-source (the SAME reveal the diagram panel does, generalised
 * through the platform). The Range is `(line-1, col-1)` — taken STRAIGHT
 * from the JSON (the CLI's 1-based Unicode-scalar coordinate, the exact
 * mapping the V3 `cliOracle` contract uses). We do NOT recompute any
 * position; we map the contract's numbers into VS Code's 0-based system
 * and nothing else.
 *
 * Returns the list of published `{line,col}` pairs (test observability —
 * the §5.4 (d) acceptance asserts these equal the CLI JSON's line/col).
 */
export function publishReachabilityDiagnostics(
  collection: vscode.DiagnosticCollection,
  fsmUri: vscode.Uri,
  v: VerifyJson,
): Array<{ readonly line: number; readonly col: number; readonly code: string }> {
  collection.delete(fsmUri);
  const diags = v.properties?.reachability?.diagnostics ?? [];
  const published: Array<{
    readonly line: number;
    readonly col: number;
    readonly code: string;
  }> = [];
  const vscodeDiags: vscode.Diagnostic[] = diags.map((d) => {
    // JSON: 1-based scalar line/col → VS Code: 0-based. The exact
    // `cliOracle` mapping. Clamp at 0 so a malformed 0 never throws.
    const line = Math.max(0, d.line - 1);
    const col = Math.max(0, d.col - 1);
    const pos = new vscode.Position(line, col);
    // A zero-width range at the reported position — VS Code still makes
    // it click-navigable; we do not invent an end position the contract
    // does not give.
    const range = new vscode.Range(pos, pos);
    const severity =
      d.severity === "error"
        ? vscode.DiagnosticSeverity.Error
        : d.severity === "warning"
          ? vscode.DiagnosticSeverity.Warning
          : d.severity === "info"
            ? vscode.DiagnosticSeverity.Information
            : vscode.DiagnosticSeverity.Hint;
    const diag = new vscode.Diagnostic(range, d.message, severity);
    diag.code = d.code;
    diag.source = "fsm verify";
    published.push({ line: d.line, col: d.col, code: d.code });
    return diag;
  });
  collection.set(fsmUri, vscodeDiags);
  return published;
}
