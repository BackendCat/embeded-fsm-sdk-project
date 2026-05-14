//! Formatter engine.
//!
//! Two entry points:
//! - [`format_string`] — parse + format. Errors if the source doesn't
//!   parse cleanly (Doc 19 §1.4 semantic-neutral guarantee).
//! - [`format`] — format a pre-parsed CST root. Always succeeds.
//!
//! Both accept a [`crate::FormatOptions`] reference; pass
//! `&FormatOptions::default()` for canonical defaults (Doc 19 §2 default
//! values).

use fsm_parser::{parse, SyntaxKind, SyntaxNode};

use crate::options::FormatOptions;

mod error;
mod expr;
mod machine;
mod state;
mod stmt;
mod top_level;
mod transition;
mod trivia;
mod writer;

pub use error::FormatError;

/// Format a CST root. The root SHOULD be a `FILE` node — for any other
/// kind, the formatter walks the subtree using best-effort dispatch. The
/// caller's writer settings are honoured exactly.
pub fn format(node: &SyntaxNode, opts: &FormatOptions) -> String {
    let mut w = writer::FormatWriter::new(opts);
    if node.kind() == SyntaxKind::FILE {
        top_level::emit_file(&mut w, node, opts);
    } else {
        // Subtree-level entry — useful for embedded callers (LSP code
        // action, REPL). Dispatch by kind.
        format_subtree(&mut w, node, opts);
    }
    w.finish()
}

/// Convenience: parse + format. Returns [`FormatError::ParseFailed`] if
/// the source did not parse cleanly.
pub fn format_string(src: &str, opts: &FormatOptions) -> Result<String, FormatError> {
    let result = parse(src);
    if !result.errors.is_empty() {
        return Err(FormatError::ParseFailed(result.errors));
    }
    let syntax = result.syntax();
    Ok(format(&syntax, opts))
}

fn format_subtree(w: &mut writer::FormatWriter, node: &SyntaxNode, opts: &FormatOptions) {
    match node.kind() {
        SyntaxKind::MACHINE_DECL => machine::emit_machine_decl(w, node, opts),
        SyntaxKind::STATE_DECL
        | SyntaxKind::REGION_DECL
        | SyntaxKind::CHOICE_DECL
        | SyntaxKind::JUNCTION_DECL
        | SyntaxKind::FORK_DECL
        | SyntaxKind::JOIN_DECL
        | SyntaxKind::SHALLOW_HISTORY_DECL
        | SyntaxKind::DEEP_HISTORY_DECL
        | SyntaxKind::INITIAL_DECL
        | SyntaxKind::FINAL_DECL
        | SyntaxKind::ENTRY_POINT_DECL
        | SyntaxKind::EXIT_POINT_DECL
        | SyntaxKind::ENTRY_DECL
        | SyntaxKind::EXIT_DECL
        | SyntaxKind::DEFER_DECL => state::emit_state_item(w, node, opts),
        k if transition::is_transition_kind(k) => {
            transition::emit_transition(w, node, None, opts);
        }
        _ => {
            // Last-resort: write raw text.
            w.write(node.text().to_string().trim());
        }
    }
}

// ─── Unit tests — exercises one minimal example per rule. ──────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(src: &str) -> String {
        format_string(src, &FormatOptions::default()).expect("parses + formats")
    }

    #[test]
    fn empty_file_is_just_newline() {
        let out = fmt("");
        assert_eq!(out, "\n");
    }

    #[test]
    fn language_header_alone() {
        let out = fmt("language fsm 2.0\n");
        assert_eq!(out, "language fsm 2.0\n");
    }

    #[test]
    fn idempotency_of_minimal_machine() {
        let src = "language fsm 2.0\nmachine M {\n}\n";
        let once = fmt(src);
        let twice = fmt(&once);
        assert_eq!(once, twice, "should be idempotent");
    }

    #[test]
    fn context_field_alignment() {
        let out = fmt(
            "language fsm 2.0\nmachine M { context { x : u8 = 0  longer_name : bool = false } }\n",
        );
        // Both names left-aligned, `:` aligned to widest+1 — `longer_name` is
        // longest (11 chars).
        assert!(out.contains("x          : u8"), "actual:\n{}", out);
        assert!(out.contains("longer_name: bool"), "actual:\n{}", out);
    }

    #[test]
    fn arrow_alignment_on_transition_group() {
        let src = r#"language fsm 2.0
machine M {
    state S {
        on A -> X
        on LongEvent -> Y
        on B -> Z
    }
}
"#;
        let out = fmt(src);
        // After alignment, both A and B lines should pad to the column
        // where LongEvent's `->` lives.
        let lines: Vec<&str> = out.lines().collect();
        let arrow_cols: Vec<usize> = lines
            .iter()
            .filter(|l| l.contains("->"))
            .map(|l| l.find("->").unwrap())
            .collect();
        assert!(arrow_cols.len() >= 3, "{out}");
        let all_same = arrow_cols.iter().all(|c| *c == arrow_cols[0]);
        assert!(all_same, "arrow columns not aligned: {arrow_cols:?}\n{out}");
    }

    #[test]
    fn shallow_history_single_token_form() {
        // Doc 04 §1.5 / §7.1: `shallow_history` is one keyword. We must
        // emit it as such (Doc 19's two-word form is rejected).
        let src = r#"language fsm 2.0
machine M {
    state S {
        shallow_history H { initial X }
    }
}
"#;
        let out = fmt(src);
        assert!(out.contains("shallow_history H {"), "{out}");
    }

    #[test]
    fn composite_uses_state_keyword_not_composite() {
        // Doc 04 has no `composite` keyword (per VALIDATION_REPORT 2.21).
        // The grammar uses `state NAME { nested state … }`.
        let src = r#"language fsm 2.0
machine M {
    state Outer {
        initial Inner
        state Inner { }
    }
}
"#;
        let out = fmt(src);
        assert!(out.contains("state Outer {"), "{out}");
        assert!(!out.contains("composite"), "{out}");
    }

    #[test]
    fn redundant_parens_stripped_top_level() {
        let src = "language fsm 2.0\nconst C = (1 + 2)\n";
        let out = fmt(src);
        // At top-level the surrounding parens around `1 + 2` add nothing
        // because there is no parent expression to interact with — strip.
        assert!(out.contains("const C = 1 + 2"), "{out}");
    }
}
