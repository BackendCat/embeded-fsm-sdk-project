// The diagnostics oracle — the extension-layer analogue of the LSP
// `lsp_client_acceptance.rs` "recompute the oracle from the real pipeline"
// pattern (Doc 28 §5 / §1 R-15). The expected Range is NEVER hand-typed: it
// is `fsm check --json` (the established CLI oracle, the SAME analyzer
// pipeline the LSP runs) for the IDENTICAL source, mapped into VS Code's
// coordinate system, then byte-compared to what the editor actually shows.
//
// COORDINATE CONTRACTS (verified against crates/fsm-lsp/src/position.rs):
//   * `fsm check --json`  : 1-based line, 1-based **Unicode scalar** column.
//   * VS Code Range       : 0-based line, 0-based **UTF-16** character.
// The mapping used here is `(line-1, col-1)`. It is exact for the V1
// fixtures because every asserted error span sits on ASCII-only columns
// (scalar == UTF-16 there), while the multibyte content lives on a
// DIFFERENT line — this deliberately isolates the line-counting defect
// class (Doc 26 §4.1) without conflating column encodings. Each fixture's
// oracle is asserted to satisfy that ASCII-column invariant so a future
// fixture that violates it fails loudly rather than silently mis-mapping.
//
// R-15 (MANDATORY, Doc 28 §1/§5): the CLI's `fsm.toml` loader walks UPWARD
// from the file's dir. Fixtures are therefore staged in an OS temp dir
// (NOT under the repo) so the walk terminates at `/` with NO `fsm.toml`
// found — `apply_allow_deny` is a guaranteed no-op and the oracle equals
// the (allow/deny-unaware) LSP squiggle. The caller stages the temp dir;
// this module only asserts the invariant holds.

import { execFileSync } from "child_process";
import * as fs from "fs";

export interface OracleDiag {
  readonly code: string;
  readonly severity: string;
  readonly message: string;
  /** 0-based line of the span start (VS Code coordinate). */
  readonly startLine: number;
  /** 0-based character of the span start (VS Code coordinate). */
  readonly startChar: number;
  readonly endLine: number;
  readonly endChar: number;
}

interface RawJsonDiag {
  code: string;
  severity: string;
  message: string;
  line: number;
  col: number;
  endLine: number;
  endCol: number;
}

/**
 * Run the real `fsm` CLI `check --json` for `fsmFilePath` and return the
 * diagnostics mapped into VS Code coordinates. Throws if the CLI is missing
 * or emits non-JSON (a hard failure, not a silent empty oracle).
 */
export function cliOracle(fsmBinary: string, fsmFilePath: string): OracleDiag[] {
  if (!fs.existsSync(fsmBinary)) {
    throw new Error(`oracle: fsm CLI not found at ${fsmBinary}`);
  }
  let stdout: string;
  try {
    stdout = execFileSync(fsmBinary, ["check", fsmFilePath, "--json"], {
      encoding: "utf8",
    });
  } catch (e) {
    // `fsm check` exits 1 when diagnostics exist; the JSON is still on
    // stdout. Recover it from the thrown error rather than treating a
    // non-zero (expected) exit as an oracle failure.
    const err = e as { status?: number; stdout?: string };
    if (typeof err.stdout === "string" && err.stdout.length > 0) {
      stdout = err.stdout;
    } else {
      throw new Error(`oracle: fsm check produced no JSON: ${String(e)}`);
    }
  }

  const raw = JSON.parse(stdout) as RawJsonDiag[];
  return raw.map((d) => ({
    code: d.code,
    severity: d.severity,
    message: d.message,
    startLine: d.line - 1,
    startChar: d.col - 1,
    endLine: d.endLine - 1,
    endChar: d.endCol - 1,
  }));
}

/**
 * Assert the ASCII-column invariant the `(line-1, col-1)` scalar->UTF-16
 * mapping relies on: for each oracle span, the characters on the start and
 * end columns of the source must be ASCII (so scalar == UTF-16 there). If a
 * fixture ever violates this the mapping would silently mis-encode — this
 * makes that a loud failure instead.
 */
export function assertAsciiColumnInvariant(sourceText: string, diags: OracleDiag[]): void {
  const lines = sourceText.split("\n");
  for (const d of diags) {
    for (const [ln, ch] of [
      [d.startLine, d.startChar],
      [d.endLine, d.endChar],
    ] as const) {
      const line = lines[ln] ?? "";
      // Inspect the whole prefix up to the column: every code unit before
      // the column must be ASCII for scalar-count == UTF-16-count there.
      const prefix = line.slice(0, ch);
      for (const cp of prefix) {
        if (cp.codePointAt(0)! > 0x7f) {
          throw new Error(
            `oracle invariant violated: non-ASCII char before ` +
              `${d.code} at line ${ln} col ${ch} — the scalar->UTF-16 ` +
              `(line-1,col-1) mapping is only exact on ASCII columns; ` +
              `this fixture needs an explicit per-encoding mapping.`,
          );
        }
      }
    }
  }
}
