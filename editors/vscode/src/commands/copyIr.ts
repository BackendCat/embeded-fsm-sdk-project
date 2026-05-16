// `fsm.copyIR` — Doc 27 §5/§6/§10 + Doc 28 §3-V3: the ONLY IR-JSON path is
// `fsm generate --emit-ir` writing `<out>/<first-machine>.ir.json` (there
// is NO `fsm ir` subcommand and NO LSP IR method — R-9/R-10). The command
// runs it to a TEMP dir and copies the file's content to the clipboard.
//
// R-11 CODEGEN-GATED BOUNDARY (the DRIFT-class assertion, Doc 28 §3-V3 /
// Doc 27 §6.1.3): verified against the shipped CLI
// (crates/fsm-cli/src/cmd/generate.rs:165-209) — `--emit-ir` writes the
// `.ir.json` ONLY AFTER codegen `emit()` + the generated-file `fs::write`
// succeed; if codegen fails the CLI exits BEFORE the IR write, so NO
// `.ir.json` is produced. This command MUST surface that honestly: if the
// CLI failed / no `.ir.json` exists, it tells the user (codegen-gated) and
// copies NOTHING — never a stale/empty clipboard presented as success (the
// cardinal-sin bar; the named V3 gate "the test fixture is one that
// codegen-succeeds … the command must surface that honestly, asserted
// here").
//
// V4 REFACTOR (AUDIT_PHASE_V2V3_2026_05_16 §1.2, the recommended clean
// move): the `--emit-ir`-to-temp + locate-`*.ir.json` + codegen-gated
// honesty logic that used to live INLINE here is now the shared
// `diagram/emitIr.ts` core, so the V4 diagram Webview consumes the SAME
// honest IR producer (one seam, no duplicated cardinal-sin logic, no
// drift). The OBSERVABLE BEHAVIOUR of `fsm.copyIR` is byte-unchanged — the
// V3 `commands.test.ts` copyIR tests (clipboard byte-identical to the real
// artifact on success; clipboard NOT mutated on a codegen-failing fixture)
// are re-asserted unregressed (the SUBAGENT §10 refactor pre/post-identity
// bar). The mapping is exact: emitIr `ok` → write the clipboard;
// `codegenFailed`/`noIrFile` → the same "cannot copy IR — codegen failed …
// the IR is codegen-gated" decline; `noCli`/`spawnError` → the same
// missing/unspawnable-CLI errors.

import * as vscode from "vscode";

import { resolveTargetFsm } from "./activeFsm";
import { CommandDeps } from "./index";
import { emitIr } from "../diagram/emitIr";
import { error, errorWithLog, info, warn } from "./notify";

export function registerCopyIr(context: vscode.ExtensionContext, deps: CommandDeps): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("fsm.copyIR", async (arg) => {
      const fsmPath = resolveTargetFsm(arg);
      if (!fsmPath) {
        warn("FSM Studio: open a .fsm file to copy its IR.");
        return;
      }

      // The shared honest IR producer (the ONE real `fsm generate
      // --emit-ir`-to-temp path; the codegen-gated boundary is mapped to a
      // typed `codegenFailed` — emitIr never fabricates an IR).
      const res = await emitIr(
        fsmPath,
        deps.extensionPath,
        vscode.workspace.getConfiguration("fsmLang"),
        deps.outputChannel,
      );

      if (!res.ok) {
        if (res.reason === "noCli") {
          // No silent no-op — the verbatim missing-CLI message.
          error(res.detail);
          return;
        }
        if (res.reason === "spawnError") {
          error(`FSM Studio: ${res.detail}`);
          return;
        }
        // codegenFailed / noIrFile: the R-11 codegen-gated boundary,
        // surfaced honestly — copy NOTHING, tell the user exactly why.
        // Fire-and-handle: the command is DONE (correctly declining to
        // copy); the toast must not pin it "in progress".
        errorWithLog(
          "FSM Studio: cannot copy IR — codegen failed, so no IR was " +
            `produced (the IR is codegen-gated). ${res.detail}`,
          deps.outputChannel,
        );
        return;
      }

      await vscode.env.clipboard.writeText(res.json);
      deps.outputChannel.appendLine(
        `[fsm] copied IR (${res.irFileName}, ${res.json.length} bytes) ` + "to the clipboard.",
      );
      info(`FSM Studio: IR JSON copied to the clipboard (${res.irFileName}).`);
    }),
  );
}
