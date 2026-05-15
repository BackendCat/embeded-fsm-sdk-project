//! Grammar rules, one file per top-level area.
//!
//! Each module exports rule functions named after the production they
//! implement. Rules call `Parser` methods (start_node / bump / expect /
//! error_until) and possibly delegate to sister rules. Sync-set TokenSets
//! are defined as const fields near the rules that use them so panic-mode
//! recovery is a one-liner.

pub(crate) mod file;
pub(crate) mod guard;
pub(crate) mod machine;
pub(crate) mod state;
pub(crate) mod stmt;
pub(crate) mod top_level;
pub(crate) mod transition;

/// Convenience re-export so call sites can use `grammar::parse_file(p)` from
/// the public entry point in `parse.rs`.
pub(crate) use file::parse_file;
