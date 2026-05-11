//! `fsm-lexer` — tokenizer for FSM-Lang.
//!
//! Per Doc 20 §3, this crate is zero-copy and `no_std`-friendly. It produces
//! `(TokenKind, Span)` pairs over a borrowed `&str`. No allocation per token,
//! no analysis, no recovery beyond emitting `TokenKind::Error` and advancing.
//!
//! This file is a Phase 0 scaffold — no real lexer logic yet.

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
