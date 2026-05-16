// Status-bar server-state indicator — Doc 22 §10, Doc 27 §5.
//
// WHY passive: the icon is derived purely from (a) the client lifecycle
// state (`onDidChangeState`) and (b) whether the latest publishDiagnostics
// carried any Error/Warning — a *read* of data the client already received.
// It runs NO analysis (Doc 27 §5 "a passive read of the diagnostics the
// client already received — no new analysis").

import * as vscode from "vscode";

export type DiagnosticSeverityHint = "error" | "warning" | "clean";

/** The Doc 22 §10 status-bar states. */
export type ServerStatus =
  | "starting"
  | "running"
  | "warnings"
  | "errors"
  | "stopped";

interface StatusVisual {
  readonly icon: string;
  readonly text: string;
  readonly tooltip: string;
}

/** Doc 22 §10 icon/text/tooltip table — single source of truth. */
function visualFor(status: ServerStatus): StatusVisual {
  switch (status) {
    case "starting":
      return {
        icon: "$(sync~spin)",
        text: "FSM",
        tooltip: "FSM Language Server: starting",
      };
    case "running":
      return {
        icon: "$(check)",
        text: "FSM",
        tooltip: "FSM Language Server: Running",
      };
    case "warnings":
      return {
        icon: "$(warning)",
        text: "FSM",
        tooltip: "FSM Language Server: running, warnings present",
      };
    case "errors":
      return {
        icon: "$(error)",
        text: "FSM",
        tooltip: "FSM Language Server: running, errors present",
      };
    case "stopped":
      return {
        icon: "$(circle-slash)",
        text: "FSM (stopped)",
        tooltip: "FSM Language Server stopped. Click to restart.",
      };
  }
}

export class FsmStatusBar {
  private readonly item: vscode.StatusBarItem;
  /** Last lifecycle status, before severity refinement. */
  private lifecycle: ServerStatus = "starting";

  constructor() {
    this.item = vscode.window.createStatusBarItem(
      "fsm.serverStatus",
      vscode.StatusBarAlignment.Right,
      100,
    );
    this.item.command = "fsm.showOutputChannel";
    this.set("starting");
    this.item.show();
  }

  /** A lifecycle transition (running/starting/stopped) from the client. */
  set(status: ServerStatus): void {
    this.lifecycle = status;
    this.render(status);
  }

  /**
   * Refine a *running* server's icon by the latest diagnostics severity
   * (Doc 22 §10: warnings -> $(warning), errors -> $(error)). A no-op when
   * the server is not in a running state (starting/stopped win).
   */
  refineBySeverity(hint: DiagnosticSeverityHint): void {
    if (
      this.lifecycle !== "running" &&
      this.lifecycle !== "warnings" &&
      this.lifecycle !== "errors"
    ) {
      return;
    }
    const refined: ServerStatus =
      hint === "error"
        ? "errors"
        : hint === "warning"
          ? "warnings"
          : "running";
    this.render(refined);
  }

  private render(status: ServerStatus): void {
    const v = visualFor(status);
    this.item.text = `${v.icon} ${v.text}`;
    this.item.tooltip = v.tooltip;
  }

  dispose(): void {
    this.item.dispose();
  }
}
