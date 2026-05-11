//! Grammar rules, one file per top-level area.
//!
//! Each module exports rule functions named after the production they
//! implement. Rules call `Parser` methods (start_node / bump / expect /
//! error_until) and possibly delegate to sister rules. Sync-set TokenSets
//! are defined as const fields near the rules that use them so panic-mode
//! recovery is a one-liner.

pub mod file;
pub mod guard;
pub mod machine;
pub mod state;
pub mod stmt;
pub mod top_level;
pub mod transition;

/// Convenience re-export so call sites can use `grammar::parse_file(p)` from
/// the public entry point in `parse.rs`.
pub use file::parse_file;
