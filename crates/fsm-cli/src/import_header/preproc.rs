//! Stage 1+2 of the header-import pipeline:
//!
//! 1. **Comment + literal blanking** — `//` and `/* */` comments are
//!    removed, string/char literals are blanked (so a `;` or `)` inside
//!    them never terminates a phantom declaration), and preprocessor
//!    directives are dropped. Function-like macros are surfaced as
//!    `SkipNote`s because they are the most common "why didn't my function
//!    import" surprise.
//! 2. **Top-level declaration splitting** — the cleaned source is broken
//!    into one chunk per top-level `;` or `}` so each downstream classifier
//!    sees exactly one declaration.

use super::util::{condense_ws, is_c_ident};
use super::SkipNote;

/// Remove `//` + `/* */` comments and string/char literals (replaced by a
/// space so token boundaries survive), and drop preprocessor directives
/// (any line whose first non-space char is `#`, honouring `\`-continuation).
/// Preprocessor lines are noted as skipped only when they look like a macro
/// that *might* have been a function the user expected (`#define X(...)`),
/// keeping the note list signal-rich rather than spamming every `#include`.
pub(super) fn strip_comments_and_preproc(src: &str, skipped: &mut Vec<SkipNote>) -> String {
    // Pass A: comment + literal blanking (character state machine).
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    #[derive(PartialEq)]
    enum S {
        Code,
        Line,
        Block,
        Str,
        Chr,
    }
    let mut st = S::Code;
    while i < bytes.len() {
        let c = bytes[i];
        let n = bytes.get(i + 1).copied().unwrap_or(0);
        match st {
            S::Code => {
                if c == b'/' && n == b'/' {
                    st = S::Line;
                    i += 2;
                } else if c == b'/' && n == b'*' {
                    st = S::Block;
                    out.push(' ');
                    i += 2;
                } else if c == b'"' {
                    st = S::Str;
                    out.push(' ');
                    i += 1;
                } else if c == b'\'' {
                    st = S::Chr;
                    out.push(' ');
                    i += 1;
                } else {
                    out.push(c as char);
                    i += 1;
                }
            }
            S::Line => {
                if c == b'\n' {
                    st = S::Code;
                    out.push('\n');
                }
                i += 1;
            }
            S::Block => {
                if c == b'*' && n == b'/' {
                    st = S::Code;
                    out.push(' ');
                    i += 2;
                } else {
                    if c == b'\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
            S::Str => {
                if c == b'\\' {
                    i += 2;
                } else if c == b'"' {
                    st = S::Code;
                    i += 1;
                } else {
                    i += 1;
                }
            }
            S::Chr => {
                if c == b'\\' {
                    i += 2;
                } else if c == b'\'' {
                    st = S::Code;
                    i += 1;
                } else {
                    i += 1;
                }
            }
        }
    }

    // Pass B: drop preprocessor lines (with backslash continuation).
    let mut result = String::with_capacity(out.len());
    let mut cont_pp = false;
    for line in out.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let is_pp_start = body.trim_start().starts_with('#');
        if cont_pp || is_pp_start {
            // Note function-like macros — a `#define FOO(x) ...` is the most
            // common "why didn't my function import" surprise.
            let t = body.trim_start();
            if is_pp_start {
                if let Some(rest) = t.strip_prefix('#') {
                    let rest = rest.trim_start();
                    if let Some(def) = rest.strip_prefix("define") {
                        let def = def.trim_start();
                        if def.contains('(')
                            && def
                                .split('(')
                                .next()
                                .map(|n| is_c_ident(n.trim()))
                                .unwrap_or(false)
                        {
                            skipped.push(SkipNote {
                                fragment: condense_ws(body.trim()),
                                reason: "function-like macro (preprocessor) — \
                                         not a linkable symbol; declare it as \
                                         an `extern` in the .fsm if you need it"
                                    .into(),
                            });
                        }
                    }
                }
            }
            cont_pp = body.trim_end().ends_with('\\');
            result.push('\n'); // keep line numbering roughly aligned
        } else {
            result.push_str(body);
            result.push('\n');
        }
    }
    result
}

/// Yield each top-level declaration (terminated by `;` or a closing `}` at
/// brace-depth 0). Parens and brackets are depth-tracked too so a `;`
/// inside a (hypothetical) initialiser or a `,` does not split mid-decl.
pub(super) fn split_top_level_decls(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut brace = 0i32;
    let mut paren = 0i32;
    for ch in s.chars() {
        match ch {
            '{' => {
                brace += 1;
                cur.push(ch);
            }
            '}' => {
                brace -= 1;
                cur.push(ch);
                if brace <= 0 && paren == 0 {
                    out.push(std::mem::take(&mut cur));
                    brace = 0;
                }
            }
            '(' => {
                paren += 1;
                cur.push(ch);
            }
            ')' => {
                paren -= 1;
                cur.push(ch);
            }
            ';' if brace == 0 && paren == 0 => {
                out.push(std::mem::take(&mut cur));
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}
