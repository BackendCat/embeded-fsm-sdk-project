//! `fsm-simulator` — pure-Rust interpreter implementing FSM Studio semantics.
//!
//! Per Doc 20 §8, executes the IR directly (no generated C code). Implements
//! the Run-To-Completion step algorithm from FSM-SPEC-SEM, with virtual or
//! wall clock, breakpoints, and trace recording. The WebSocket JSON-RPC
//! server described in §8.4 is deferred to v1.1+.
//!
//! This file is a Phase 0 scaffold — no real interpreter logic yet.

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
