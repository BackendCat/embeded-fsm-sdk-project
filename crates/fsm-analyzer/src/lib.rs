//! `fsm-analyzer` — semantic analysis pipeline for FSM Studio.
//!
//! Per Doc 20 §5, runs as two sequential phases: structural validation
//! (symbol table, name resolution) and semantic validation (type check,
//! determinism, reachability, history invariants). Output is an
//! `AnalysisResult { ir, diagnostics, symbol_table }`.
//!
//! This file is a Phase 0 scaffold — no real analysis logic yet.

/// Placeholder stub so the crate compiles before real implementation lands.
pub fn placeholder() -> &'static str {
    "TODO"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke() {
        assert_eq!(placeholder(), "TODO");
    }
}
