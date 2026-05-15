//! IR tree walker — Doc 20 §6 / VALIDATION_REPORT 1.15–1.17.
//!
//! `IrVisitor` is a default-method-based trait. Override the visit method
//! for the node kind you care about; default impls call the matching
//! `walk_*` function to descend. Walk functions live as free functions so
//! overriding code can fall back into them with `walk_machine(self, m)`
//! after doing per-node work.
//!
//! Critical fix from VALIDATION_REPORT 1.15-1.17: this walker descends
//! `MachineObject.root` (the root [`RegionObject`]) — NOT a non-existent
//! `machine.states` field — and visits externs/imports/consts so the
//! visitor surface matches Doc 09 / Doc 00 §7.4.

use crate::model::*;

/// Default-method tree walker over [`Ir`].
///
/// All hooks default to no-op + walk, so an override that only cares about
/// (say) transitions can implement only `visit_transition`.
pub trait IrVisitor {
    fn visit_ir(&mut self, ir: &Ir) {
        walk_ir(self, ir);
    }

    fn visit_machine(&mut self, m: &MachineObject) {
        walk_machine(self, m);
    }

    fn visit_region(&mut self, r: &RegionObject) {
        walk_region(self, r);
    }

    fn visit_state(&mut self, s: &StateNode) {
        walk_state(self, s);
    }

    fn visit_transition(&mut self, _t: &TransitionObject) {}

    fn visit_event(&mut self, _e: &EventObject) {}

    fn visit_extern(&mut self, _e: &ExternObject) {}

    fn visit_import(&mut self, _i: &ImportDecl) {}

    fn visit_const(&mut self, _c: &ConstDecl) {}

    fn visit_feature(&mut self, _f: &FeatureDecl) {}

    fn visit_timer(&mut self, _t: &TimerObject) {}

    fn visit_defer(&mut self, _d: &DeferDecl) {}
}

pub fn walk_ir<V: IrVisitor + ?Sized>(v: &mut V, ir: &Ir) {
    for m in &ir.machines {
        v.visit_machine(m);
    }
}

pub fn walk_machine<V: IrVisitor + ?Sized>(v: &mut V, m: &MachineObject) {
    // Per Doc 00 §7.4, file-scope declarations live on the machine. Visit
    // them BEFORE descending into the state tree so listeners that record
    // per-machine context (e.g. extern signatures) populate first.
    for i in &m.imports {
        v.visit_import(i);
    }
    for f in &m.features {
        v.visit_feature(f);
    }
    for c in &m.consts {
        v.visit_const(c);
    }
    for e in &m.events {
        v.visit_event(e);
    }
    for e in &m.externs {
        v.visit_extern(e);
    }
    // Doc 09 §3: `machine.root` is the root region. Older versions of
    // Doc 20 walked a fictional `m.states` field; that was the source of
    // VALIDATION_REPORT 1.15 / 1.16. Descend the actual structure here.
    v.visit_region(&m.root);
    // Submachines are recursive MachineObjects per Doc 09 §4.11. Walk them
    // last so any cross-machine references emitted by visit_machine come
    // before nested machines (matters for codegen ordering).
    for sub in &m.submachines {
        v.visit_machine(sub);
    }
}

pub fn walk_region<V: IrVisitor + ?Sized>(v: &mut V, r: &RegionObject) {
    for s in &r.states {
        v.visit_state(s);
    }
}

pub fn walk_state<V: IrVisitor + ?Sized>(v: &mut V, s: &StateNode) {
    match s {
        StateNode::Simple(ss) => {
            for t in &ss.transitions {
                v.visit_transition(t);
            }
            for t in &ss.timers {
                v.visit_timer(t);
            }
            for d in &ss.defers {
                v.visit_defer(d);
            }
        }
        StateNode::Composite(cs) => {
            for t in &cs.transitions {
                v.visit_transition(t);
            }
            for t in &cs.timers {
                v.visit_timer(t);
            }
            for d in &cs.defers {
                v.visit_defer(d);
            }
            for region in &cs.regions {
                v.visit_region(region);
            }
        }
        StateNode::Parallel(ps) => {
            for t in &ps.transitions {
                v.visit_transition(t);
            }
            for t in &ps.timers {
                v.visit_timer(t);
            }
            for d in &ps.defers {
                v.visit_defer(d);
            }
            for region in &ps.regions {
                v.visit_region(region);
            }
        }
        StateNode::Submachine(sm) => {
            for t in &sm.transitions {
                v.visit_transition(t);
            }
        }
        // Pseudo-states without nested transition lists — nothing to walk.
        StateNode::Initial(_)
        | StateNode::Final(_)
        | StateNode::Choice(_)
        | StateNode::Junction(_)
        | StateNode::History(_)
        | StateNode::Fork(_)
        | StateNode::Join(_)
        | StateNode::EntryPoint(_)
        | StateNode::ExitPoint(_) => {}
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_diagnostics::{SourceLocation, Span};

    fn loc() -> SourceLocation {
        SourceLocation::new("v.fsm", Span::new(0, 1), 1, 1)
    }

    #[derive(Default)]
    struct CountingVisitor {
        machines: usize,
        regions: usize,
        states: usize,
        transitions: usize,
        events: usize,
        externs: usize,
        imports: usize,
        consts: usize,
        features: usize,
    }

    impl IrVisitor for CountingVisitor {
        fn visit_machine(&mut self, m: &MachineObject) {
            self.machines += 1;
            walk_machine(self, m);
        }

        fn visit_region(&mut self, r: &RegionObject) {
            self.regions += 1;
            walk_region(self, r);
        }

        fn visit_state(&mut self, s: &StateNode) {
            self.states += 1;
            walk_state(self, s);
        }

        fn visit_transition(&mut self, _t: &TransitionObject) {
            self.transitions += 1;
        }

        fn visit_event(&mut self, _e: &EventObject) {
            self.events += 1;
        }

        fn visit_extern(&mut self, _e: &ExternObject) {
            self.externs += 1;
        }

        fn visit_import(&mut self, _i: &ImportDecl) {
            self.imports += 1;
        }

        fn visit_const(&mut self, _c: &ConstDecl) {
            self.consts += 1;
        }

        fn visit_feature(&mut self, _f: &FeatureDecl) {
            self.features += 1;
        }
    }

    #[allow(deprecated)]
    fn motor_ir() -> Ir {
        // 1 machine, 1 root region, 3 states with 1 transition each, plus
        // an event, an extern, an import, a const, a feature.
        let make_simple = |id: &str, target: &str| {
            StateNode::Simple(SimpleState {
                id: id.into(),
                stable_id: format!("Motor:state:{id}"),
                name: id.into(),
                entry: vec![],
                exit: vec![],
                transitions: vec![TransitionObject {
                    id: format!("t-{id}-{target}"),
                    stable_id: format!("Motor:transition:{id}-{target}"),
                    source: id.into(),
                    target: target.into(),
                    trigger: Some(Trigger::Event {
                        event_id: "ev-tick".into(),
                        payload_binding: None,
                    }),
                    guard: None,
                    actions: vec![],
                    priority: 100,
                    kind: TransitionKind::External,
                    internal: false,
                    hint: None,
                    loc: loc(),
                }],
                timers: vec![],
                defers: vec![],
                loc: loc(),
            })
        };
        Ir {
            ir_version: "1.0.0".into(),
            source_hash: "sha256:".into(),
            source_files: vec!["motor.fsm".into()],
            machines: vec![MachineObject {
                id: "m-motor".into(),
                stable_id: "Motor".into(),
                name: "Motor".into(),
                context: ContextSchema::default(),
                events: vec![EventObject {
                    id: "ev-tick".into(),
                    stable_id: "Motor:event:TICK".into(),
                    name: "TICK".into(),
                    payload: vec![],
                    loc: loc(),
                }],
                externs: vec![ExternObject {
                    id: "ext-log".into(),
                    stable_id: "Motor:extern:log".into(),
                    name: "log".into(),
                    pure: false,
                    params: vec![],
                    return_type: None,
                    loc: loc(),
                }],
                root: RegionObject {
                    id: "r-root".into(),
                    stable_id: None,
                    name: "__root".into(),
                    initial: "ps-initial-0".into(),
                    states: vec![
                        StateNode::Initial(InitialPseudo {
                            id: "ps-initial-0".into(),
                            target: "Idle".into(),
                            loc: loc(),
                        }),
                        make_simple("Idle", "Running"),
                        make_simple("Running", "Faulted"),
                        make_simple("Faulted", "Idle"),
                    ],
                    priority: 0,
                    loc: loc(),
                },
                submachines: vec![],
                consts: vec![ConstDecl {
                    id: "c-max".into(),
                    stable_id: "Motor:const:MAX".into(),
                    name: "MAX".into(),
                    ty: Type::Primitive { name: "u16".into() },
                    value: Literal::Int(IntLit {
                        value: 8,
                        loc: None,
                    }),
                    loc: loc(),
                }],
                imports: vec![ImportDecl {
                    path: "common/events.fsm".into(),
                    alias: Some("Common".into()),
                    named_imports: None,
                    loc: loc(),
                }],
                features: vec![FeatureDecl {
                    name: "timers".into(),
                    loc: loc(),
                }],
                queue: QueueConfig {
                    capacity: 16,
                    overflow_policy: OverflowPolicy::Assert,
                    loc: loc(),
                },
                targets: vec![],
                loc: loc(),
            }],
            diagnostics: vec![],
        }
    }

    #[test]
    fn counting_visitor_descends_root_region() {
        let ir = motor_ir();
        let mut v = CountingVisitor::default();
        v.visit_ir(&ir);
        assert_eq!(v.machines, 1);
        // Exactly one region (the root); state machine has no nested
        // composite states.
        assert_eq!(v.regions, 1);
        // 4 state nodes: 1 initial pseudo + 3 simples.
        assert_eq!(v.states, 4);
        // 3 transitions, one per simple state.
        assert_eq!(v.transitions, 3);
        assert_eq!(v.events, 1);
        assert_eq!(v.externs, 1);
        assert_eq!(v.imports, 1);
        assert_eq!(v.consts, 1);
        assert_eq!(v.features, 1);
    }
}
