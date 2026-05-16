// The FOUR real `fsmLang.*` inlay keys — Doc 27 §2.5 / Doc 28 R-5.
//
// WHY only four: the shipped server (`crates/fsm-lsp/src/config.rs`
// `InlayHintConfig::from_settings`) reads EXACTLY these four keys and
// nothing else from `fsmLang.*`. V1 wires precisely the real surface; it
// does NOT contribute or forward keys the server ignores (the Doc 27 §2.5
// "never silently no-op a user setting" rule — the dead-config keys belong
// to the later V-waves that own the commands which give them meaning).
//
// The server accepts both the nested-VS-Code shape AND a flat-dotted shape
// (config.rs `from_settings`); we send the nested shape that
// `workspace.getConfiguration("fsmLang")` naturally produces.

import * as vscode from "vscode";

export interface InlayHintSettings {
  readonly enableInlayHints: boolean;
  readonly inlayHints: {
    readonly showTransitionPriorities: boolean;
    readonly showStateTypes: boolean;
    readonly showTimerDurations: boolean;
  };
}

/**
 * Read the four real inlay keys from the `fsmLang` configuration section,
 * applying the server's documented defaults (master/priorities/timers ON,
 * state-types OFF — Doc 22 §8 / config.rs defensive defaults) so the
 * payload is well-formed even if a key is unset.
 */
export function readInlayHintSettings(
  config: Pick<vscode.WorkspaceConfiguration, "get">,
): InlayHintSettings {
  return {
    enableInlayHints: config.get<boolean>("enableInlayHints", true),
    inlayHints: {
      showTransitionPriorities: config.get<boolean>(
        "inlayHints.showTransitionPriorities",
        true,
      ),
      showStateTypes: config.get<boolean>(
        "inlayHints.showStateTypes",
        false,
      ),
      showTimerDurations: config.get<boolean>(
        "inlayHints.showTimerDurations",
        true,
      ),
    },
  };
}
