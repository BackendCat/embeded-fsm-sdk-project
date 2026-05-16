// `fsm.restartLanguageServer` + `fsm.showOutputChannel` — the audit M-2
// commands (Doc 27 §5/§8-V3; Doc 22 §4; AUDIT_PHASE_V1_2026_05_16.md §5
// M-2). V1 already WIRED the status-bar click (statusBar.ts:74) and the
// crash-exhaustion notification buttons (extension.ts) to these IDs but
// did not contribute/register them — registering them here makes those
// already-wired V1 affordances functional.
//
// M-1 (AUDIT_PHASE_V1_2026_05_16.md §3.A — mandatory): the restart MUST go
// through V1's EXISTING `restartServer()` (which calls
// `LanguageClient.restart()` on the client built with V1's omit-transport
// `Executable`). This module NEVER constructs a `ServerOptions`, NEVER sets
// `transport`, NEVER switches to a `NodeModule` shape. `client.restart()`
// re-spawns with the SAME unchanged `serverOptions`, so the no-`--stdio`
// (omit-transport) spawn that round-trips against the strict
// `fsm-lang-server` arg-parser (crates/fsm-lsp/src/main.rs:49-52 exits 2
// on any unknown arg) is preserved by construction.

import { State } from "vscode-languageclient/node";
import * as vscode from "vscode";

import { CommandDeps } from "./index";
import { warnWithLog } from "./notify";

/**
 * `fsm.restartLanguageServer` — reuse V1's `restartServer()` (reset crash
 * counter + `client.restart()`), then surface the outcome honestly. If no
 * client exists (no binary resolved at activation) the command does NOT
 * silently no-op: it tells the user why and points at the log (the
 * cardinal-sin bar at the command boundary).
 */
export function registerRestartLanguageServer(
  context: vscode.ExtensionContext,
  deps: CommandDeps,
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "fsm.restartLanguageServer",
      async () => {
        const client = deps.getClient();
        if (!client) {
          // Honest degradation: no language client to restart (no
          // fsm-lang-server binary resolved). Not a silent no-op. The
          // toast is fire-and-handle (not awaited) — the command is done.
          deps.outputChannel.appendLine(
            "[fsm] restart requested but no language client is running " +
              "(no fsm-lang-server binary resolved at activation).",
          );
          warnWithLog(
            "FSM Studio: no language server is running to restart. " +
              "Set fsmLang.compilerPath or install a bundled binary.",
            deps.outputChannel,
          );
          return;
        }

        // V1's restart semantics — resets the Doc 22 §13.2 crash counter
        // and calls client.restart() (re-spawns with V1's UNCHANGED
        // omit-transport serverOptions; M-1 preserved by construction).
        deps.outputChannel.appendLine(
          "[fsm] fsm.restartLanguageServer — restarting language client " +
            "(omit-transport Executable preserved; M-1).",
        );
        await deps.restartServer();

        // Confirm the client actually came back up (it would NOT if a
        // regression had re-added `--stdio`/`transport` and the server
        // exited 2). This is the user-visible half of the M-1 guard; the
        // V3 behavioural test asserts a diagnostic re-round-trips.
        if (client.state === State.Running) {
          deps.outputChannel.appendLine(
            "[fsm] language client is Running again after restart.",
          );
        } else {
          deps.outputChannel.appendLine(
            `[fsm] WARNING: language client state is ${State[client.state]} ` +
              "after restart — the server may have failed to start.",
          );
        }
      },
    ),
  );
}

/**
 * `fsm.showOutputChannel` — reveal V1's "FSM Language Server" output
 * channel (Doc 27 §5: "reveal the client's output channel"). This is the
 * exact target statusBar.ts:74 already wires the status-bar click to;
 * registering it makes that V1 click functional. `preserveFocus=true` so
 * revealing the log does not steal focus from the editor.
 */
export function registerShowOutputChannel(
  context: vscode.ExtensionContext,
  deps: CommandDeps,
): void {
  context.subscriptions.push(
    vscode.commands.registerCommand("fsm.showOutputChannel", () => {
      deps.outputChannel.show(true);
    }),
  );
}
