//! Action-sublanguage style lint — Doc 04 §8.7.2 / Doc 02 §9.2 /
//! Doc 10 §FSM-W0200.
//!
//! Loops are **permitted** in action blocks (Doc 02 §9.2: prohibiting them
//! while allowing `extern` calls — which may themselves loop — would be a
//! leaky abstraction). The compiler accepts them and still generates code
//! (Doc 11 §18: "Inline while/for loops generate directly as C loops"). It
//! emits **`FSM-W0200`** as a style nudge: complex looping logic is usually
//! clearer in a named `extern` function where the embedded toolchain can do
//! bounded-execution / WCET analysis.
//!
//! This is the single emission site for `FSM-W0200`. Before this module the
//! code was catalog-reserved with zero emitters (Doc 00 §11.38 noted it as
//! never emitted); IMPLEMENT was chosen — the corpus specifies it as a real
//! intended compiler diagnostic in four normative docs (Doc 02 §9.2 "The
//! compiler emits `FSM-W0200` … when a loop appears in an action block",
//! Doc 04 §8.7.2, Doc 11 §18, Doc 10 §FSM-W0200's full catalog entry with an
//! exact `Message`), and the conformance suite (Doc 15 §5) reserves a
//! fixture for it. RETIRE would contradict those.
//!
//! Detection is purely structural: a `STMT_WHILE` / `STMT_FOR` whose
//! ancestor chain contains an `ACTION_BLOCK`. Loops only ever parse inside
//! an action block in FSM-Lang (the grammar has no other statement context),
//! but anchoring on the `ACTION_BLOCK` ancestor keeps the rule's intent
//! explicit and robust to future grammar growth.

use fsm_diagnostics::{Diagnostic, DiagnosticCode};
use fsm_parser::ast::{self, AstNode};
use fsm_parser::cst::SyntaxKind;

use crate::symbol_table::SymbolTable;
use crate::util::span_of;

/// Canonical message for `FSM-W0200` — verbatim from Doc 10 §FSM-W0200
/// ("Message" row). Kept as a single const so the catalog text has exactly
/// one source.
const W0200_MESSAGE: &str = "Loop in action block. Consider extracting to a named extern function \
     for bounded-execution analysis.";

/// Run the action-block style lint. Appends one `FSM-W0200` per loop
/// statement found inside an action block.
pub fn check(file: &ast::File, _st: &SymbolTable, out: &mut Vec<Diagnostic>) {
    for machine in file.machines() {
        for node in machine.syntax().descendants() {
            match node.kind() {
                SyntaxKind::STMT_WHILE | SyntaxKind::STMT_FOR => {
                    if in_action_block(&node) {
                        out.push(
                            Diagnostic::new(DiagnosticCode::W0200, span_of(&node))
                                .with_message(W0200_MESSAGE),
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

/// True iff `node` has an `ACTION_BLOCK` somewhere in its ancestor chain.
fn in_action_block(node: &fsm_parser::cst::SyntaxNode) -> bool {
    node.ancestors()
        .any(|a| a.kind() == SyntaxKind::ACTION_BLOCK)
}
