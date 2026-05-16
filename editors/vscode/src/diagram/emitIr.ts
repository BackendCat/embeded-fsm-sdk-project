// Shared `fsm generate --emit-ir`-to-temp IR-acquisition core (Doc 27 §6.1.3
// / §6.2; Doc 28 §3-V4 MV4-1).
//
// WHY this module exists (the V4 keystone): the ONLY working IR-JSON path is
// the CODEGEN-GATED `fsm generate --emit-ir` writing
// `<first-machine>.ir.json` via `fsm_ir::to_json` — there is NO LSP/server
// IR method and NO `fsm ir` subcommand (R-9/R-10/R-11; verified against
// crates/fsm-cli/src/cli.rs + crates/fsm-cli/src/cmd/generate.rs:165-209).
// V3's `copyIr.ts` already implemented exactly this path end-to-end over the
// V3 `cliBinary.ts` (the `fsm`-CLI three-rule resolver, no silent fallback)
// + `cliRunner.ts` (honest resolve-never-reject) seam. V4 (the diagram
// Webview) needs the SAME producer. Per AUDIT_PHASE_V2V3_2026_05_16 §1.2
// the clean move is to extract that producer ONCE here so both `copyIR`
// (clipboard) and V4 (postMessage / last-valid+banner) consume one honest
// seam — no duplicated cardinal-sin logic, no drift, no second resolver,
// and crucially NO new server method (the symmetric analogue of V1's
// omit-transport N-1 foot-gun). `copyIr.ts` is refactored to call this; the
// V3 `copyIR` Extension-Host test is re-asserted unregressed (the SUBAGENT
// §10 refactor pre/post-identity bar).
//
// THE CODEGEN-GATED BOUNDARY (the DRIFT-class contract, proven not assumed):
// `crates/fsm-cli/src/cmd/generate.rs` writes the `.ir.json` ONLY AFTER
// codegen `emit()` (:171) + the generated-C `fs::write` (:176-189) succeed
// (:191-209). A source that PARSES/ANALYZES clean but a codegen edge rejects
// (e.g. an unsupported `--target`, :27-33 → `ExitCode::from(2)`) exits
// BEFORE the IR write, so NO `.ir.json` is produced. This module surfaces
// that as a typed `codegenFailed` outcome — it NEVER fabricates an IR. The
// caller decides the consequence: `copyIR` declines to mutate the clipboard;
// V4 keeps the last-valid render + shows the Doc 05 §1.5.9 banner. Either
// way the failure is HONEST (the cardinal-sin bar).

import * as fs from "fs";
import * as os from "os";
import * as path from "path";

import * as vscode from "vscode";

import { noCliBinaryMessage, resolveCliBinary } from "../commands/cliBinary";
import { runCli } from "../commands/cliRunner";

/**
 * Outcome of an `--emit-ir` attempt. Exactly one of three honest states —
 * the same cardinal-sin discipline `cliRunner.ts` enforces, lifted to the
 * IR-document level so neither caller can present a failure as success.
 */
export type EmitIrResult =
  | {
      /** The CLI produced a `.ir.json`; `json` is its verbatim contents. */
      readonly ok: true;
      readonly json: string;
      /** The `<machine>.ir.json` filename (for the output channel / logs). */
      readonly irFileName: string;
    }
  | {
      readonly ok: false;
      /**
       * Why no IR was produced. `noCli` → no `fsm` binary resolved (Rule 3,
       * the verbatim message). `spawnError` → the binary could not be
       * spawned. `codegenFailed` → the CLI ran but codegen rejected before
       * the IR write (the codegen-gated boundary — NO `.ir.json`; this is
       * the V4 last-valid-render + banner / the `copyIR` decline trigger).
       * `noIrFile` → defensive: exit 0 but no `.ir.json` (the verified CLI
       * contract says this cannot happen; still never fake output).
       */
      readonly reason: "noCli" | "spawnError" | "codegenFailed" | "noIrFile";
      /** A single-line, user-presentable explanation. */
      readonly detail: string;
    };

/**
 * Run the REAL `fsm generate --target c99 --out <tmp> --emit-ir <file>` (the
 * one real IR path) to an isolated OS temp dir and return the
 * `<machine>.ir.json` contents — or a typed failure. NEVER throws for a
 * process that ran; a non-zero exit is data mapped to `codegenFailed` (the
 * codegen-gated boundary), not a masked success.
 *
 * @param fsmPath  absolute path to the `.fsm` source.
 * @param extensionPath  the extension install root (for the bundled CLI).
 * @param config  the `fsmLang` workspace configuration (for Rule-1 resolve).
 * @param log  appended with the exact command + outcome (the V3 pattern).
 */
export async function emitIr(
  fsmPath: string,
  extensionPath: string,
  config: Pick<vscode.WorkspaceConfiguration, "get">,
  log: vscode.OutputChannel,
): Promise<EmitIrResult> {
  const cli = resolveCliBinary(extensionPath, config);
  if (!cli) {
    // Rule 3 (Doc 22 §2.2 / §12): no CLI — the verbatim message, no
    // guessed-binary fallback (the cardinal-sin bar at the resolve point;
    // reused exactly from V3, not re-implemented).
    return {
      ok: false,
      reason: "noCli",
      detail: noCliBinaryMessage(`${process.platform}-${process.arch}`),
    };
  }

  // Generate into an isolated temp dir so we never pollute the workspace
  // just to read the IR (the copyIr.ts:51-56 pattern, verbatim intent).
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "fsm-emitir-"));
  try {
    const args = ["generate", "--target", "c99", "--out", tmpDir, "--emit-ir", fsmPath];
    log.appendLine(`[fsm] emitIr: ${cli.command} ${args.join(" ")}`);
    const res = await runCli(cli.command, args, {
      cwd: path.dirname(fsmPath),
    });

    if (res.spawnError) {
      log.appendLine(`[fsm] emitIr could not spawn the CLI: ${res.stderr}`);
      return {
        ok: false,
        reason: "spawnError",
        detail:
          `could not run fsm generate — ${res.stderr.trim()}` || "the fsm CLI could not be spawned",
      };
    }
    if (res.stderr.trim().length > 0) {
      log.appendLine(res.stderr.trimEnd());
    }

    // The codegen-gated boundary, surfaced honestly: a non-zero exit means
    // codegen rejected and NO .ir.json was written (generate.rs exits
    // 1/2/3/4 BEFORE the IR write at :191-209). Map it to a typed
    // codegenFailed — the caller (copyIR / V4) decides the consequence; we
    // NEVER fabricate an IR document. This is the symmetric analogue of
    // copyIr.ts:91-104 refusing to fake a clipboard.
    if (res.code !== 0) {
      const detail =
        res.stderr.trim().length > 0
          ? res.stderr.trim().split("\n")[0]
          : `fsm generate exited with code ${res.code}`;
      return { ok: false, reason: "codegenFailed", detail };
    }

    // Locate the single `*.ir.json` (named after the first machine —
    // crates/fsm-cli/src/cmd/generate.rs:191-199).
    const irFiles = fs.readdirSync(tmpDir).filter((f) => f.endsWith(".ir.json"));
    if (irFiles.length === 0) {
      // Defensive: exit 0 but no IR file (cannot happen given the verified
      // CLI contract) — still never fake output.
      return {
        ok: false,
        reason: "noIrFile",
        detail: "the CLI reported success but wrote no .ir.json file",
      };
    }

    const irPath = path.join(tmpDir, irFiles[0]);
    const json = fs.readFileSync(irPath, "utf8");
    log.appendLine(`[fsm] emitIr produced ${irFiles[0]} (${json.length} bytes).`);
    return { ok: true, json, irFileName: irFiles[0] };
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}
