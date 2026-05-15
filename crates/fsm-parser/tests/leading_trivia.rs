//! Regression suite for PARSE-BUG-1.
//!
//! Any whitespace/comment trivia (line comment, block comment, doc comment,
//! blank line, or a mix) *before* the `language fsm X.Y` header used to be
//! emitted into the rowan `GreenNodeBuilder` by the parser constructor
//! **before** the root `FILE` node was opened. rowan asserts a single root
//! child; the stray leading-trivia tokens made it N+1, tripping
//! `rowan-0.15.18/src/green/builder.rs:113` (`assertion left == right`,
//! observed `left: 3 right: 1` for `// comment\n`). License/banner headers
//! atop a `.fsm` are ubiquitous real-world input, so the panic violated G1
//! (no undefined behaviour).
//!
//! These tests FAIL on `main` (panic/abort while building the green tree)
//! and PASS after the fix. Each asserts the four robustness invariants:
//!   1. `parse()` does not panic;
//!   2. no error diagnostics (leading trivia is benign — including a bare
//!      `///` doc comment with nothing to attach, preserved as file-leading
//!      trivia per the existing trivia convention, NOT a new diagnostic);
//!   3. the typed AST still exposes the `language` decl + the `machine`
//!      (the header is recognised *after* the trivia);
//!   4. byte-exact CST round-trip — `reconstructed_text() == src` — so the
//!      leading comment is preserved at its original file-leading position
//!      (rowan's lossless invariant).

use fsm_parser::parse;

/// The minimal valid machine appended after every leading-trivia prefix.
/// Kept on its own so each case is `prefix + BODY` and the round-trip
/// assertion compares against the exact concatenation.
const BODY: &str = "language fsm 2.0\nmachine M { initial S\n state S {} }\n";

/// (case-name, leading trivia prefixed before `BODY`).
///
/// Covers every `TokenKind::is_trivia()` variant in leading position plus
/// blank lines and a mixed banner, which is what real license headers look
/// like.
fn leading_trivia_cases() -> Vec<(&'static str, &'static str)> {
    vec![
        ("line_comment", "// SPDX-License-Identifier: MIT\n"),
        ("block_comment", "/* Copyright 2026 Acme. */\n"),
        (
            "doc_comment",
            "/// crate-level doc with nothing to attach\n",
        ),
        ("single_blank_line", "\n"),
        ("multiple_blank_lines", "\n\n\n"),
        ("leading_whitespace", "   \t  \n"),
        (
            "mixed_banner",
            "// SPDX-License-Identifier: MIT\n\
             /*\n * Acme FSM\n * (c) 2026\n */\n\
             \n\
             /// module: motor controller\n",
        ),
    ]
}

#[test]
fn leading_trivia_before_language_header_parses_without_panic() {
    for (name, prefix) in leading_trivia_cases() {
        let src = format!("{prefix}{BODY}");

        // (1) No panic. `parse` returns; on `main` this line aborts the
        //     process inside rowan's builder before returning.
        let pr = parse(&src);

        // (2) Leading trivia is benign — zero error diagnostics for every
        //     variant, including the bare `///`.
        assert!(
            pr.errors.is_empty(),
            "[{name}] expected clean parse, got diagnostics: {:#?}\n--- src ---\n{src}",
            pr.errors
        );

        // (3) The header + machine are still recognised after the trivia.
        let ast = pr.ast();
        assert!(
            ast.language_decl().is_some(),
            "[{name}] language decl not found after leading trivia\n--- src ---\n{src}",
        );
        let machines: Vec<_> = ast.machines().collect();
        assert_eq!(
            machines.len(),
            1,
            "[{name}] expected exactly one machine, got {}\n--- src ---\n{src}",
            machines.len(),
        );
        assert_eq!(
            machines[0].name(),
            Some("M".to_string()),
            "[{name}] machine name lost\n--- src ---\n{src}",
        );

        // (4) Byte-exact CST round-trip — the leading comment is preserved
        //     verbatim at its original file-leading position.
        assert_eq!(
            pr.reconstructed_text(),
            src,
            "[{name}] CST is not byte-exact lossless",
        );
    }
}

/// Tightly pin the canonical PARSE-BUG-1 reproduction (the exact input from
/// the bug report) so a future regression is unambiguous, and assert the
/// comment text is actually present in the reconstructed CST (not merely
/// that lengths match).
#[test]
fn spdx_license_header_is_preserved_in_cst() {
    let src =
        "// SPDX-License-Identifier: MIT\nlanguage fsm 2.0\nmachine M { initial S\n state S {} }";
    let pr = parse(src);
    assert!(
        pr.errors.is_empty(),
        "expected clean parse, got: {:#?}",
        pr.errors
    );
    let reconstructed = pr.reconstructed_text();
    assert_eq!(reconstructed, src, "CST not byte-exact");
    assert!(
        reconstructed.starts_with("// SPDX-License-Identifier: MIT\n"),
        "leading license comment not preserved at file-leading position: {reconstructed:?}",
    );
    assert!(
        pr.ast().language_decl().is_some(),
        "language decl not recognised after the license banner",
    );
}

/// A file that is *only* leading trivia (no real tokens at all) must also
/// not panic — the constructor skips to EOF, the FILE node opens, the
/// trivia flushes inside it, and the missing-header diagnostic (E0010) is
/// the single expected error. This guards the `leading_trivia_end == pos ==
/// EOF` edge of the fix.
#[test]
fn comment_only_file_does_not_panic() {
    let src = "// just a banner, no FSM here\n\n";
    let pr = parse(src); // must not panic
    assert_eq!(
        pr.reconstructed_text(),
        src,
        "comment-only file must still round-trip byte-exact",
    );
    // No header → exactly the documented E0010, never a panic.
    assert!(
        pr.errors
            .iter()
            .all(|d| d.code == fsm_diagnostics::DiagnosticCode::E0010),
        "expected only the missing-header E0010, got: {:#?}",
        pr.errors
    );
}
