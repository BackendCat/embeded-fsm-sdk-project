//! Small lexical helpers shared across the header-import pipeline.
//! Every function here is pure and dependency-free so the rest of the
//! submodules can rely on them without import cycles.

/// True iff `s` is a valid C identifier (`[A-Za-z_][A-Za-z0-9_]*`, ASCII).
pub(super) fn is_c_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// True iff `s` is a C type keyword (used to distinguish a trailing
/// parameter *name* from a type token in `int` or `unsigned char`).
pub(super) fn is_type_keyword(s: &str) -> bool {
    matches!(
        s,
        "void"
            | "bool"
            | "_Bool"
            | "char"
            | "short"
            | "int"
            | "long"
            | "float"
            | "double"
            | "signed"
            | "unsigned"
            | "const"
            | "volatile"
            | "int8_t"
            | "int16_t"
            | "int32_t"
            | "int64_t"
            | "uint8_t"
            | "uint16_t"
            | "uint32_t"
            | "uint64_t"
            | "size_t"
            | "ssize_t"
            | "ptrdiff_t"
            | "uintptr_t"
            | "intptr_t"
    )
}

/// Reserved words that are NOT types — if one of these turns up where a
/// type belongs the fragment is not a function declaration we can model.
pub(super) fn is_reserved_nontype(s: &str) -> bool {
    matches!(
        s,
        "return"
            | "if"
            | "else"
            | "for"
            | "while"
            | "do"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "goto"
            | "sizeof"
            | "typedef"
            | "extern"
            | "static"
            | "inline"
            | "register"
            | "auto"
            | "default"
    )
}

/// Collapse internal whitespace runs to single spaces; trim ends.
pub(super) fn condense_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `const  uint8_t   *` → `const uint8_t *`; `char**` → `char **`.
pub(super) fn normalise_pointer(s: &str) -> String {
    let mut spaced = String::new();
    for ch in s.chars() {
        if ch == '*' {
            if !spaced.ends_with(' ') && !spaced.is_empty() {
                spaced.push(' ');
            }
            spaced.push('*');
        } else {
            spaced.push(ch);
        }
    }
    condense_ws(&spaced)
}

/// Index of the `)` matching the `(` at `open` (byte indices). Quotes were
/// already blanked upstream so no string-awareness is needed here.
pub(super) fn matching_paren(s: &str, open: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    let mut depth = 0i32;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Split a parameter list body on top-level commas (depth-aware so a
/// pointer-to-array or nested paren — which we'll skip anyway — doesn't
/// shatter).
pub(super) fn split_params(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    for ch in s.chars() {
        match ch {
            '(' | '[' => {
                depth += 1;
                cur.push(ch);
            }
            ')' | ']' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// Strip leading storage/specifier keywords and GCC/MSVC attributes from a
/// prototype so the type/name splitter sees a clean `RET NAME(...)`.
pub(super) fn strip_attributes(proto: &str) -> String {
    let mut s = condense_ws(proto);
    // Remove `__attribute__((...))`, `__declspec(...)`, `__asm__(...)`
    // wherever they appear (balanced-paren aware).
    for kw in ["__attribute__", "__declspec", "__asm__", "__asm"] {
        loop {
            let Some(at) = s.find(kw) else { break };
            let after = at + kw.len();
            let rest = s[after..].trim_start();
            if !rest.starts_with('(') {
                break;
            }
            let popen = s[after..].find('(').unwrap() + after;
            let Some(pclose) = matching_paren(&s, popen) else {
                break;
            };
            s.replace_range(at..pclose + 1, "");
            s = condense_ws(&s);
        }
    }
    // Leading storage-class / inline specifiers.
    loop {
        let head = s.split_whitespace().next().unwrap_or("");
        if matches!(
            head,
            "extern" | "static" | "inline" | "__inline" | "__inline__" | "_Noreturn"
        ) {
            s = s[head.len()..].trim_start().to_string();
        } else {
            break;
        }
    }
    s
}
