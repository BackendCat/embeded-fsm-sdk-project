//! Virtual-clock timer set — Doc 08 §13 timer lifecycle.
//!
//! A [`Timer`] is created when its owning state is entered (Doc 08 §13.1)
//! and cancelled when the owning state is exited (§13.2). `every` (periodic)
//! timers re-arm to `last_scheduled_fire + period` so drift is zero (§13.3).
//!
//! The simulator does not use a `BinaryHeap` — for v1.0 we keep a `Vec`
//! sorted-on-access. The expected timer count per state is 1-3; the cost of
//! a linear scan is far below the cost of the surrounding event loop, and
//! keeping mutation simple avoids re-heapification headaches when a state
//! cancels one of its timers due to an external transition.
//!
//! [`Timer`] / [`TimerFire`] derive `Serialize`/`Deserialize` so the armed
//! set is part of [`crate::InterpreterSnapshot`] (v1.4-W2): the armed
//! timers *are* configuration — two otherwise-identical configs that differ
//! only in which timers are armed (or in remaining timer phase) are
//! behaviourally distinct (a timer-fire edge can make progress from one and
//! not the other), so the verifier's visited-set key must not conflate
//! them. Every field is a serde-trivial scalar/`String`/`Option`/enum, so
//! the Doc 13 §11 byte-determinism contract holds (no `HashMap`).

use serde::{Deserialize, Serialize};

/// A single armed timer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timer {
    /// Source-of-truth: the timer ID from the IR. Trace records reference
    /// this back to the IR `TimerObject.id`.
    pub timer_id: String,
    /// Owning state. The timer is cancelled when this state is exited.
    pub source_state: String,
    /// Transition that fires when the timer expires. `None` for
    /// `every_internal` timers that have no transition (Doc 09 §13 — only
    /// actions are run).
    pub transition_id: Option<String>,
    /// Absolute virtual-clock time at which the next fire is scheduled.
    pub expiry_ms: u64,
    /// One-shot or periodic.
    pub fires: TimerFire,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimerFire {
    OneShot,
    Periodic { period_ms: u32 },
}

/// Total-order key for [`TimerFire`] used only by
/// [`TimerSet::snapshot_sorted`] so the canonical armed-set form is stable
/// across runs. `(discriminant, period)` — `OneShot` sorts before any
/// `Periodic`; `Periodic`s order by period.
fn fire_ord(f: &TimerFire) -> (u8, u32) {
    match f {
        TimerFire::OneShot => (0, 0),
        TimerFire::Periodic { period_ms } => (1, *period_ms),
    }
}

/// In-flight timer set. We keep a `Vec` rather than a `BinaryHeap` so that
/// cancellation by state id is cheap (no `re-heapify`). The "next expiry"
/// scan is O(N) — acceptable when N is the count of timers armed
/// simultaneously across the active configuration.
#[derive(Clone, Debug, Default)]
pub struct TimerSet {
    timers: Vec<Timer>,
}

impl TimerSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn arm(&mut self, t: Timer) {
        self.timers.push(t);
    }

    /// Cancel every timer whose owning state matches `state_id`. Doc 08
    /// §13.2: a timer is cancelled exactly when its owning state is exited.
    pub fn cancel_owned_by(&mut self, state_id: &str) {
        self.timers.retain(|t| t.source_state != state_id);
    }

    /// All currently armed timers (for snapshot / debugging).
    pub fn iter(&self) -> impl Iterator<Item = &Timer> {
        self.timers.iter()
    }

    /// Canonical, byte-deterministic armed-set form for
    /// [`crate::InterpreterSnapshot`] (v1.4-W2). The live `timers` `Vec`
    /// preserves *arm order* (insertion order is irrelevant to firing —
    /// `pop_fired_through` sorts by expiry — but is observable order); the
    /// snapshot returns a **sorted clone** so two configurations with the
    /// same armed set reached via different arm orders serialise to
    /// identical bytes (the digest must key the *set*, not the arm
    /// sequence, or the visited-set would split behaviourally-identical
    /// configs and never converge — the same termination guard
    /// `next_trace_id` exclusion serves for the parent counter). The sort
    /// key is the full timer tuple (`timer_id`, `source_state`,
    /// `expiry_ms`, …) so it is total and stable.
    pub fn snapshot_sorted(&self) -> Vec<Timer> {
        let mut v = self.timers.clone();
        v.sort_by(|a, b| {
            (
                &a.timer_id,
                &a.source_state,
                &a.transition_id,
                a.expiry_ms,
                fire_ord(&a.fires),
            )
                .cmp(&(
                    &b.timer_id,
                    &b.source_state,
                    &b.transition_id,
                    b.expiry_ms,
                    fire_ord(&b.fires),
                ))
        });
        v
    }

    /// Replace the armed set wholesale from a previously captured snapshot
    /// (the [`TimerSet`] half of `Interpreter::restore`). Arm order is not
    /// load-bearing for firing, so restoring the sorted form is
    /// behaviourally exact.
    pub fn restore_from(&mut self, timers: Vec<Timer>) {
        self.timers = timers;
    }

    pub fn is_empty(&self) -> bool {
        self.timers.is_empty()
    }

    /// Find the soonest expiry, if any.
    pub fn next_expiry_ms(&self) -> Option<u64> {
        self.timers.iter().map(|t| t.expiry_ms).min()
    }

    /// Pop every timer whose `expiry_ms <= now`, in chronological order.
    /// `Periodic` timers are re-armed with `expiry_ms += period_ms` per Doc
    /// 08 §13.3 to prevent drift; they reappear at the back of the returned
    /// vec at their new scheduled time only if they are still pending after
    /// re-arming.
    ///
    /// Returns the fired timers as cloned values (the caller dispatches a
    /// synthetic event for each).
    pub fn pop_fired_through(&mut self, now: u64) -> Vec<Timer> {
        // Step 1 — pull out everything that fires by `now`.
        let mut fired = Vec::new();
        let mut keep: Vec<Timer> = Vec::with_capacity(self.timers.len());
        let drained = std::mem::take(&mut self.timers);
        for t in drained {
            if t.expiry_ms <= now {
                fired.push(t);
            } else {
                keep.push(t);
            }
        }
        self.timers = keep;

        // Step 2 — sort fired by expiry to deliver in chronological order.
        fired.sort_by_key(|t| t.expiry_ms);

        // Step 3 — for each periodic timer, re-arm at the next scheduled
        // moment. Multiple intervals may have elapsed in one tick; we fire
        // ONCE per call and schedule the next fire to absorb missed ones
        // forward of `now`. (Doc 08 §13.3: "from the scheduled moment to
        // prevent drift".)
        for f in &fired {
            if let TimerFire::Periodic { period_ms } = f.fires {
                let mut next = f.expiry_ms + period_ms as u64;
                while next <= now {
                    next += period_ms as u64;
                }
                self.timers.push(Timer {
                    expiry_ms: next,
                    ..f.clone()
                });
            }
        }

        fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: &str, owner: &str, expiry: u64, fires: TimerFire) -> Timer {
        Timer {
            timer_id: id.into(),
            source_state: owner.into(),
            transition_id: Some("tr".into()),
            expiry_ms: expiry,
            fires,
        }
    }

    #[test]
    fn pop_fired_returns_chronological_order() {
        let mut ts = TimerSet::new();
        ts.arm(t("late", "S", 200, TimerFire::OneShot));
        ts.arm(t("early", "S", 50, TimerFire::OneShot));
        ts.arm(t("mid", "S", 100, TimerFire::OneShot));

        let fired = ts.pop_fired_through(150);
        assert_eq!(fired.len(), 2);
        assert_eq!(fired[0].timer_id, "early");
        assert_eq!(fired[1].timer_id, "mid");
        // 'late' still pending.
        assert_eq!(ts.next_expiry_ms(), Some(200));
    }

    #[test]
    fn periodic_re_arms_at_scheduled_moment() {
        let mut ts = TimerSet::new();
        ts.arm(t("p", "S", 100, TimerFire::Periodic { period_ms: 100 }));
        // First fire at 100; next scheduled at 200.
        let fired = ts.pop_fired_through(150);
        assert_eq!(fired.len(), 1);
        assert_eq!(ts.next_expiry_ms(), Some(200));
        // Advance past 500 ms — re-arming should chase the schedule, not
        // pile up missed fires.
        let _ = ts.pop_fired_through(550);
        assert_eq!(ts.next_expiry_ms(), Some(600));
    }

    #[test]
    fn cancel_owned_by_removes_all_matching() {
        let mut ts = TimerSet::new();
        ts.arm(t("a", "S", 100, TimerFire::OneShot));
        ts.arm(t("b", "S", 200, TimerFire::OneShot));
        ts.arm(t("c", "OTHER", 300, TimerFire::OneShot));
        ts.cancel_owned_by("S");
        let remaining: Vec<_> = ts.iter().map(|t| t.timer_id.clone()).collect();
        assert_eq!(remaining, vec!["c".to_string()]);
    }
}
