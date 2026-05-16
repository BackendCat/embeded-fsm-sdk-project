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
export type ServerStatus = "starting" | "running" | "warnings" | "errors" | "stopped";

interface StatusVisual {
  readonly icon: string;
  readonly text: string;
  readonly tooltip: string;
}

/**
 * The Doc 22:679 crash-recovery "restarting (attempt N)" tooltip literal —
 * the N-4 fix (AUDIT_PHASE_V1 §4 / V2V3 §3 N-4 / V4 §2). Exported as a
 * pure builder so the Extension-Host §5.4 acceptance can assert the
 * shipped string is the Doc 22:679 text VERBATIM (the same
 * verbatim-contract assertion V4 used for the Doc 05 §1.5.9 STALE_BANNER —
 * a paraphrase is a contract regression). The recovery icon/text stay
 * `$(sync~spin)`/`FSM` exactly as Doc 22:679 mandates.
 */
export function restartingTooltip(attempt: number): string {
  return `FSM Language Server restarting (attempt ${attempt}/3)...`;
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
   * N-4 fix (AUDIT_PHASE_V1 §4 / V2V3 §3 N-4 / V4 §2 — the audit-sanctioned
   * V5 status-bar fold-in, Doc 22 §13.2:679). The crash-recovery
   * "restarting (attempt N)" state must render the Doc 22:679 tooltip
   * literal `FSM Language Server restarting (attempt N/3)...` — NOT the
   * generic "…starting" tooltip the recovery path previously fell through
   * to (`extension.ts` mapped the `restarting` error-handler state to the
   * shared `set("starting")`, which mislabels the tooltip; icon/text
   * already matched per the V1 audit). The Doc 22:679 string carries a
   * DYNAMIC attempt count, so a pure shared-literal swap on `"starting"`
   * is impossible without also mislabeling a genuine first-start — hence
   * a dedicated restarting render. This method is purely additive:
   * `lifecycle` is set to `"starting"` to preserve the EXACT prior
   * `refineBySeverity` behaviour the old `set("starting")` had (a no-op
   * while not running), and `visualFor`/`set`/`render`/`refineBySeverity`/
   * `ServerStatus` are byte-unchanged. The recovery icon/text stay
   * `$(sync~spin)`/`FSM` exactly as Doc 22:679 mandates.
   */
  setRestarting(attempt: number): void {
    this.lifecycle = "starting";
    this.item.text = "$(sync~spin) FSM";
    this.item.tooltip = restartingTooltip(attempt);
  }

  /**
   * The currently-rendered (icon-prefixed text, tooltip) pair. Read-only
   * test-observability so the §5.4 acceptance can assert the N-4 fix
   * BEHAVIOURALLY (a real `FsmStatusBar.setRestarting` renders the Doc
   * 22:679 tooltip on the actual status-bar item) — not merely that an
   * exported constant equals a string. No runtime path reads this.
   */
  rendered(): { text: string; tooltip: string } {
    return {
      text: this.item.text,
      tooltip: String(this.item.tooltip ?? ""),
    };
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
      hint === "error" ? "errors" : hint === "warning" ? "warnings" : "running";
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
