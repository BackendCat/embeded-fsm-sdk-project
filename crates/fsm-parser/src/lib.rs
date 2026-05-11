//! `fsm-parser` — recursive-descent parser building a rowan CST and typed AST.
//!
//! Per Doc 20 §4, the parser is two-phase: lossless CST first (every token,
//! including trivia, retained for the formatter and LSP incremental reparse),
//! then a thin typed AST layered on top. Synchronizes on top-level keywords
//! during error recovery.
//!
//! This file is a Phase 0 scaffold — no real parser logic yet.

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
