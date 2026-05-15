//! Monotonic ID factory — the *only* mutable participant in lowering.
//!
//! AD-3 (2026-05-15): extracted from the former `LoweringCtx` god-object
//! (Audit C LCOM cluster A). The 27 `&mut self` methods that existed only
//! to bump one of three counters collapsed to this focused type; the
//! lowerers became free functions taking `&mut IdMinter` explicitly, which
//! is what dropped the analyzer crate's mutation density (~18% → target).
//!
//! Every method here is a verbatim transcription of the corresponding
//! `LoweringCtx` method — same `format!` templates, same increment order —
//! so the IDs the lowered IR carries are byte-identical pre/post (proven
//! by the example-IR snapshot guard + the full analyzer suite).

/// Per-machine monotonic ID source.
#[derive(Debug)]
pub(crate) struct IdMinter {
    pub(crate) machine_name: String,
    /// Reserved for future cross-machine resolution; carry the machine's
    /// position in the symbol table for submachine wiring. (Audit C P0-2
    /// tracks removing this dead field — intentionally left untouched by
    /// this structure-only wave; the W2b submachine epic decides its
    /// fate.)
    #[allow(dead_code)]
    pub(crate) m_idx: usize,
    /// Counter for auto-generated transition IDs.
    transition_counter: usize,
    /// Counter for auto-generated pseudo-state IDs.
    pseudo_counter: usize,
    /// Counter for fallback state IDs when the AST is incomplete.
    state_counter: usize,
}

impl IdMinter {
    pub(crate) fn new(machine_name: &str, m_idx: usize) -> Self {
        Self {
            machine_name: machine_name.to_string(),
            m_idx,
            transition_counter: 0,
            pseudo_counter: 0,
            state_counter: 0,
        }
    }

    pub(crate) fn next_transition_id(&mut self) -> String {
        let id = format!("t-{}-{}", self.machine_name, self.transition_counter);
        self.transition_counter += 1;
        id
    }

    pub(crate) fn next_pseudo_id(&mut self, kind: &str) -> String {
        let id = format!("ps-{}-{}-{}", kind, self.machine_name, self.pseudo_counter);
        self.pseudo_counter += 1;
        id
    }

    pub(crate) fn state_id(&mut self, name: &str) -> String {
        if name.is_empty() {
            let id = format!("s-{}-anon-{}", self.machine_name, self.state_counter);
            self.state_counter += 1;
            id
        } else {
            format!("s-{}-{}", self.machine_name, name)
        }
    }

    /// Non-mutating target-ID form (was the free `state_target_id(ctx, …)`).
    /// Used where the caller wants the canonical `s-<machine>-<name>` key
    /// for a *referenced* state without auto-numbering an anonymous one.
    pub(crate) fn state_target_id(&self, name: &str) -> String {
        if name.is_empty() {
            format!("s-{}-unknown", self.machine_name)
        } else {
            format!("s-{}-{}", self.machine_name, name)
        }
    }

    /// `pseudo_counter` snapshot — `lower_region` used the live counter
    /// value as part of a fallback region name (`__region_{n}`). Preserved
    /// verbatim so that name is byte-identical.
    pub(crate) fn pseudo_counter(&self) -> usize {
        self.pseudo_counter
    }
}
