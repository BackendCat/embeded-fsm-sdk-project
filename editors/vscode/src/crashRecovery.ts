// Exponential-backoff crash recovery — Doc 22 §13.2 verbatim, Doc 27 §2.2.
//
// WHY hand-rolled instead of the client default: Doc 22 §13.2 pins exact
// parameters (3 restarts, 3 s base, 3x multiplier, 60 s success-reset, a
// manual-restart escape after exhaustion). The default
// `vscode-languageclient` handler does not match that contract, so the
// extension owns it (Doc 27 §2.2: "implemented in the client
// `LanguageClientOptions.errorHandler` exactly as Doc 22 §13.2 specifies").

import {
  CloseAction,
  CloseHandlerResult,
  ErrorAction,
  ErrorHandler,
  ErrorHandlerResult,
} from "vscode-languageclient/node";

/** Doc 22 §13.2 recovery parameters (do not retune without the spec). */
export const MAX_RESTARTS = 3;
export const BASE_DELAY_MS = 3000;
export const BACKOFF_MULTIPLIER = 3;
export const RESET_DELAY_MS = 60_000;

/** Attempt-N delay: 3 s, 9 s, 27 s (Doc 22 §13.2 table). */
export function backoffDelayMs(attempt: number): number {
  return BASE_DELAY_MS * Math.pow(BACKOFF_MULTIPLIER, attempt - 1);
}

/**
 * The Doc 22 §13.2 error handler. `onState` is invoked on every
 * restart/give-up so the status bar (Doc 22 §10) can reflect recovery; the
 * crash counter resets to 0 after {@link RESET_DELAY_MS} of healthy uptime
 * (Doc 22 §13.2 "prevents a single transient crash from permanently
 * consuming a restart attempt").
 */
export class FsmErrorHandler implements ErrorHandler {
  private crashCount = 0;
  private resetTimer: NodeJS.Timeout | undefined;

  constructor(
    private readonly onState: (
      state:
        | { kind: "restarting"; attempt: number; delayMs: number }
        | { kind: "exhausted" },
    ) => void,
  ) {}

  /** Call when the server reaches a healthy running state. */
  noteServerReady(): void {
    this.clearResetTimer();
    this.resetTimer = setTimeout(() => {
      this.crashCount = 0;
    }, RESET_DELAY_MS);
  }

  /** Manual restart (Doc 22 §13.2 step 3) resets the counter. */
  resetCrashCount(): void {
    this.crashCount = 0;
    this.clearResetTimer();
  }

  dispose(): void {
    this.clearResetTimer();
  }

  private clearResetTimer(): void {
    if (this.resetTimer) {
      clearTimeout(this.resetTimer);
      this.resetTimer = undefined;
    }
  }

  error(): ErrorHandlerResult {
    // A protocol error is not a crash — keep going (Doc 22 §13.2 sketch).
    return { action: ErrorAction.Continue };
  }

  closed(): CloseHandlerResult {
    // The healthy-uptime timer is void the moment the process dies.
    this.clearResetTimer();
    if (this.crashCount < MAX_RESTARTS) {
      this.crashCount++;
      const delayMs = backoffDelayMs(this.crashCount);
      this.onState({
        kind: "restarting",
        attempt: this.crashCount,
        delayMs,
      });
      return {
        action: CloseAction.Restart,
        message:
          `FSM Language Server crashed. Restarting in ` +
          `${delayMs / 1000}s... (attempt ${this.crashCount}/${MAX_RESTARTS})`,
      };
    }
    this.onState({ kind: "exhausted" });
    return { action: CloseAction.DoNotRestart };
  }
}
