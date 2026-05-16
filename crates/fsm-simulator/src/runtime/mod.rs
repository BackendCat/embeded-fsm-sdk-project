//! Runtime building blocks shared across interpreter modules.
//!
//! Pure data + a thin set of helpers. No event-loop logic lives here — that
//! is centralized in [`crate::interpreter`].

pub mod completion;
pub mod event;
pub mod lca;
pub mod machine_index;
pub mod queue;
pub mod state;
pub mod submachine;
pub mod timer;
pub mod value;

pub use completion::{active_leaf_in_region, check_and_enqueue_completion};
pub use event::{EventKind, QueuedEvent};
pub use lca::{effective_lca, lca_inclusive};
pub use machine_index::{MachineIndex, NodeKind, NodeRef, RegionRef};
pub use queue::{EventQueue, QueueError};
pub use state::{ContextValues, InterpreterSnapshot, RuntimeState, SubmachineSnapshot};
pub use submachine::{build_sub_runtime, entry_target, sub_reached_final, MAX_SUBMACHINE_DEPTH};
pub use timer::{Timer, TimerFire, TimerSet};
pub use value::Value;
