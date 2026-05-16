// V3 — thin CLI-wrapper + client-control command registration
// (Doc 28 §3-V3 / Doc 27 §8-V3 / §5; Doc 22 §4/§5/§6).
//
// SCOPE (V3, strictly disjoint from the parallel V2 grammar/snippets wave
// and from V1's client-spawn): this module contributes & registers exactly
// the seven Doc 28 §3-V3 commands —
//   fsm.checkFile, fsm.generateC99, fsm.generateCpp17, fsm.copyIR,
//   fsm.formatDocument, fsm.restartLanguageServer, fsm.showOutputChannel.
// The last two are the audit's M-2: V1 already WIRED the status-bar click
// (statusBar.ts:74 -> "fsm.showOutputChannel") and the crash-exhaustion
// notification buttons (extension.ts) to these IDs but, correctly per its
// depth-first scope, did NOT contribute/register them — so they are inert
// until V3 (AUDIT_PHASE_V1_2026_05_16.md §3 N-5 / §5 M-2). V3 makes them
// functional WITHOUT touching V1's client-spawn / activation /
// positionEncoding logic.
//
// M-1 (AUDIT_PHASE_V1_2026_05_16.md §3.A — CARRIED, mandatory): V1 spawns
// the language client as an `Executable` with `transport` DELIBERATELY
// OMITTED (`{ command, args: [] }`, no `transport`) because the shipped
// `fsm-lang-server` exits 2 on ANY unknown arg incl. `--stdio`
// (crates/fsm-lsp/src/main.rs:49-52). `fsm.restartLanguageServer` MUST
// therefore restart via `LanguageClient.restart()`, which re-spawns using
// V1's UNCHANGED `serverOptions` — this module never constructs a
// ServerOptions, never sets `transport`, never converts to a `NodeModule`
// shape. The omit-transport spawn is preserved by construction; the V3
// behavioural test asserts a diagnostic round-trips AFTER restart (proving
// the re-spawn did not exit 2 — the M-1 / Finding-1 regression guard).

import { State } from "vscode-languageclient/node";
import * as vscode from "vscode";

import { registerCheckFile } from "./checkFile";
import { registerCopyIr } from "./copyIr";
import { registerFormatDocument } from "./formatDocument";
import { registerGenerate } from "./generate";
import {
  registerRestartLanguageServer,
  registerShowOutputChannel,
} from "./clientControl";

/**
 * The V1-owned state the V3 commands need, passed in by `extension.ts`
 * WITHOUT V3 reaching into V1's module internals or altering its
 * client-spawn. `extension.ts` populates this from its existing variables;
 * V3 only READS the client/channel and invokes the EXISTING restart
 * semantics (it does not re-implement the spawn).
 */
export interface CommandDeps {
  /**
   * The language client `extension.ts` already built (V1's omit-transport
   * `Executable`). May be `undefined` if no binary resolved (the
   * client-control commands then degrade honestly — no silent no-op).
   */
  readonly getClient: () => import("vscode-languageclient/node").LanguageClient | undefined;
  /** V1's output channel ("FSM Language Server"). */
  readonly outputChannel: vscode.OutputChannel;
  /**
   * V1's EXISTING restart semantics (extension.ts `restartServer()`:
   * reset crash counter + `client.restart()` — preserves the
   * omit-transport spawn). V3 reuses it; it does NOT re-implement it.
   */
  readonly restartServer: () => Promise<void>;
  /** The extension install root (for the bundled `bin/<triple>/fsm`). */
  readonly extensionPath: string;
}

/** The `State` enum re-exported so the test can assert lifecycle. */
export { State };

/**
 * Register all seven V3 commands. Each `vscode.commands.registerCommand`
 * disposable is pushed onto `context.subscriptions` (the standard cleanup
 * contract). Called once from `activate()` AFTER V1 has built the client —
 * an additive call site, no change to V1's spawn path.
 */
export function registerCommands(
  context: vscode.ExtensionContext,
  deps: CommandDeps,
): void {
  registerCheckFile(context, deps);
  registerGenerate(context, deps);
  registerCopyIr(context, deps);
  registerFormatDocument(context, deps);
  registerRestartLanguageServer(context, deps);
  registerShowOutputChannel(context, deps);
}
