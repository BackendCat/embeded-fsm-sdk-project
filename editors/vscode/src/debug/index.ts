// Debug-W2 — the interactive `.fsm` debug WebviewPanel registration
// (Doc 33 §W2). Mirrors the v1.3 `diagram/index.ts` shape exactly: it
// contributes & registers ONE command — `fsm.openDebug` — and adds NO
// runtime to the language client (additive call site; V1's client-spawn
// stays byte-unchanged — the MV4-2 / M-1 lineage the diagram followed).
//
// THE DATA SOURCE (the keystone, Doc 33 §2): the panel drives the W1
// `fsm/simulate` LSP custom request (the running language client, the
// v1.5 W-A2 boundary) as its ONE semantic oracle. It computes no FSM
// semantics — see `debugPanel.ts` / `webview/debugWebview.ts`. The
// statechart is the v1.3 diagram REUSED VERBATIM (the Doc 33 §W2 reuse
// ledger). The codegen-gated IR boundary is the v1.3 `emitIr` (last-valid
// render + the Doc 05 §1.5.9 stale banner + transport-disabled-with-reason
// — proven by the Extension-Host E2E, not assumed).

import * as vscode from "vscode";

import { resolveTargetFsm } from "../commands/activeFsm";
import { CommandDeps } from "../commands/index";
import { warn } from "../commands/notify";
import { DebugController } from "./debugPanel";

/** Re-export so the Extension-Host §W2 acceptance can observe the
 * structural model the controller rendered (the diagram-REUSE fidelity
 * gate) + assert the stale-banner string verbatim. */
export { DebugController, STALE_BANNER } from "./debugPanel";

/**
 * Register `fsm.openDebug`. Called once from `activate()` AFTER V1 has
 * built the client — an additive call site, no change to V1's spawn path
 * (the exact pattern V3's `registerCommands` / V4's `registerOpenDiagram`
 * established). Returns the controller so the E2E can observe the rendered
 * model + the W1 round-trip.
 */
export function registerOpenDebug(
  context: vscode.ExtensionContext,
  deps: CommandDeps,
): DebugController {
  const controller = new DebugController(context, deps);
  context.subscriptions.push(
    vscode.commands.registerCommand("fsm.openDebug", async (arg) => {
      const fsmPath = resolveTargetFsm(arg);
      if (!fsmPath) {
        // No silent no-op — honest "open a .fsm" guidance (the same
        // cardinal-sin bar the sibling commands apply at this boundary).
        warn("FSM Studio: open a .fsm file to debug it.");
        return;
      }
      await controller.open(fsmPath);
    }),
  );
  return controller;
}
