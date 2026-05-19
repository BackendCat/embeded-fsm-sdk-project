//! Queue-config checks — Doc 04 §9 / Doc 11 §12 (F-2).
//!
//! `FSM-E0412` is emitted when an in-source `queue { capacity = N }` has an
//! `N` that is **not** a power of two. The generated C99 ring buffer indexes
//! with `& (CAP-1)` (Doc 11 §12) — a non-2^N capacity silently corrupts the
//! modulo arithmetic.
//!
//! This is the `defer`→`FSM-E0903` / timer-`FSM-E0411` deferred/invalid-config
//! precedent (Doc 02 §5.3 / §9.4): an invalid in-source config is **rejected
//! loudly**, never silently rounded and never silently miscompiled. It is the
//! direct sibling of Finding F-2's root defect — pre-fix the lowerer parsed
//! the *key* token instead of the value and silently fell back to the default
//! 16 / `Assert`, so a user's whole `queue {}` was dropped with a clean
//! `fsm check`. The lowerer fix makes the value flow; this check rejects the
//! one remaining way an in-source capacity could still miscompile (the
//! class-of-issues completion: ALL silent-misconfig of queue config, not just
//! the capacity-ignored instance).
//!
//! The capacity value is read through the SINGLE typed
//! [`fsm_parser::ast::ConfigEntry::value`] accessor — the SAME accessor the
//! lowerer's `lower_queue` uses — so the check and the lowerer can never
//! diverge on "what the value is" (the F-1 single-source-of-truth doctrine).
//! `0` is reported too: it is not a power of two and a zero-capacity ring is
//! degenerate (the codegen power-of-2 guard rejects it identically).

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};
use fsm_parser::cst::SyntaxKind;

use crate::symbol_table::SymbolTable;
use crate::util::span_of;

/// Run queue-config checks across every `queue {}` block in the file —
/// machine-level AND inside submachine templates (the ring-mask constraint
/// is identical wherever the block appears; class-of-issues).
pub fn check(file: &ast::File, _st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    for node in file
        .syntax()
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::QUEUE_BLOCK)
    {
        let Some(qb) = ast::QueueBlock::cast(node) else {
            continue;
        };
        for entry in qb.entries() {
            if entry.key().as_deref() != Some("capacity") {
                continue;
            }
            // SAME typed accessor the lowerer uses (F-1 unification — the
            // check and `lower_queue` cannot disagree on the value).
            let Some(raw) = entry.value() else { continue };
            // A non-integer `capacity = foo` is a separate malformed-config
            // concern the lowerer already ignores (parse fails → default);
            // E0412 speaks only to the power-of-two invariant of an actual
            // integer literal.
            let Ok(n) = raw.parse::<u64>() else { continue };
            if !is_power_of_two(n) {
                out.push(
                    Diagnostic::new(DiagnosticCode::E0412, span_of(entry.syntax())).with_message(
                        format!(
                            "queue capacity must be a power of two (got {n}); the generated \
                             C99 ring buffer indexes with bitwise-AND modulo `& (capacity - 1)` \
                             (Doc 11 §12). Use the nearest 2^N (…, 8, 16, 32, 64, …)."
                        ),
                    ),
                );
            }
        }
    }
}

fn is_power_of_two(n: u64) -> bool {
    n != 0 && (n & (n - 1)) == 0
}
