//! Bounded event queue with `OverflowPolicy` handling.
//!
//! Doc 00 §7.4 item 3 + Doc 04 §9 specify per-machine queue config. The
//! capacity comes from `MachineObject.queue.capacity`; the four overflow
//! policies — `Assert`, `DropOldest`, `DropNewest`, `Error` — come from
//! `OverflowPolicy`. The simulator maps them as follows:
//!
//! | Policy       | Behaviour on push to full queue                           |
//! |--------------|-----------------------------------------------------------|
//! | `Assert`     | Returns [`QueueError::Overflow`]; caller decides what to do |
//! | `DropOldest` | Removes the front element, pushes the new one at the back |
//! | `DropNewest` | Drops the new element silently                            |
//! | `Error`      | Returns [`QueueError::Overflow`]                          |
//!
//! `push_front` (used by `raise` and completion events, Doc 08 §14) shares
//! the same capacity but does not honour `DropOldest` — it always returns
//! `Overflow` on a full queue, because dropping a queue-front element by
//! definition cancels the head-of-line invariant for internal events.

use std::collections::VecDeque;

use fsm_ir::OverflowPolicy;
use thiserror::Error;

use super::event::QueuedEvent;

#[derive(Debug, Error)]
pub enum QueueError {
    /// Bound was exceeded and the policy rejected the push.
    #[error("event queue overflow (capacity {capacity})")]
    Overflow { capacity: usize },
}

#[derive(Debug, Clone)]
pub struct EventQueue {
    inner: VecDeque<QueuedEvent>,
    capacity: usize,
    overflow: OverflowPolicy,
}

impl EventQueue {
    pub fn new(capacity: u32, overflow: OverflowPolicy) -> Self {
        // Treat 0 as "unbounded" in the simulator — Doc 04 §9 lets the user
        // omit capacity for sim-only machines.
        let cap = if capacity == 0 {
            usize::MAX
        } else {
            capacity as usize
        };
        Self {
            inner: VecDeque::new(),
            capacity: cap,
            overflow,
        }
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Front-to-back iterator over queued events without consuming them.
    /// Used by the submachine sync (Doc 08 §12) to make the parent
    /// `Completion(ref_state)` enqueue idempotent — a sub that stays Final
    /// until its ref-state exits must enqueue its completion exactly once,
    /// otherwise every sync re-enqueues it and trips the §9.4
    /// completion-loop cap.
    pub fn iter(&self) -> impl Iterator<Item = &QueuedEvent> {
        self.inner.iter()
    }

    pub fn pop_front(&mut self) -> Option<QueuedEvent> {
        self.inner.pop_front()
    }

    /// External event push — honours overflow policy.
    pub fn push_back(&mut self, ev: QueuedEvent) -> Result<(), QueueError> {
        if self.inner.len() >= self.capacity {
            match self.overflow {
                OverflowPolicy::DropOldest => {
                    let _ = self.inner.pop_front();
                    self.inner.push_back(ev);
                    return Ok(());
                }
                OverflowPolicy::DropNewest => return Ok(()),
                OverflowPolicy::Assert | OverflowPolicy::Error => {
                    return Err(QueueError::Overflow {
                        capacity: self.capacity,
                    });
                }
            }
        }
        self.inner.push_back(ev);
        Ok(())
    }

    /// Internal event push — never drops the head. Doc 08 §14 puts `raise`,
    /// completion, and released deferred events at the front of the queue.
    pub fn push_front(&mut self, ev: QueuedEvent) -> Result<(), QueueError> {
        if self.inner.len() >= self.capacity {
            return Err(QueueError::Overflow {
                capacity: self.capacity,
            });
        }
        self.inner.push_front(ev);
        Ok(())
    }

    /// Bulk prepend in FIFO order (Doc 08 §10.3 — released deferred events).
    pub fn prepend(
        &mut self,
        evs: impl IntoIterator<Item = QueuedEvent>,
    ) -> Result<(), QueueError> {
        for ev in evs.into_iter().collect::<Vec<_>>().into_iter().rev() {
            self.push_front(ev)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::event::{EventKind, QueuedEvent};

    fn ev(name: &str) -> QueuedEvent {
        QueuedEvent::new(
            EventKind::Dispatched {
                event_id: name.into(),
            },
            None,
        )
    }

    #[test]
    fn assert_policy_returns_overflow() {
        let mut q = EventQueue::new(1, OverflowPolicy::Assert);
        q.push_back(ev("A")).unwrap();
        let err = q.push_back(ev("B")).unwrap_err();
        assert!(matches!(err, QueueError::Overflow { capacity: 1 }));
    }

    #[test]
    fn drop_oldest_evicts_head() {
        let mut q = EventQueue::new(2, OverflowPolicy::DropOldest);
        q.push_back(ev("A")).unwrap();
        q.push_back(ev("B")).unwrap();
        q.push_back(ev("C")).unwrap();
        // A was dropped, B and C remain.
        let first = q.pop_front().unwrap();
        assert_eq!(first.kind.event_id(), Some("B"));
        let second = q.pop_front().unwrap();
        assert_eq!(second.kind.event_id(), Some("C"));
    }

    #[test]
    fn drop_newest_silently_discards() {
        let mut q = EventQueue::new(1, OverflowPolicy::DropNewest);
        q.push_back(ev("A")).unwrap();
        q.push_back(ev("B")).unwrap();
        let head = q.pop_front().unwrap();
        assert_eq!(head.kind.event_id(), Some("A"));
        assert!(q.is_empty());
    }

    #[test]
    fn unbounded_capacity_when_zero() {
        let mut q = EventQueue::new(0, OverflowPolicy::Assert);
        for i in 0..1000 {
            q.push_back(ev(&format!("e{i}"))).unwrap();
        }
        assert_eq!(q.len(), 1000);
    }
}
