//! The bounded explicit-state explorer — **the keystone** (Doc 30 §4.1).
//!
//! [`verify`] performs a bounded breadth-first exploration of the reachable
//! configuration space, driving the shipped [`fsm_simulator::Interpreter`]
//! as the transition oracle, and reports deadlock + reachability.
//!
//! **Scope (v1.4-W2).** W1 was *flat single-machine* only. W2 covers
//! **composite / parallel / history / timer / submachine** machines — the
//! full language the interpreter supports — gated on the W2-P0 lossless
//! `InterpreterSnapshot` (the armed `TimerSet` + recursive `submachines`
//! now round-trip, so the `ConfigDigest` no longer conflates
//! behaviourally-distinct configs → no false `ProvenNoDeadlock`; the audit
//! D-2 blocker, closed).
//!
//! ### How it drives the interpreter (the proof there is no second
//! semantics — the §4.1 keystone, restated for W2)
//!
//! One [`Interpreter`] is built over the IR and `init`ed once. The frontier
//! holds [`InterpreterSnapshot`]s. For each frontier snapshot the explorer
//! produces successors through **two** edge families, *both* driven by the
//! Interpreter — never a re-implemented enabled-set / clock / firing /
//! join / completion rule:
//!
//! 1. `interp.restore(snapshot)` — put the oracle *at* that configuration
//!    (`crates/fsm-simulator/src/interpreter.rs` — the `restore` seam);
//! 2. **Edge family 1 — every declared event**: `interp.dispatch(e)` — the
//!    oracle runs the *real* RTC step (transition selection, guards, LCA,
//!    region semantics, completion drain, **submachine delegation +
//!    submachine-completion** — Doc 08, all inside the interpreter);
//! 3. **Edge family 2 — the timer-fire edge** (Doc 08 §13): if any timer
//!    is armed anywhere (parent or, recursively, a nested sub-instance),
//!    `interp.advance_clock(Δ to the soonest expiry)` — the oracle runs
//!    its *own* real timer-fire RTC step;
//! 4. `interp.snapshot()` — the resulting configuration is the successor;
//! 5. `interp.restore(snapshot)` again — backtrack to try the next edge.
//!
//! Composite/parallel/history transitions and submachine-completion need
//! **no new edge kind**: they are taken *inside* `dispatch`/`advance_clock`
//! by the interpreter's real RTC drain. The explorer **never** inspects a
//! transition, evaluates a guard, computes an LCA, decides a join, fires a
//! timer, or runs a completion. Its only IR reads are the *declared event
//! name list* and the *`final` discriminator* (structural shape, not
//! behaviour). This is the §4.1 keystone realised for the full W2 feature
//! set: the verifier *executes* (via the interpreter) where the analyzer
//! deliberately does not.
//!
//! ### Successor / progress detection
//!
//! Doc 08 §3.1: if no transition is enabled the event is *discarded* and
//! the configuration is unchanged. So an edge makes *progress* from C iff
//! it yields a snapshot whose [`crate::digest`] differs from C's. "No
//! progress on **any** edge (declared event OR timer-fire) ∧ not a final
//! configuration" ⇒ deadlock — the W2-extended definition (Doc 30 §4.1 /
//! [`crate::deadlock`]): an armed-timer wait is not a deadlock (the
//! timer-fire edge progresses it), a submachine-completion-only config is
//! not a deadlock (the interpreter takes it inside `dispatch`), and an
//! all-regions-final parallel is a legitimate terminal, not a deadlock.

use std::collections::{BTreeSet, HashSet, VecDeque};

use fsm_ir::Ir;
use fsm_simulator::{InitOptions, Interpreter, InterpreterSnapshot, StepError, SubmachineSnapshot};
use thiserror::Error;

use crate::deadlock::{is_final_configuration, DeadlockReport};
use crate::digest::ConfigDigest;
use crate::reachability::ReachabilityReport;

/// Soonest absolute armed-timer expiry anywhere in the configuration —
/// the parent runtime **and**, recursively, every nested submachine
/// sub-instance (a timer armed on a state *inside* a sub-instance can
/// still fire and make progress; Doc 08 §12 + §13). `None` ⇒ no timer is
/// armed anywhere ⇒ there is no timer-fire exploration edge from this
/// configuration.
///
/// This reads **only** the (now-lossless, W2 P0) snapshot's `timers`
/// fields — it does not re-implement timer scheduling/firing (that is
/// `Interpreter::advance_clock`'s job, driven below). It just picks the
/// clock delta at which the Interpreter's *own* timer-fire RTC step will
/// produce the next distinct configuration.
fn min_armed_expiry(snap: &InterpreterSnapshot) -> Option<u64> {
    fn sub_min(sub: &SubmachineSnapshot) -> Option<u64> {
        let here = sub.timers.iter().map(|t| t.expiry_ms).min();
        let nested = sub.submachines.values().filter_map(sub_min).min();
        match (here, nested) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
    let here = snap.timers.iter().map(|t| t.expiry_ms).min();
    let nested = snap.submachines.values().filter_map(sub_min).min();
    match (here, nested) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// Default visited-configuration ceiling.
///
/// **Judgment call (Doc 30 §3.2 / §8 — disclosed).** The shared box is
/// disk-tight; an unbounded explorer is a memory-pressure vector. 100_000
/// distinct configuration *digests* (two `u64`s each ⇒ ~1.6 MB of keys,
/// plus the witness path) is comfortably within budget on the shared host
/// yet large enough that every test-suite-scale FSM verifies *exhaustively*
/// (their reachable spaces are tiny). A real customer FSM that exceeds it
/// gets an **honest** `Inconclusive`, never a false "proven" — the bound is
/// conservative *and* honest, the §3.2 owner-decision default (A). Raise it
/// with [`VerifyOptions::max_states`].
pub const DEFAULT_MAX_STATES: usize = 100_000;

/// Default explored-edge ceiling. One "step" = one `dispatch` of one
/// declared event from one frontier configuration. Bounds total work
/// independently of the visited-set size (a pathological model could pump
/// edges without growing the visited set). Scaled an order above
/// `DEFAULT_MAX_STATES` since branching factor (declared-event count) is
/// typically small.
pub const DEFAULT_MAX_STEPS: usize = 2_000_000;

/// Tunable exploration bounds + the machine to verify.
#[derive(Clone, Debug)]
pub struct VerifyOptions {
    /// Stop after this many distinct configurations are visited;
    /// [`Verdict::Inconclusive`] if hit. Defaults to
    /// [`DEFAULT_MAX_STATES`].
    pub max_states: usize,
    /// Stop after this many explored edges (event dispatches);
    /// [`Verdict::Inconclusive`] if hit. Defaults to [`DEFAULT_MAX_STEPS`].
    pub max_steps: usize,
    /// Machine to verify. Empty ⇒ the first machine in the IR.
    pub machine_name: String,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        Self {
            max_states: DEFAULT_MAX_STATES,
            max_steps: DEFAULT_MAX_STEPS,
            machine_name: String::new(),
        }
    }
}

/// Why exploration stopped — surfaced so a bound-hit is *visibly* honest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// The reachable space was fully explored within the bounds. A
    /// `ProvenNoDeadlock` is only legitimate with this reason.
    Exhausted,
    /// `max_states` was hit — result is necessarily inconclusive.
    MaxStatesHit,
    /// `max_steps` was hit — result is necessarily inconclusive.
    MaxStepsHit,
}

/// Exploration accounting (also drives the `--json` `bound`/`hit` fields in
/// W2's CLI; W1 surfaces it for the acceptance tests + library callers).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExplorationStats {
    /// Distinct configurations visited.
    pub configs_visited: usize,
    /// Edges (event dispatches) explored.
    pub edges_explored: usize,
    /// The `max_states` bound in force.
    pub max_states: usize,
    /// The `max_steps` bound in force.
    pub max_steps: usize,
    pub stop_reason: StopReason,
}

impl ExplorationStats {
    /// True iff a bound was hit (⇒ the result is inconclusive, never
    /// "proven"). The honest-bound guard reads this.
    pub fn bound_hit(&self) -> bool {
        !matches!(self.stop_reason, StopReason::Exhausted)
    }
}

/// The verification verdict.
///
/// **The honest-bound invariant (Doc 30 R1 — the cardinal sin):**
/// [`Verdict::ProvenNoDeadlock`] is *only* ever returned when the reachable
/// space was **exhausted within the bounds**. If any bound was hit the
/// verdict is [`Verdict::Inconclusive`] — the explorer never claims
/// "proven" on a truncated search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The *entire* reachable configuration space was explored within the
    /// bounds and contains **no deadlock**. A genuine proof (for the full
    /// W2 feature set — composite/parallel/history/timer/submachine), not a
    /// bounded guess.
    ProvenNoDeadlock,
    /// A reachable, non-final configuration with no possible progress was
    /// found. `witness` is the event-name sequence (from the initial
    /// configuration) that reaches it — replay it through a fresh
    /// `Interpreter` and you land in `report.config`.
    Deadlock {
        report: DeadlockReport,
        /// Declared-event names, in dispatch order, from `init` to the
        /// deadlocked configuration.
        witness: Vec<String>,
    },
    /// A bound was hit before the reachable space was exhausted. The result
    /// is **not** a proof — `stats.stop_reason` says which bound. Treating
    /// this as "verified" is the forbidden false-proven.
    Inconclusive { reason: String },
}

/// Full verification outcome: the verdict plus the reachability fact (the
/// honest backing for W2's `FSM-E0400` emission) plus exploration
/// accounting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyOutcome {
    pub verdict: Verdict,
    pub reachability: ReachabilityReport,
    pub stats: ExplorationStats,
}

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("interpreter: {0}")]
    Interpreter(#[from] StepError),
    #[error("the IR contains no machines to verify")]
    NoMachines,
    #[error("machine {0:?} not found in the IR (declared machines: {1})")]
    UnknownMachine(String, String),
    /// A model shape the verifier genuinely cannot soundly explore is
    /// rejected *loudly* (STOP-and-report, exit 4) rather than risk a
    /// false verdict (Doc 30 §6 / §8).
    ///
    /// **W2 status:** the interpreter supports composite / parallel /
    /// history / timer / submachine, the snapshot is now lossless for all
    /// of them (W2 P0 — the armed `TimerSet` + recursive `submachines` are
    /// captured), and the explorer drives every one of those edges through
    /// the Interpreter (timer-fire via `advance_clock`,
    /// composite/parallel/submachine-completion via `dispatch`'s real RTC
    /// drain). So **no in-language shape is currently out of scope** — this
    /// variant is retained as the durable STOP-not-wrong contract (exit-4)
    /// for any future shape the interpreter cannot drive, and is not
    /// emitted for any shape the v1.4 interpreter handles. The message is
    /// generic (no longer "W1 flat-only").
    #[error(
        "fsm-verify cannot soundly explore this model: {0} \
         (STOP-and-report rather than risk a false verdict — Doc 30 §6/§8)"
    )]
    OutOfScope(String),
}

/// Verify `ir`'s selected machine: bounded explicit-state exploration for
/// deadlock + reachability, driving the shipped interpreter as the oracle.
///
/// See the module docs for the keystone (how the interpreter is driven and
/// why no semantics are re-implemented) and the honest-bound invariant.
pub fn verify(ir: &Ir, opts: VerifyOptions) -> Result<VerifyOutcome, VerifyError> {
    let machine = select_machine(ir, &opts.machine_name)?;
    reject_unsupported(machine)?;

    let machine_name = machine.name.clone();
    let declared_events: Vec<String> = machine.events.iter().map(|e| e.name.clone()).collect();

    // ── Build + init the oracle once. The interpreter IS the semantics.
    let mut interp = Interpreter::new(ir)?;
    let init_opts = InitOptions {
        machine_name: machine_name.clone(),
        initial_context: None,
        virtual_clock_start_ms: 0,
    };
    interp.init(init_opts)?;
    let initial_snap = interp.snapshot()?;
    let initial_digest = ConfigDigest::of(&initial_snap);

    // ── BFS frontier. Each item carries the snapshot (to `restore` the
    // oracle there) and the witness path (declared-event names that reach
    // it). Retaining snapshots only for the live frontier + witness path is
    // the Doc 30 §3.2 memory bound; the *visited set* is digests only.
    struct Node {
        snap: InterpreterSnapshot,
        path: Vec<String>,
    }
    let mut visited: HashSet<ConfigDigest> = HashSet::new();
    visited.insert(initial_digest);
    let mut reachable_states: BTreeSet<String> = interp.current_states().into_iter().collect();

    let mut frontier: VecDeque<Node> = VecDeque::new();
    frontier.push_back(Node {
        snap: initial_snap,
        path: Vec::new(),
    });

    let mut edges_explored = 0usize;

    while let Some(node) = frontier.pop_front() {
        // Restore the oracle to this configuration so we can read its
        // active leaves and probe each declared event from it.
        interp.restore(node.snap.clone())?;
        let config_here = interp.current_states();

        let node_digest = ConfigDigest::of(&node.snap);

        // `progressed` tracks whether *any* exploration edge from this
        // configuration reaches a different configuration. An edge is one
        // of:
        //   • a declared external event — `dispatch(ev)` (Doc 08 §3.1);
        //   • the timer-fire edge — `advance_clock(Δ to next expiry)`
        //     (Doc 08 §13), driven through the Interpreter, NOT a
        //     re-implemented clock/firing rule.
        // Composite/parallel/history transitions and submachine-completion
        // need NO new edge kind: they are taken *inside* the Interpreter's
        // real RTC drain during `dispatch` / `advance_clock` (the
        // interpreter does all LCA / region / completion / sub-delegation
        // semantics). The W2 P0 lossless snapshot makes the successor
        // digest distinguish nested sub-instance / armed-timer state, so
        // these are explored correctly *by construction* with no
        // fsm-verify semantics.
        let mut progressed = false;

        // ── Edge family 1: every declared external event.
        for ev in &declared_events {
            if edges_explored >= opts.max_steps {
                return Ok(inconclusive(
                    machine,
                    reachable_states,
                    visited.len(),
                    edges_explored,
                    &opts,
                    StopReason::MaxStepsHit,
                ));
            }
            // Restore before EACH edge so every probe starts from
            // `config_here` (dispatch/advance_clock mutate the oracle).
            // This is the backtrack — pure `restore`, no re-implemented
            // backtracking.
            interp.restore(node.snap.clone())?;
            // Drive the real RTC step. Any successor state IS the
            // interpreter's answer; we do not decide it.
            interp.dispatch(ev)?;
            let succ_snap = interp.snapshot()?;
            let succ_digest = ConfigDigest::of(&succ_snap);
            edges_explored += 1;

            if succ_digest == node_digest {
                // Event discarded — no enabled transition from here
                // (Doc 08 §3.1). Not progress.
                continue;
            }
            progressed = true;

            // Record reachable leaves from the real successor config.
            for s in interp.current_states() {
                reachable_states.insert(s);
            }

            if visited.insert(succ_digest) {
                if visited.len() > opts.max_states {
                    return Ok(inconclusive(
                        machine,
                        reachable_states,
                        visited.len(),
                        edges_explored,
                        &opts,
                        StopReason::MaxStatesHit,
                    ));
                }
                let mut path = node.path.clone();
                path.push(ev.clone());
                frontier.push_back(Node {
                    snap: succ_snap,
                    path,
                });
            }
        }

        // ── Edge family 2: the timer-fire edge (Doc 08 §13). If any timer
        // is armed anywhere in the configuration (parent or, recursively,
        // a nested sub-instance — the W2 P0 lossless snapshot now carries
        // both), advancing the virtual clock to the soonest expiry lets
        // the Interpreter run its *own* real timer-fire RTC step. The
        // successor is `snapshot()`. This is the §4.1 keystone for timers:
        // we never re-implement which timer fires or what transition it
        // triggers — `advance_clock` does, exactly as production does.
        if let Some(next_expiry) = min_armed_expiry(&node.snap) {
            if edges_explored >= opts.max_steps {
                return Ok(inconclusive(
                    machine,
                    reachable_states,
                    visited.len(),
                    edges_explored,
                    &opts,
                    StopReason::MaxStepsHit,
                ));
            }
            interp.restore(node.snap.clone())?;
            let now = interp.virtual_clock_ms();
            // `next_expiry` is an absolute virtual-clock time; the delta
            // is `next_expiry - now` (saturating: a timer whose expiry is
            // already ≤ now fires on a 0-delta advance — Doc 08 §13.4).
            let delta = next_expiry.saturating_sub(now);
            interp.advance_clock(delta)?;
            let succ_snap = interp.snapshot()?;
            let succ_digest = ConfigDigest::of(&succ_snap);
            edges_explored += 1;

            if succ_digest != node_digest {
                progressed = true;
                for s in interp.current_states() {
                    reachable_states.insert(s);
                }
                if visited.insert(succ_digest) {
                    if visited.len() > opts.max_states {
                        return Ok(inconclusive(
                            machine,
                            reachable_states,
                            visited.len(),
                            edges_explored,
                            &opts,
                            StopReason::MaxStatesHit,
                        ));
                    }
                    let mut path = node.path.clone();
                    // The witness records the timer-fire as a synthetic
                    // step so a replay is unambiguous (it is not a
                    // declared event — the consumer advances the clock).
                    path.push(format!("<timer-fire @ {next_expiry}ms>"));
                    frontier.push_back(Node {
                        snap: succ_snap,
                        path,
                    });
                }
            }
        }

        // Deadlock test — Doc 30 §4.1 / [`crate::deadlock`], the
        // W2-extended definition. A reachable configuration is deadlocked
        // iff it made **no progress on ANY exploration edge** (no declared
        // event AND no timer-fire) AND it is **not a final configuration**.
        // The two W2 foot-gun guards are now structural, not special-cased
        // here:
        //   • a state armed-waiting on a timer is NOT deadlocked — the
        //     timer-fire edge above will have set `progressed` (or, if the
        //     fire produced no config change, the timer cannot make
        //     progress and the config genuinely *is* stuck — correct);
        //   • a config whose only progress is a submachine-completion is
        //     NOT deadlocked — that completion is taken inside the
        //     interpreter's `dispatch` drain, so an event edge sets
        //     `progressed` (or `init`/a prior edge already drained it);
        //   • an all-regions-final parallel config is a *legitimate
        //     terminal*, caught by `is_final_configuration` (every active
        //     leaf is `final`), NOT a deadlock.
        if !progressed && !is_final_configuration(machine, &config_here) {
            let stats = ExplorationStats {
                configs_visited: visited.len(),
                edges_explored,
                max_states: opts.max_states,
                max_steps: opts.max_steps,
                // The reachable space was fully walked *to this point* and
                // a genuine stuck config was found — a definite property
                // violation, not a bound artefact.
                stop_reason: StopReason::Exhausted,
            };
            return Ok(VerifyOutcome {
                verdict: Verdict::Deadlock {
                    report: DeadlockReport {
                        config: config_here,
                    },
                    witness: node.path,
                },
                reachability: ReachabilityReport::build(machine, reachable_states),
                stats,
            });
        }
    }

    // Frontier exhausted within the bounds, no deadlock found ⇒ a genuine
    // proof for the full W2 feature set. (`ProvenNoDeadlock` is ONLY
    // reachable on the exhausted path — the honest-bound invariant.)
    let stats = ExplorationStats {
        configs_visited: visited.len(),
        edges_explored,
        max_states: opts.max_states,
        max_steps: opts.max_steps,
        stop_reason: StopReason::Exhausted,
    };
    Ok(VerifyOutcome {
        verdict: Verdict::ProvenNoDeadlock,
        reachability: ReachabilityReport::build(machine, reachable_states),
        stats,
    })
}

fn inconclusive(
    machine: &fsm_ir::MachineObject,
    reachable: BTreeSet<String>,
    configs_visited: usize,
    edges_explored: usize,
    opts: &VerifyOptions,
    reason: StopReason,
) -> VerifyOutcome {
    let msg = match reason {
        StopReason::MaxStatesHit => format!(
            "exploration bound hit: visited {} configurations (max_states = {}). \
             The reachable space was NOT fully explored; this is NOT a proof. \
             Re-run with a higher --max-states to attempt a conclusive verdict.",
            configs_visited, opts.max_states
        ),
        StopReason::MaxStepsHit => format!(
            "exploration bound hit: explored {} edges (max_steps = {}). \
             The reachable space was NOT fully explored; this is NOT a proof. \
             Re-run with a higher --max-steps to attempt a conclusive verdict.",
            edges_explored, opts.max_steps
        ),
        StopReason::Exhausted => unreachable!("inconclusive() is never called when exhausted"),
    };
    VerifyOutcome {
        verdict: Verdict::Inconclusive { reason: msg },
        reachability: ReachabilityReport::build(machine, reachable),
        stats: ExplorationStats {
            configs_visited,
            edges_explored,
            max_states: opts.max_states,
            max_steps: opts.max_steps,
            stop_reason: reason,
        },
    }
}

fn select_machine<'a>(ir: &'a Ir, name: &str) -> Result<&'a fsm_ir::MachineObject, VerifyError> {
    if ir.machines.is_empty() {
        return Err(VerifyError::NoMachines);
    }
    if name.is_empty() {
        return Ok(&ir.machines[0]);
    }
    ir.machines.iter().find(|m| m.name == name).ok_or_else(|| {
        let declared = ir
            .machines
            .iter()
            .map(|m| m.name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        VerifyError::UnknownMachine(name.to_string(), declared)
    })
}

/// Scope gate (Doc 30 §4.2-W2 + §6/§8 STOP-and-report).
///
/// **W2 relaxation (the audit's item 2, gated on the W2-P0 lossless
/// snapshot).** W1 loud-rejected composite / parallel / history / timer /
/// submachine because W1's explorer only enumerated declared-event edges
/// (a timer-wait state would have been wrongly flagged deadlock — the
/// §4.1 foot-gun) **and** because the pre-W2 snapshot was lossy for
/// timers/submachines (digest conflation → false `ProvenNoDeadlock`).
/// W2 closes **both**: the snapshot is now lossless (W2 P0 — armed
/// `TimerSet` + recursive `submachines` round-trip byte-identically) and
/// the explorer drives **every** edge through the Interpreter —
/// composite/parallel/history/submachine-completion via `dispatch`'s real
/// RTC drain (the interpreter does all LCA / region / completion /
/// delegation semantics), timer-fire via `advance_clock` (the
/// `next_expiry` edge; Doc 08 §13). The deadlock predicate is extended
/// (armed-timer / final-parallel are NOT deadlock — see [`crate::deadlock`]).
///
/// Therefore **no in-language shape is out of scope for v1.4**: the
/// interpreter handles all of them and the explorer consumes that. This
/// function is retained as the durable STOP-not-wrong contract — it
/// rejects only a model the *interpreter itself* cannot construct/drive
/// (so the verifier never emits a verdict it cannot stand behind), which
/// is currently the empty set in-language. Kept (a) so a future
/// genuinely-unhandleable shape has a single honest reject site rather
/// than a silent wrong answer, and (b) so the exit-4 contract a factory CI
/// integrated against W1 stays a real, exercised code path.
fn reject_unsupported(machine: &fsm_ir::MachineObject) -> Result<(), VerifyError> {
    // The Interpreter (`Interpreter::new` + `init`) is the authority on
    // "can this model be driven at all". W1's structural pre-rejection of
    // composite/parallel/history/timer/submachine is REMOVED — those are
    // now fully explored through the Interpreter. No structural shape is
    // rejected here for the v1.4 interpreter's supported language; an
    // un-instantiable model surfaces as a `VerifyError::Interpreter`
    // (`StepError`) from `Interpreter::new`/`init` below, which the CLI
    // already maps to a loud non-verdict exit. `_machine` is accepted to
    // keep the seam for any future shape-level honest reject without an
    // API churn.
    let _ = machine;
    Ok(())
}
