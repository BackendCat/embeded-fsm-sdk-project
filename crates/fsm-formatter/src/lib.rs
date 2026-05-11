//! `fsm-formatter` — canonical formatter for FSM-Lang source.
//!
//! Per Doc 20 §12.9, operates over the lossless CST so comments and
//! original trivia survive. Idempotent: `fmt` then `fmt --check` must
//! exit 0. Independent of the analyzer and IR.
//!
//! This file is a Phase 0 scaffold — no real formatting logic yet.

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
