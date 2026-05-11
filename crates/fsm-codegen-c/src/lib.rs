//! `fsm-codegen-c` — C99 code generator for FSM Studio.
//!
//! Per Doc 20 §7, template-free emission via `IndentWriter` after a single
//! planning pass over the IR. Produces `Motor.h`, `Motor.c`, `Motor_impl.h`,
//! and `Motor_conf.h` for each machine. Both switch-based and table-driven
//! dispatch strategies share the planner.
//!
//! This file is a Phase 0 scaffold — no real codegen logic yet.

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
