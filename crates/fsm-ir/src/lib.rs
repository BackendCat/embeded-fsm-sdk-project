//! `fsm-ir` — canonical Intermediate Representation for FSM Studio.
//!
//! Per Doc 20 §6, this crate is pure data: structs, enums, builder, visitor.
//! No business logic. Codegen and the simulator consume this IR. The only
//! foundation types imported are `Span` and `SourceLocation` from
//! `fsm-diagnostics` (Doc 00 §7.1; previously sourced from `fsm-parser`).
//!
//! This file is a Phase 0 scaffold — no real IR types yet, but the
//! re-exports below are stable: downstream crates may already write
//! `use fsm_ir::{Span, SourceLocation};`.

pub use fsm_diagnostics::{SourceLocation, Span};

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

    #[test]
    fn reexports_span_from_diagnostics() {
        // Compile-time check that the re-export path is stable.
        let s = Span::new(0, 4);
        let loc = SourceLocation::new("file.fsm", s, 1, 1);
        assert_eq!(loc.span, s);
    }
}
