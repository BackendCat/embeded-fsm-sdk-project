//! `fsm-formatter` — canonical formatter for FSM-Lang source.
//!
//! Per Doc 20 §12.9 / Doc 23 §4 the formatter walks the **CST**
//! (lossless green tree) produced by `fsm-parser`. It does NOT depend on
//! `fsm-analyzer`, `fsm-ir`, or any code-generator crate.
//!
//! The cardinal property is **idempotency**:
//!
//! ```text
//! format(format(src)) == format(src)
//! ```
//!
//! enforced by `tests/idempotency.rs` over a corpus of representative
//! fixtures spanning every major Doc 04 grammar form.
//!
//! ## Grammar reality vs. Doc 19
//!
//! Doc 19 ("Formatter Specification") was written before the v1.0 grammar
//! settled, so several of its example outputs use syntax that does not
//! parse against Doc 04 §1.5 / §13. Per
//! `docs/00-Decisions-And-Reconciliation.md` Section 5 G-12 and Section 8
//! ("Doc 19 patches"), the formatter implementation follows
//! **Doc 04 (the grammar)**. Resolved disagreements:
//!
//! - `composite NAME { … }` (Doc 19 §12) — does **not** exist in Doc 04.
//!   Composite states are ordinary `state NAME { … }` blocks whose body
//!   contains nested `state_decl`s. Emitted as such.
//! - `parallel NAME { region R { … } }` (Doc 19 §12) — likewise no
//!   `parallel` keyword in Doc 04 grammar (it exists only as a feature-
//!   flag identifier per §2.2). The grammar shape is `state NAME { region
//!   R { … } region R2 { … } }`. Emitted as such.
//! - `history shallow` / `history deep` (Doc 19 §12) — Doc 04 §1.5 has
//!   single-token keywords `shallow_history` / `deep_history`. The
//!   formatter emits `shallow_history NAME { initial Target }`.
//! - `internal on EVENT:` (Doc 19 §10) — Doc 04 §8.2 defines internal
//!   transitions as `on EVENT [g] : action` with **no** `internal`
//!   keyword. Emitted as such.
//! - Field declarations carry **no trailing `;`** (Doc 04 §4.1 EBNF). The
//!   Doc 19 §5 example uses semicolons; the formatter omits them.
//! - Doc 19 §16's "canonical example" includes an `initial -> Idle` form;
//!   Doc 04 §4.5 defines `initial_decl = "initial" , identifier` with no
//!   arrow. Formatter emits `initial Idle`.
//!
//! ## Public API
//!
//! - [`format_string`] — convenience entry: parse, format, re-emit.
//! - [`format`] — format an already-parsed CST root.
//! - [`FormatOptions`] — knobs (indent width, max line width, etc.).

#![forbid(unsafe_code)]
#![deny(missing_debug_implementations)]
// Surface accidentally-dropped Results — the formatter swallows nothing.
#![warn(unused_must_use)]

pub use fsm_parser::SyntaxNode;

pub mod format;
pub mod options;

pub use format::{format, format_string, FormatError};
pub use options::FormatOptions;
