//! Reachability report — the set of state IDs the explorer actually
//! entered, and the structurally-declared states that were *never* entered
//! on any explored path.
//!
//! This is the honest backing for the catalog-reserved-but-unimplemented
//! `FSM-E0400` (unreachable state) — see Doc 30 §1.3. **Diagnostic
//! *emission* (mapping unreachable states to `FSM-E0400` /
//! `FSM-W0602` `Diagnostic` values + the CLI subcommand) is W2** (Doc 30
//! §4.2-W2 explicitly scopes E0400/W0602 emission + `fsm verify` wiring to
//! W2). W1 produces the *reachable-set fact* the W2 emission consumes; it
//! does not itself emit diagnostics or touch `fsm-analyzer`.
//!
//! Like [`crate::deadlock`], the only IR data read here is **structural**
//! (the declared state list) — "which states exist" is a shape question.
//! "Which states are reachable" is answered *only* by the interpreter-
//! driven exploration, never by a re-implemented graph walk.

use std::collections::BTreeSet;

use fsm_ir::{MachineObject, StateNode};

/// Outcome of the reachable-set computation for one machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachabilityReport {
    /// Every state ID that appeared in at least one explored
    /// configuration (the interpreter's active-leaf sets across the BFS).
    /// `BTreeSet` so iteration / serialisation is deterministic — the
    /// project's reproducibility discipline.
    pub reachable: BTreeSet<String>,
    /// Declared *concrete* states (simple / final — the leaf states a
    /// configuration can contain) that were never in any explored
    /// configuration. Empty ⇒ every concrete state is reachable. This is
    /// the honest input to W2's `FSM-E0400` emission.
    pub unreachable: BTreeSet<String>,
}

/// Collect the IDs of every concrete leaf state (simple / final) declared
/// in the machine. Pseudo-states (initial / choice / fork / …) are never
/// part of a stable configuration (Doc 08 §1) so they are excluded — an
/// `initial` pseudo-state is *transient*, flagging it "unreachable" would
/// be a false positive. **Structural shape read only.**
fn declared_concrete_states(machine: &MachineObject) -> BTreeSet<String> {
    fn walk(states: &[StateNode], acc: &mut BTreeSet<String>) {
        for s in states {
            match s {
                StateNode::Simple(s) => {
                    acc.insert(s.id.clone());
                }
                StateNode::Final(f) => {
                    acc.insert(f.id.clone());
                }
                StateNode::Composite(c) => {
                    // A composite is itself in the configuration (with one
                    // active substate). Exercised by W2's composite/parallel
                    // coverage — the declared-concrete-state set must
                    // include composite/parallel container IDs so the
                    // reachable-set fact (FSM-E0400 backing) is correct.
                    acc.insert(c.id.clone());
                    for r in &c.regions {
                        walk(&r.states, acc);
                    }
                }
                StateNode::Parallel(p) => {
                    acc.insert(p.id.clone());
                    for r in &p.regions {
                        walk(&r.states, acc);
                    }
                }
                _ => {}
            }
        }
    }
    let mut acc = BTreeSet::new();
    walk(&machine.root.states, &mut acc);
    acc
}

impl ReachabilityReport {
    /// Build the report from the explored reachable leaf-set and the
    /// machine's declared states. `reachable` is exactly what the
    /// interpreter-driven BFS observed (no inference).
    pub(crate) fn build(machine: &MachineObject, reachable: BTreeSet<String>) -> Self {
        let declared = declared_concrete_states(machine);
        let unreachable: BTreeSet<String> = declared.difference(&reachable).cloned().collect();
        Self {
            reachable,
            unreachable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ir_of(src: &str) -> fsm_ir::Ir {
        let pr = fsm_parser::parse(src);
        let res = fsm_analyzer::analyze(&pr);
        assert!(
            !res.diagnostics
                .iter()
                .any(|d| d.severity == fsm_diagnostics::Severity::Error),
            "fixture must analyze clean; got {:?}",
            res.diagnostics
        );
        res.ir.expect("analyzer produced IR")
    }

    #[test]
    fn declared_set_excludes_pseudostates() {
        let ir = ir_of(
            r#"language fsm 2.0
machine M {
    events { GO }
    initial A
    state A { on GO -> Done }
    final Done
}"#,
        );
        let m = &ir.machines[0];
        let declared = declared_concrete_states(m);
        // Two concrete states (A, Done); the `initial` pseudo-state is
        // excluded — it is transient and never part of a configuration.
        assert_eq!(declared.len(), 2, "got {declared:?}");
    }

    #[test]
    fn unreachable_is_declared_minus_explored() {
        let ir = ir_of(
            r#"language fsm 2.0
machine M {
    events { GO }
    initial A
    state A { on GO -> B }
    state B { }
    state Island { }
}"#,
        );
        let m = &ir.machines[0];
        // Pretend the explorer reached only A and B.
        let mut explored = BTreeSet::new();
        for s in &m.root.states {
            if let StateNode::Simple(s) = s {
                if s.name == "A" || s.name == "B" {
                    explored.insert(s.id.clone());
                }
            }
        }
        let report = ReachabilityReport::build(m, explored);
        assert_eq!(
            report.unreachable.len(),
            1,
            "exactly `Island` is unreachable; got {:?}",
            report.unreachable
        );
    }
}
