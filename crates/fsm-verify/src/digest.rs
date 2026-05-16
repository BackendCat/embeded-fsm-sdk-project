//! Stable configuration digest (Doc 30 §3.2 / §4.1 sub-foot-gun).
//!
//! The visited set is keyed by a *digest* of the interpreter snapshot, not
//! the retained snapshot. [`InterpreterSnapshot`] is composed entirely of
//! `Vec` / `BTreeMap` / scalars, so its canonical `serde_json` encoding is
//! byte-deterministic across runs and processes — the *same* wire-format
//! contract (`BTreeMap` key ordering, no `HashMap`) that
//! `fsm-simulator`'s trace layer depends on for golden-trace replay
//! (Doc 13 §11). We hash that canonical form.
//!
//! We deliberately do **not** invent a bespoke field-walk: serialising the
//! snapshot reuses the project's existing byte-stability guarantee rather
//! than fabricating a parallel one (a digest that disagreed with the wire
//! form would silently conflate or split configurations).
//!
//! ### W2 clock-origin normalisation (sound — disclosed judgment call)
//!
//! W1 keyed the **absolute** `virtual_clock_ms`. For W2's timer scope that
//! is unsound *in the false-negative direction*: a `every_internal`
//! heartbeat or any cyclic-timer FSM advances the clock forever, so the
//! digest never repeats, the visited set never converges, and a **genuine
//! timer-deadlock is reported `Inconclusive` instead of `Deadlock`**
//! (observed empirically). Doc 08 §13.5 (the normative
//! absolute-virtual-clock non-observability lemma) makes the absolute clock
//! **non-observable** — only *relative* timer phase affects behaviour — so
//! [`canonical_bytes`] normalises the clock origin to 0 and keys timers by
//! *remaining duration*, recursively for sub-instances. This merges
//! exactly the clock-shift-equivalent (= strongly bisimilar) configs:
//! **sound for reachability + deadlock, never a false `ProvenNoDeadlock`**
//! (it only merges configs with a provably identical future), and
//! *required* for timer-deadlock detection + termination. The bisimulation
//! proof is on [`normalize_clock_origin_sub`]. The §4.1 conflation the W2
//! P0 lossless snapshot closed stays closed — relative timer phase and all
//! nested sub-instance structure remain in the key (digest-soundness
//! tests assert this). This supersedes W1's `distinct_virtual_clock_*`
//! unit assumption (which encoded a conservative over-approximation that
//! actively *blocked* correct W2 deadlock detection — the same class as
//! the audit's Doc 30 §4.1 "already complete" overstatement: a W1
//! assumption that does not hold for W2's scope).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use fsm_simulator::{InterpreterSnapshot, SubmachineSnapshot};

/// A stable 128-bit-ish configuration key. Two snapshots collide here iff
/// their canonical JSON encodings are byte-identical — i.e. iff they are
/// the *same* reachable configuration up to the sound normalisations in
/// [`canonical_bytes`] (same active leaves, history, defer set, context,
/// **and the same relative timer phases / nested sub-instance state**).
/// `next_trace_id` and the clock *origin* are book-keeping / non-observable
/// and are normalised out; see [`canonical_bytes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ConfigDigest {
    /// Two independent hashers over the same canonical bytes — widening the
    /// key to make an accidental collision astronomically unlikely without
    /// pulling a crypto-hash dependency into a disk-conscious workspace.
    hi: u64,
    lo: u64,
}

/// Sound **clock-origin normalisation** (v1.4-W2, Doc 08 §13.5 — disclosed
/// judgment call, see the module-level `### W2 clock-origin normalisation`
/// note and the completion report).
///
/// Shifts a runtime's `virtual_clock_ms` to 0 and rewrites every armed
/// timer's absolute `expiry_ms` to its **remaining duration**
/// (`expiry_ms − virtual_clock_ms`, saturating). Applied to the parent
/// runtime and, **recursively and independently**, to every nested
/// submachine sub-instance (each sub has its own clock + timers — Doc 08
/// §12; each is normalised by *its own* clock origin).
///
/// **Why this is sound (the bisimulation argument).** Doc 08 §13.5: a timer
/// fires exactly when its owning runtime's `virtual_clock_ms` reaches
/// `expiry_ms`; nothing else in the RTC step reads the absolute clock
/// (events, guards, context, completion are clock-independent). Hence two
/// configurations that are identical in active states / history / defer /
/// context / nested-sub-structure and have the **same remaining duration**
/// for every armed timer are *strongly bisimilar*: from either, advancing
/// by the same δ fires the same timer with the same effect, and every
/// event/completion edge is identical. A uniform shift of (clock + all
/// expiries) is exactly such an equivalence. Merging them is therefore
/// sound for reachability **and** deadlock (the merged class has the
/// identical successor set), and is in fact *required* for correctness:
/// without it, a `every_internal` heartbeat or any timer cycle advances
/// the clock forever, the digest never repeats, and a **genuine
/// timer-deadlock is never detected** (verdict wrongly `Inconclusive`
/// instead of `Deadlock` — observed empirically pre-fix). It never causes
/// a false `ProvenNoDeadlock`: it only merges configs whose entire future
/// behaviour is provably identical, so no reachable deadlock can be hidden
/// by the merge.
///
/// **Digest-soundness preserved.** Two configs with *different* remaining
/// timer durations normalise to *different* bytes (the relative phase is
/// kept). Two configs differing only in a nested sub-instance leaf / its
/// context / its relative timer phase normalise to different bytes (the
/// recursion preserves all nested structure). So the §4.1 conflation the
/// W2 P0 lossless-snapshot closed stays closed — the normalisation removes
/// only the *non-observable clock origin*, never observable state.
///
/// **Also zeroes the nested `next_trace_id`** (the W2 submachine
/// termination guard). A sub-instance carries its *own* `next_trace_id`
/// emit counter, incremented on every sub-RTC step. It is book-keeping,
/// **not** configuration — exactly like the parent's (see
/// [`canonical_bytes`] rationale 1). If it were keyed, a sub reached via
/// different-length delegated-event paths (e.g. across `RECONNECT`
/// teardown/re-create cycles) would never collide, the visited set would
/// never converge, and a sound submachine model would explode to a false
/// `Inconclusive` (observed: 100k configs / bound-hit before this fix).
/// The exclusion must be applied **recursively** — the parent-only
/// zeroing in `canonical_bytes` does not reach nested counters.
fn normalize_clock_origin_sub(sub: &mut SubmachineSnapshot) {
    let origin = sub.virtual_clock_ms;
    sub.virtual_clock_ms = 0;
    sub.next_trace_id = 0;
    for t in &mut sub.timers {
        t.expiry_ms = t.expiry_ms.saturating_sub(origin);
    }
    for nested in sub.submachines.values_mut() {
        normalize_clock_origin_sub(nested);
    }
}

/// Canonical, semantics-relevant byte form of a snapshot.
///
/// Two normalisations make the digest key the *observable* configuration
/// exactly (no more, no less):
///
/// 1. **`next_trace_id` zeroed.** A monotonically-increasing emit counter;
///    behaviourally-identical configs reached via different path lengths
///    have different counters. Keying it would defeat the visited set and
///    break termination. (`initialized` is always `true` post-`init`, so
///    it is constant and left as-is.)
/// 2. **Clock-origin normalised** (the W2 timer-soundness fix —
///    [`normalize_clock_origin_sub`] for the rationale + bisimulation
///    proof): the absolute virtual-clock value is non-observable; only
///    *relative* timer phase is. We zero `virtual_clock_ms` and rewrite
///    every armed timer to its remaining duration, recursively for nested
///    sub-instances. This merges exactly the clock-shift-equivalent (=
///    strongly bisimilar) configs, which is sound for reachability +
///    deadlock and required so timer cycles / heartbeats terminate and
///    real timer-deadlocks are detected (never a false proven).
///
/// Everything else (`active_states`, `history`, `defer_set`, `context`,
/// the *relative* timer set, the recursive `submachines`) *is* the
/// observable configuration and is keyed.
fn canonical_bytes(snap: &InterpreterSnapshot) -> Vec<u8> {
    let mut s = snap.clone();
    s.next_trace_id = 0;
    // Clock-origin normalisation (sound — see `normalize_clock_origin_sub`).
    let origin = s.virtual_clock_ms;
    s.virtual_clock_ms = 0;
    for t in &mut s.timers {
        t.expiry_ms = t.expiry_ms.saturating_sub(origin);
    }
    for sub in s.submachines.values_mut() {
        normalize_clock_origin_sub(sub);
    }
    // `serde_json::to_vec` over an all-BTreeMap/Vec/scalar value is
    // deterministic (sorted map keys, fixed array order) — the Doc 13 §11
    // byte-exactness contract. `expect` is sound: the snapshot derives
    // `Serialize` over only JSON-trivial types and cannot fail to encode.
    serde_json::to_vec(&s).expect("InterpreterSnapshot is always JSON-encodable")
}

impl ConfigDigest {
    pub(crate) fn of(snap: &InterpreterSnapshot) -> Self {
        let bytes = canonical_bytes(snap);
        let mut h1 = DefaultHasher::new();
        bytes.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        // Salt the second hasher so `hi` and `lo` are not the same value;
        // a differing prefix gives two independent hash streams over the
        // identical payload.
        0xC0FFEE_u64.hash(&mut h2);
        bytes.hash(&mut h2);
        Self {
            hi: h1.finish(),
            lo: h2.finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsm_simulator::{Timer, TimerFire};
    use std::collections::BTreeMap;

    fn snap(states: &[&str], clock: u64) -> InterpreterSnapshot {
        InterpreterSnapshot {
            active_states: states.iter().map(|s| s.to_string()).collect(),
            history: BTreeMap::new(),
            defer_set: vec![],
            virtual_clock_ms: clock,
            timers: vec![],
            context: BTreeMap::new(),
            next_trace_id: 0,
            initialized: true,
            submachines: BTreeMap::new(),
        }
    }

    fn timer(owner: &str, expiry: u64) -> Timer {
        Timer {
            timer_id: format!("tm-{owner}"),
            source_state: owner.into(),
            transition_id: Some("tr".into()),
            expiry_ms: expiry,
            fires: TimerFire::OneShot,
        }
    }

    fn sub(states: &[&str], clock: u64, timers: Vec<Timer>) -> SubmachineSnapshot {
        SubmachineSnapshot {
            active_states: states.iter().map(|s| s.to_string()).collect(),
            history: BTreeMap::new(),
            defer_set: vec![],
            virtual_clock_ms: clock,
            timers,
            context: BTreeMap::new(),
            next_trace_id: 0,
            initialized: true,
            submachines: BTreeMap::new(),
        }
    }

    #[test]
    fn identical_configs_collide() {
        assert_eq!(
            ConfigDigest::of(&snap(&["s-a"], 0)),
            ConfigDigest::of(&snap(&["s-a"], 0))
        );
    }

    #[test]
    fn distinct_active_states_differ() {
        assert_ne!(
            ConfigDigest::of(&snap(&["s-a"], 0)),
            ConfigDigest::of(&snap(&["s-b"], 0))
        );
    }

    #[test]
    fn clock_shift_equivalent_configs_collide() {
        // W2 sound clock-origin normalisation (Doc 08 §13.5): with NO armed
        // timer the absolute clock is non-observable, so the same active
        // state at t=0 and t=500 is the SAME reachable behaviour and MUST
        // collide. (W1 keyed absolute clock — a conservative
        // over-approximation that, for W2's timer scope, prevents cyclic
        // timers / heartbeats from ever converging and blocks genuine
        // timer-deadlock detection. The merge is sound: identical future.)
        assert_eq!(
            ConfigDigest::of(&snap(&["s-a"], 0)),
            ConfigDigest::of(&snap(&["s-a"], 500)),
            "clock-shift-equivalent configs (no armed timer) are bisimilar \
             and must collide — the sound W2 normalisation"
        );

        // And with timers: same state, timer with the SAME REMAINING
        // duration but a shifted absolute origin ⇒ bisimilar ⇒ collide.
        let mut a = snap(&["s-a"], 100);
        a.timers = vec![timer("s-a", 1100)]; // remaining 1000
        let mut b = snap(&["s-a"], 700);
        b.timers = vec![timer("s-a", 1700)]; // remaining 1000
        assert_eq!(
            ConfigDigest::of(&a),
            ConfigDigest::of(&b),
            "same state + same REMAINING timer duration (shifted origin) is \
             bisimilar and must collide"
        );
    }

    #[test]
    fn distinct_relative_timer_phase_differs() {
        // Digest-soundness (the §4.1 conflation guard, timer axis): two
        // configs in the SAME active state differing ONLY in the armed
        // timer's REMAINING duration are behaviourally distinct (a
        // timer-fire edge progresses at a different δ) and MUST hash
        // DISTINCT — else the explorer prunes a genuinely-unexplored config
        // → false `ProvenNoDeadlock`.
        let mut near = snap(&["s-a"], 0);
        near.timers = vec![timer("s-a", 100)]; // remaining 100
        let mut far = snap(&["s-a"], 0);
        far.timers = vec![timer("s-a", 9000)]; // remaining 9000
        assert_ne!(
            ConfigDigest::of(&near),
            ConfigDigest::of(&far),
            "different remaining timer durations are behaviourally distinct \
             and must hash distinct (digest-soundness, timer axis)"
        );

        // An armed-timer config vs a no-timer config in the same state must
        // also differ (one has a timer-fire edge, the other does not).
        assert_ne!(
            ConfigDigest::of(&near),
            ConfigDigest::of(&snap(&["s-a"], 0)),
            "armed-timer vs no-timer in the same state must hash distinct"
        );
    }

    #[test]
    fn distinct_nested_subinstance_state_differs() {
        // Digest-soundness (the §4.1 conflation guard, submachine axis —
        // the W2 P0 lossless-snapshot's whole purpose): two configs whose
        // PARENT is identical but whose nested sub-instance is in a
        // different leaf MUST hash DISTINCT. Pre-W2-P0 the snapshot dropped
        // `submachines` so these were identical bytes → the exact
        // conflation that yields a false `ProvenNoDeadlock`.
        let mut a = snap(&["s-parent-ref"], 0);
        a.submachines
            .insert("s-parent-ref".into(), sub(&["s-sub-Idle"], 0, vec![]));
        let mut b = snap(&["s-parent-ref"], 0);
        b.submachines
            .insert("s-parent-ref".into(), sub(&["s-sub-Running"], 0, vec![]));
        assert_ne!(
            ConfigDigest::of(&a),
            ConfigDigest::of(&b),
            "configs differing ONLY in the nested sub-instance leaf must \
             hash distinct (digest-soundness, submachine axis)"
        );

        // Also: differ only in the nested sub's RELATIVE timer phase.
        let mut c = snap(&["s-parent-ref"], 0);
        c.submachines.insert(
            "s-parent-ref".into(),
            sub(&["s-sub-Wait"], 0, vec![timer("s-sub-Wait", 500)]),
        );
        let mut d = snap(&["s-parent-ref"], 0);
        d.submachines.insert(
            "s-parent-ref".into(),
            sub(&["s-sub-Wait"], 0, vec![timer("s-sub-Wait", 5000)]),
        );
        assert_ne!(
            ConfigDigest::of(&c),
            ConfigDigest::of(&d),
            "configs differing ONLY in a nested sub-instance's relative \
             timer phase must hash distinct"
        );

        // But a clock-shift of the WHOLE nested sub (same relative phase)
        // collides — the recursion normalises each sub by its own origin.
        let mut e = snap(&["s-parent-ref"], 0);
        e.submachines.insert(
            "s-parent-ref".into(),
            sub(&["s-sub-Wait"], 200, vec![timer("s-sub-Wait", 700)]), // rem 500
        );
        assert_eq!(
            ConfigDigest::of(&c),
            ConfigDigest::of(&e),
            "nested sub clock-shift with identical relative phase is \
             bisimilar and must collide"
        );
    }

    #[test]
    fn trace_id_is_not_part_of_the_key() {
        // The cardinal termination guard: behaviourally-identical configs
        // reached via different path lengths (hence different trace-id
        // counters) MUST collide, or the visited set never converges.
        let mut a = snap(&["s-a"], 0);
        let mut b = snap(&["s-a"], 0);
        a.next_trace_id = 1;
        b.next_trace_id = 999;
        assert_eq!(ConfigDigest::of(&a), ConfigDigest::of(&b));
    }

    #[test]
    fn distinct_context_differs() {
        let mut with_ctx = snap(&["s-a"], 0);
        with_ctx
            .context
            .insert("count".into(), fsm_simulator::Value::I32(3));
        assert_ne!(
            ConfigDigest::of(&snap(&["s-a"], 0)),
            ConfigDigest::of(&with_ctx)
        );
    }
}
