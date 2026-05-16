// V4 — read-only diagram WebviewPanel registration (Doc 28 §3-V4 / Doc 27
// §6 / §8-V4; Doc 22 §4/§5/§6).
//
// SCOPE (V4, strictly disjoint from V1's client-spawn, V2's grammar/
// snippets, and V3's CLI-wrapper/client-control commands): this module
// contributes & registers exactly ONE command — `fsm.openDiagram` — plus
// its manifest forward-slots (the keybinding + the `editor/title` /
// `editor/context fsm@1` / `commandPalette` slots V3's JC-5 deliberately
// LEFT for V4 — AUDIT_PHASE_V2V3_2026_05_16 §3.B / MV4-3). It adds NO
// runtime to the language client (MV4-2 / M-1 lineage: V4 touches no client
// options; V1's omit-transport `Executable` stays byte-unchanged).
//
// THE V4 DATA SOURCE (MV4-1, the keystone): the diagram consumes the SHARED
// `emitIr` core (the ONE real `fsm generate --emit-ir`-to-temp path over
// the V3 `cliBinary.ts`+`cliRunner.ts` seam). There is NO LSP/server IR
// method and NO `fsm ir` subcommand (R-9/R-10/R-11) — a V4 that "asks the
// language server for the IR" would fabricate a non-existent method (the
// symmetric analogue of V1's omit-transport foot-gun). The codegen-gated
// boundary is handled by `DiagramController` (last-valid render + Doc 05
// §1.5.9 banner — proven by the V4 Extension-Host test, not assumed).

import * as vscode from "vscode";

import { resolveTargetFsm } from "../commands/activeFsm";
import { CommandDeps } from "../commands/index";
import { DiagramController } from "./diagramPanel";
import { warn } from "../commands/notify";

/** Re-export so the V4 Extension-Host test can assert the structural model
 * the controller actually rendered (the §5.4 IR→graph fidelity gate). */
export { DiagramController } from "./diagramPanel";
export { STALE_BANNER } from "./diagramPanel";
export { parseAndBuild, buildDiagramModel } from "./irGraph";
export type { DiagramModel } from "./irGraph";

/**
 * Register `fsm.openDiagram`. Called once from `activate()` AFTER V1 has
 * built the client — an additive call site, no change to V1's spawn path
 * (the exact pattern V3's `registerCommands` already established). Returns
 * the controller so the test can observe the rendered structural model.
 */
export function registerOpenDiagram(
  context: vscode.ExtensionContext,
  deps: CommandDeps,
): DiagramController {
  const controller = new DiagramController(context, deps);
  context.subscriptions.push(
    vscode.commands.registerCommand("fsm.openDiagram", async (arg) => {
      const fsmPath = resolveTargetFsm(arg);
      if (!fsmPath) {
        // No silent no-op — honest "open a .fsm" guidance (the same
        // cardinal-sin bar V3's commands apply at this boundary).
        warn("FSM Studio: open a .fsm file to view its diagram.");
        return;
      }
      await controller.open(fsmPath);
    }),
  );
  return controller;
}
