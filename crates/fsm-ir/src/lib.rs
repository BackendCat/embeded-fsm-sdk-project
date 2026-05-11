//! `fsm-ir` — canonical Intermediate Representation for FSM Studio.
//!
//! Per Doc 20 §6, this crate is pure data: structs, enums, builder, visitor.
//! No business logic. Codegen and the simulator consume this IR. Only the
//! parser's `SourceLocation`/`Span` type is imported (Doc 23 §4 rule).
//!
//! This file is a Phase 0 scaffold — no real IR types yet.

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
