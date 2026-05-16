// Shared resolution of "the .fsm file this command targets".
//
// WHY a shared helper: a command can be invoked from (a) the command
// palette / a keybinding — target = the active editor's document; or
// (b) the explorer context menu (Doc 22 §6 `explorer/context`,
// `resourceExtname == .fsm`) — VS Code passes the right-clicked resource
// `Uri` as the FIRST handler argument. Honouring the arg first makes the
// explorer-context entries actually act on the clicked file (not whatever
// happens to be focused), and falling back to the active editor keeps the
// palette/keybinding path working. Centralised so all four CLI-wrapper
// commands behave identically.

import * as vscode from "vscode";

/**
 * The .fsm path the command should act on, or `undefined` if none can be
 * determined (the caller then warns; never silently no-ops).
 *
 * @param arg the first handler argument — a `Uri` for explorer-context
 *            invocations, otherwise undefined.
 */
export function resolveTargetFsm(arg: unknown): string | undefined {
  // (b) explorer-context: VS Code passes the resource Uri as arg 0.
  if (arg instanceof vscode.Uri && arg.scheme === "file") {
    if (arg.fsPath.endsWith(".fsm")) {
      return arg.fsPath;
    }
    return undefined;
  }

  // (a) palette / keybinding: the active .fsm editor.
  const ed = vscode.window.activeTextEditor;
  if (ed && ed.document.languageId === "fsm-lang") {
    return ed.document.uri.fsPath;
  }
  return undefined;
}
