//! Workspace configuration the server actually consumes — Doc 26 §8 L7 /
//! Doc 14 §3 / Doc 22 §8.
//!
//! Doc 26 §7 open-question 8: v1.2 wires the `fsmLang.*` keys that gate
//! *existing* analyzer/LSP behaviour and **stub-accepts the rest**. L7 is
//! the first wave with a behaviour-gating config need: the four Doc 22 §8
//! inlay-hint toggles directly govern which `textDocument/inlayHint`
//! results are emitted. Only those four are parsed here; every other key
//! in the client's settings blob is *ignored* (a tolerant parse — exactly
//! the "stub-accept the rest" decision, no panic on unknown keys), so this
//! is the minimal, behaviour-correct slice, not a config subsystem.
//!
//! Source channels (Doc 14 §3: "on startup and on
//! `workspace/didChangeConfiguration`"): the `initialize`
//! `initializationOptions` blob (Doc 14 §2's `fsmLang` object) AND the
//! `workspace/didChangeConfiguration` `settings` blob. Both are arbitrary
//! JSON; [`InlayHintConfig::from_settings`] reads the four keys
//! defensively — any missing/mistyped key falls back to its **Doc 22 §8
//! default**, so a client that sends nothing gets exactly the documented
//! defaults (master on, priorities on, state-types off, timers on).

use serde_json::Value;

/// The four Doc 22 §8 inlay-hint toggles, with the Doc 22 §8 / Doc 14 §3
/// defaults. `enabled` is the master switch (`fsmLang.enableInlayHints`);
/// the three per-category switches gate the individual Doc 14 §11 hint
/// families.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct InlayHintConfig {
    /// `fsmLang.enableInlayHints` — Doc 22 §8 default **true**. When
    /// `false` NO inlay hint is emitted regardless of the per-category
    /// switches (the master gate, Doc 14 §3 "Toggle all inlay hints").
    pub enabled: bool,
    /// `fsmLang.inlayHints.showTransitionPriorities` — Doc 22 §8 default
    /// **true**. Gates the Doc 14 §11 non-default-priority hint.
    pub show_transition_priorities: bool,
    /// `fsmLang.inlayHints.showStateTypes` — Doc 22 §8 default **false**
    /// (the one toggle that is off by default). Gates the Doc 14 §11
    /// composite/parallel substate-count hint.
    pub show_state_types: bool,
    /// `fsmLang.inlayHints.showTimerDurations` — Doc 22 §8 default
    /// **true**. Gates the Doc 14 §11 human-readable timer-duration hint.
    pub show_timer_durations: bool,
}

impl Default for InlayHintConfig {
    /// The Doc 22 §8 / Doc 14 §3 documented defaults — what a client that
    /// sends no configuration at all must observe.
    fn default() -> Self {
        InlayHintConfig {
            enabled: true,
            show_transition_priorities: true,
            show_state_types: false,
            show_timer_durations: true,
        }
    }
}

impl InlayHintConfig {
    /// Parse the four toggles out of a client settings blob (the
    /// `initializationOptions` object or a `didChangeConfiguration`
    /// `settings` object). Defensive by design: a key that is absent or
    /// not a JSON boolean keeps its Doc 22 §8 default — a malformed config
    /// must never silently flip a hint family the user did not ask to
    /// change (and must never panic). Unknown keys are ignored (Doc 26 §7
    /// open-question 8 "stub-accept the rest").
    ///
    /// Both the flat (`"fsmLang.inlayHints.showStateTypes"`) and the
    /// nested (`fsmLang: { inlayHints: { showStateTypes } }`) shapes are
    /// accepted because VS Code's `workspace/configuration` delivers the
    /// nested object while a flat `initializationOptions` map (and some
    /// test clients) use dotted keys — reading both is strictly more
    /// tolerant and never changes a value the client did not set.
    pub fn from_settings(v: &Value) -> Self {
        let d = InlayHintConfig::default();
        let read = |flat: &str, path: &[&str]| -> bool {
            // Dotted top-level key first (flat shape), then the nested
            // object path (VS Code shape); fall back to the default.
            v.get(flat)
                .and_then(Value::as_bool)
                .or_else(|| {
                    let mut cur = v;
                    for seg in path {
                        cur = cur.get(seg)?;
                    }
                    cur.as_bool()
                })
                .unwrap_or_else(|| match path {
                    ["fsmLang", "enableInlayHints"] => d.enabled,
                    ["fsmLang", "inlayHints", "showTransitionPriorities"] => {
                        d.show_transition_priorities
                    }
                    ["fsmLang", "inlayHints", "showStateTypes"] => d.show_state_types,
                    ["fsmLang", "inlayHints", "showTimerDurations"] => d.show_timer_durations,
                    _ => unreachable!("only the four Doc 22 §8 inlay keys are read"),
                })
        };
        InlayHintConfig {
            enabled: read("fsmLang.enableInlayHints", &["fsmLang", "enableInlayHints"]),
            show_transition_priorities: read(
                "fsmLang.inlayHints.showTransitionPriorities",
                &["fsmLang", "inlayHints", "showTransitionPriorities"],
            ),
            show_state_types: read(
                "fsmLang.inlayHints.showStateTypes",
                &["fsmLang", "inlayHints", "showStateTypes"],
            ),
            show_timer_durations: read(
                "fsmLang.inlayHints.showTimerDurations",
                &["fsmLang", "inlayHints", "showTimerDurations"],
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn default_is_the_doc22_s8_documented_defaults() {
        // Doc 22 §8: master on, priorities on, state-types OFF, timers on.
        let d = InlayHintConfig::default();
        assert!(d.enabled);
        assert!(d.show_transition_priorities);
        assert!(
            !d.show_state_types,
            "showStateTypes default is false (Doc 22 §8)"
        );
        assert!(d.show_timer_durations);
    }

    #[test]
    fn empty_or_unrelated_settings_yield_defaults_not_panic() {
        // Doc 26 §7 open-question 8: unknown keys are stub-accepted, no
        // panic; absent inlay keys keep their Doc 22 §8 defaults.
        assert_eq!(
            InlayHintConfig::from_settings(&json!({})),
            InlayHintConfig::default()
        );
        assert_eq!(
            InlayHintConfig::from_settings(&json!({ "fsmLang": { "debounceMs": 50 } })),
            InlayHintConfig::default()
        );
    }

    #[test]
    fn nested_vscode_shape_overrides_only_the_set_keys() {
        let v = json!({ "fsmLang": { "inlayHints": { "showTransitionPriorities": false } } });
        let c = InlayHintConfig::from_settings(&v);
        assert!(!c.show_transition_priorities, "the set key flips");
        // Every UNSET key keeps its Doc 22 §8 default — a partial config
        // must not silently disturb other families.
        assert!(c.enabled);
        assert!(!c.show_state_types);
        assert!(c.show_timer_durations);
    }

    #[test]
    fn flat_dotted_shape_is_also_read() {
        let v = json!({
            "fsmLang.enableInlayHints": false,
            "fsmLang.inlayHints.showStateTypes": true
        });
        let c = InlayHintConfig::from_settings(&v);
        assert!(!c.enabled);
        assert!(c.show_state_types);
        // Unset → defaults.
        assert!(c.show_transition_priorities);
        assert!(c.show_timer_durations);
    }

    #[test]
    fn mistyped_value_falls_back_to_default_not_garbage() {
        // A non-boolean must NOT be coerced — keep the documented default.
        let v = json!({ "fsmLang": { "enableInlayHints": "yes" } });
        assert!(
            InlayHintConfig::from_settings(&v).enabled,
            "a string where a bool was expected keeps the Doc 22 §8 default"
        );
    }
}
