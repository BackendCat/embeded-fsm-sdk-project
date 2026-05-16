//! The bounded explicit-state explorer — **the keystone** (Doc 30 §4.1).
//!
//! [`verify`] performs a bounded breadth-first exploration of the reachable
//! configuration space of a *flat single-machine* FSM (W1 scope), driving
//! the shipped [`fsm_simulator::Interpreter`] as the transition oracle, and
//! reports deadlock + reachability.
//!
//! ### How it drives the interpreter (the proof there is no second
//! semantics)
//!
//! One [`Interpreter`] is built over the IR and `init`ed once. The frontier
//! holds [`InterpreterSnapshot`]s. For each frontier snapshot the explorer:
//!
//! 1. `interp.restore(snapshot)` — put the oracle *at* that configuration
//!    (`crates/fsm-simulator/src/interpreter.rs:432` — the `restore` seam);
//! 2. for **each declared event** `e`: `interp.dispatch(e)` — the oracle
//!    runs the *real* RTC step (transition selection, guards, LCA,
//!    completion drain — Doc 08, all inside the interpreter);
//! 3. `interp.snapshot()` (`interpreter.rs:415` — the `snapshot` seam) —
//!    the resulting configuration is the successor;
//! 4. `interp.restore(snapshot)` again — backtrack to try the next event.
//!
//! The explorer **never** inspects a transition, evaluates a guard, or
//! computes an LCA. Its only IR reads are the *declared event name list*
//! and the *`final` discriminator* (structural shape, not behaviour). This
//! is the §4.1 keystone realised: the verifier *executes* (via the
//! interpreter) where the analyzer deliberately does not.
//!
//! ### Successor / progress detection
//!
//! Doc 08 §3.1: if no transition is enabled the event is *discarded* and
//! the configuration is unchanged. So an event makes *progress* from C iff
//! dispatching it yields a snapshot whose [`crate::digest`] differs from
//! C's. "No progress on any declared event" + "not a final configuration"
//! ⇒ deadlock (Doc 30 §4.1 / [`crate::deadlock`]).

use std::collections::{BTreeSet, HashSet, VecDeque};

use fsm_ir::Ir;
use fsm_simulator::{InitOptions, Interpreter, InterpreterSnapshot, StepError};
use thiserror::Error;

use crate::deadlock::{is_final_configuration, DeadlockReport};
use crate::digest::ConfigDigest;
use crate::reachability::ReachabilityReport;

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
    /// bounds and contains **no deadlock**. A genuine proof (for W1's flat
    /// scope), not a bounded guess.
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
    /// W1 verifies **flat single-machine FSMs only** (Doc 30 §4.2-W1:
    /// composite / parallel / history / timer / submachine exploration is
    /// W2). A model using those is rejected *loudly* here rather than
    /// silently mis-verified — the explorer would otherwise not enumerate
    /// timer-fire / completion edges and could false-positive a timer-wait
    /// state as a deadlock (Doc 30 §4.1 sub-foot-gun). STOP-and-report, not
    /// a wrong answer.
    #[error(
        "fsm-verify W1 supports flat single-machine FSMs only; this model uses {0} \
         (composite/parallel/history/timer/submachine verification lands in W2 — Doc 30 §4.2-W2)"
    )]
    OutOfW1Scope(String),
}

/// Verify `ir`'s selected machine: bounded explicit-state exploration for
/// deadlock + reachability, driving the shipped interpreter as the oracle.
///
/// See the module docs for the keystone (how the interpreter is driven and
/// why no semantics are re-implemented) and the honest-bound invariant.
pub fn verify(ir: &Ir, opts: VerifyOptions) -> Result<VerifyOutcome, VerifyError> {
    let machine = select_machine(ir, &opts.machine_name)?;
    reject_non_flat(machine)?;

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

        // Probe every declared event. `progressed` tracks whether *any*
        // event yields a different configuration (Doc 08 §3.1: no enabled
        // transition ⇒ discard ⇒ unchanged config).
        let mut progressed = false;
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
            // Restore before EACH event so every probe starts from
            // `config_here` (dispatch mutates the oracle). This is the
            // backtrack — pure `restore`, no re-implemented backtracking.
            interp.restore(node.snap.clone())?;
            // Drive the real RTC step. Any successor state IS the
            // interpreter's answer; we do not decide it.
            interp.dispatch(ev)?;
            let succ_snap = interp.snapshot()?;
            let succ_digest = ConfigDigest::of(&succ_snap);

            if succ_digest == ConfigDigest::of(&node.snap) {
                // Event discarded — no enabled transition from here
                // (Doc 08 §3.1). Not progress.
                edges_explored += 1;
                continue;
            }
            progressed = true;
            edges_explored += 1;

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

        // Deadlock test (Doc 30 §4.1 / crate::deadlock): a reachable
        // configuration that made no progress on ANY declared event AND is
        // not a final configuration. Completion is already drained by the
        // interpreter (it only hands back quiescent configs), so this
        // configuration is genuinely stuck.
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
    // proof for W1's flat scope. (`ProvenNoDeadlock` is ONLY reachable on
    // the exhausted path — the honest-bound invariant.)
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

/// Reject anything outside W1's flat-single-machine scope **loudly**
/// (Doc 30 §4.2-W1 + §8 STOP-and-report). The interpreter *supports* the
/// full feature set, but W1's *explorer* only enumerates declared-event
/// edges — it does not yet enumerate timer-fire / submachine-completion
/// edges, so a timer-wait state would be wrongly flagged as a deadlock
/// (the §4.1 sub-foot-gun). Better a clear "out of W1 scope" error than a
/// confidently-wrong verdict.
fn reject_non_flat(machine: &fsm_ir::MachineObject) -> Result<(), VerifyError> {
    use fsm_ir::StateNode;
    if !machine.submachines.is_empty() {
        return Err(VerifyError::OutOfW1Scope("submachines".into()));
    }
    for s in &machine.root.states {
        match s {
            StateNode::Composite(_) => {
                return Err(VerifyError::OutOfW1Scope("a composite state".into()))
            }
            StateNode::Parallel(_) => {
                return Err(VerifyError::OutOfW1Scope("a parallel state".into()))
            }
            StateNode::History(_) => {
                return Err(VerifyError::OutOfW1Scope("a history pseudo-state".into()))
            }
            StateNode::Submachine(_) => {
                return Err(VerifyError::OutOfW1Scope("a submachine reference".into()))
            }
            StateNode::Simple(s) => {
                if !s.timers.is_empty() {
                    return Err(VerifyError::OutOfW1Scope("timers".into()));
                }
            }
            _ => {}
        }
    }
    Ok(())
}
