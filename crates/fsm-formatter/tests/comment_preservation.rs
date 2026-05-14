//! Comment preservation round-trip.
//!
//! Lex-tokens the source, lex-tokens the formatter output, and asserts
//! every comment (line / block / doc) in the input is present, in source
//! order, in the output. The formatter is allowed to rewrap or re-indent
//! comments; it MUST NOT delete any.

use fsm_formatter::{format_string, FormatOptions};
use fsm_lexer::{tokenize, TokenKind};

fn extract_comments(src: &str) -> Vec<String> {
    tokenize(src)
        .into_iter()
        .filter_map(|t| match t.kind {
            TokenKind::LineComment | TokenKind::BlockComment | TokenKind::DocComment => {
                Some(src[t.span.start..t.span.end].trim_end().to_string())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn line_comments_survive() {
    let src = r#"language fsm 2.0
// top-of-file comment
machine M {
    // inside machine
    initial S
    state S {
        // before transition
        on TICK -> S
    }
}
"#;
    let opts = FormatOptions::default();
    let formatted = format_string(src, &opts).expect("parses");
    let want = extract_comments(src);
    let got = extract_comments(&formatted);
    assert_eq!(
        want, got,
        "comments lost or reordered.\n--- input ---\n{src}\n--- output ---\n{formatted}"
    );
}

#[test]
fn doc_comments_survive() {
    let src = r#"language fsm 2.0

/// Top-level guard.
pure extern is_ready(ctx) : bool

/// Main machine.
machine M {
    initial S

    state S { }
}
"#;
    let opts = FormatOptions::default();
    let formatted = format_string(src, &opts).expect("parses");
    let want = extract_comments(src);
    let got = extract_comments(&formatted);
    assert_eq!(want, got);
}

#[test]
fn block_comments_survive() {
    let src = r#"language fsm 2.0

/* multi
   line
   block */
machine M {
    initial S

    state S {
        /* inline */
        on TICK -> S
    }
}
"#;
    let opts = FormatOptions::default();
    let formatted = format_string(src, &opts).expect("parses");
    let want = extract_comments(src);
    let got = extract_comments(&formatted);
    assert_eq!(want, got);
}

#[test]
fn mixed_comment_styles_all_survive() {
    let src = r#"language fsm 2.0

/// doc 1
machine A {
    // single-line
    initial S
    state S {
        /* block */
        on TICK -> S
    }
}
"#;
    let opts = FormatOptions::default();
    let formatted = format_string(src, &opts).expect("parses");
    let want = extract_comments(src);
    let got = extract_comments(&formatted);
    assert_eq!(want.len(), got.len(), "{formatted}");
    for (w, g) in want.iter().zip(got.iter()) {
        // Allow trailing-whitespace normalisation but assert content
        // equality.
        assert_eq!(w.trim(), g.trim());
    }
}
