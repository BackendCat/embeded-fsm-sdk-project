//! Host-side extern stubs for the interpreter.
//!
//! The simulator does not compile / link against generated C, so every
//! `extern` declared in the IR needs a Rust callback to evaluate guards and
//! action `call`s. Tests register stubs via [`ExternRegistry::register`];
//! production callers can wire up real implementations.
//!
//! Unknown externs default to:
//! - `Bool(false)` for guard-context calls (so an un-registered guard never
//!   accidentally enables a transition).
//! - `I32(0)` for action-context calls.

use std::collections::HashMap;
use std::sync::Arc;

use crate::runtime::value::Value;

/// Boxed Rust closure that simulates an extern call. Receives the argument
/// values in declaration order and returns the result.
pub type ExternFn = Arc<dyn Fn(&[Value]) -> Value + Send + Sync>;

#[derive(Clone, Default)]
pub struct ExternRegistry {
    handlers: HashMap<String, ExternFn>,
}

impl std::fmt::Debug for ExternRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternRegistry")
            .field(
                "registered_names",
                &self.handlers.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl ExternRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<F>(&mut self, name: &str, f: F)
    where
        F: Fn(&[Value]) -> Value + Send + Sync + 'static,
    {
        self.handlers.insert(name.to_string(), Arc::new(f));
    }

    pub fn invoke(&self, name: &str, args: &[Value], guard_context: bool) -> Value {
        if let Some(h) = self.handlers.get(name) {
            return h(args);
        }
        if guard_context {
            Value::Bool(false)
        } else {
            Value::I32(0)
        }
    }
}
