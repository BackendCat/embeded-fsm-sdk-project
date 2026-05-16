// Non-blocking user notifications for the V3 commands.
//
// WHY this exists (a real UX correctness fix, not just a test affordance):
// a command must NOT stay "running" (the command spinner active, the
// command un-re-invokable) while waiting for the user to dismiss a toast.
// `await vscode.window.showXMessage(msg, "Show Log")` does NOT resolve
// until the user clicks or dismisses the notification — so awaiting it
// inside a command handler pins the command "in progress" indefinitely if
// the user ignores the toast (and hangs a headless Extension-Host test
// forever). The command's WORK (file written / clipboard set / buffer
// formatted) is already complete by the time we notify; the toast + its
// optional "Show Log" button are an after-the-fact affordance and must be
// fire-and-handle, never block completion. These helpers show the toast
// and wire the "Show Log" button via `.then()` (so a click still reveals
// the log) WITHOUT the handler awaiting it.

import * as vscode from "vscode";

type Kind = "info" | "warning" | "error";

function showLogButton(kind: Kind, message: string, outputChannel: vscode.OutputChannel): void {
  const fn =
    kind === "error"
      ? vscode.window.showErrorMessage
      : kind === "warning"
        ? vscode.window.showWarningMessage
        : vscode.window.showInformationMessage;
  // NOT awaited: the command returns now; the click is handled async.
  void Promise.resolve(fn(message, "Show Log")).then((choice) => {
    if (choice === "Show Log") {
      outputChannel.show(true);
    }
  });
}

/** A plain info toast with no action (already non-blocking via `void`). */
export function info(message: string): void {
  void vscode.window.showInformationMessage(message);
}

/** An info toast offering "Show Log" — fire-and-handle, never awaited. */
export function infoWithLog(message: string, outputChannel: vscode.OutputChannel): void {
  showLogButton("info", message, outputChannel);
}

/** A warning toast offering "Show Log" — fire-and-handle, never awaited. */
export function warnWithLog(message: string, outputChannel: vscode.OutputChannel): void {
  showLogButton("warning", message, outputChannel);
}

/** An error toast offering "Show Log" — fire-and-handle, never awaited. */
export function errorWithLog(message: string, outputChannel: vscode.OutputChannel): void {
  showLogButton("error", message, outputChannel);
}

/** A plain warning toast with no action (non-blocking). */
export function warn(message: string): void {
  void vscode.window.showWarningMessage(message);
}

/** A plain error toast with no action (non-blocking). */
export function error(message: string): void {
  void vscode.window.showErrorMessage(message);
}
