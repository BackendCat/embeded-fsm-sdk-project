//! Lexical scope stack used during name resolution and checks.
//!
//! Scope semantics in FSM-Lang are simple — `machine` is the outermost scope
//! that owns events, externs, consts, and states; nested `state` and `region`
//! blocks introduce inner scopes that can shadow the machine-level state names
//! but cannot redeclare events, externs, or context fields.

/// A point in the scope stack — identifies which container declared a name and
/// is consulted by [`crate::symbol_table::SymbolTable`] when resolving
/// references.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Scope {
    /// Index of the enclosing machine in [`crate::symbol_table::SymbolTable::machines`].
    pub machine_idx: Option<usize>,
    /// Path of state / region container IDs walked from the machine root down
    /// to the current declaration. The last element is the immediate parent.
    pub container_path: Vec<String>,
}

impl Scope {
    /// Root scope — no enclosing machine yet.
    pub const fn root() -> Self {
        Self {
            machine_idx: None,
            container_path: Vec::new(),
        }
    }

    /// Construct a scope rooted inside machine index `m` with no inner
    /// containers entered yet.
    pub const fn for_machine(m: usize) -> Self {
        Self {
            machine_idx: Some(m),
            container_path: Vec::new(),
        }
    }

    /// Return a clone of this scope with `container` pushed onto the path.
    pub fn push(&self, container: impl Into<String>) -> Self {
        let mut next = self.clone();
        next.container_path.push(container.into());
        next
    }
}
