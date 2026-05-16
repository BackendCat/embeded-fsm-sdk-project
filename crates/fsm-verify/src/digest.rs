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

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use fsm_simulator::InterpreterSnapshot;

/// A stable 128-bit-ish configuration key. Two snapshots collide here iff
/// their canonical JSON encodings are byte-identical — i.e. iff they are
/// the *same* reachable configuration (same active leaves, history, defer
/// set, virtual clock, and context). `next_trace_id` and `initialized` are
/// part of the snapshot but are book-keeping, not configuration; see
/// [`canonical_bytes`] for why they are excluded from the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ConfigDigest {
    /// Two independent hashers over the same canonical bytes — widening the
    /// key to make an accidental collision astronomically unlikely without
    /// pulling a crypto-hash dependency into a disk-conscious workspace.
    hi: u64,
    lo: u64,
}

/// Canonical, semantics-relevant byte form of a snapshot.
///
/// `next_trace_id` is a monotonically-increasing emit counter: two
/// configurations that are behaviourally identical can be reached with
/// different trace-id counters (e.g. via paths of different length). If it
/// were in the key the explorer would treat the same configuration as new
/// every time, defeating the visited set and breaking termination. It is
/// therefore zeroed before serialisation. `initialized` is always `true`
/// for any explored configuration (we only explore post-`init`), so it is
/// constant and harmless either way; we leave it as serialised. Everything
/// else (`active_states`, `history`, `defer_set`, `virtual_clock_ms`,
/// `context`) *is* the configuration and is keyed.
fn canonical_bytes(snap: &InterpreterSnapshot) -> Vec<u8> {
    let mut s = snap.clone();
    s.next_trace_id = 0;
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
    use std::collections::BTreeMap;

    fn snap(states: &[&str], clock: u64) -> InterpreterSnapshot {
        InterpreterSnapshot {
            active_states: states.iter().map(|s| s.to_string()).collect(),
            history: BTreeMap::new(),
            defer_set: vec![],
            virtual_clock_ms: clock,
            context: BTreeMap::new(),
            next_trace_id: 0,
            initialized: true,
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
    fn distinct_virtual_clock_differs() {
        // Clock is part of the configuration (a timer-wait state at t=0 is
        // not the same reachable config as the same state at t=500).
        assert_ne!(
            ConfigDigest::of(&snap(&["s-a"], 0)),
            ConfigDigest::of(&snap(&["s-a"], 500))
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
