//! Compile-time validation of `opaque "C_type_string"` payloads.
//!
//! Per Doc 00 §7.12 (G-02) the contents of the string between the quotes are
//! emitted verbatim into the generated C header — `M_t.field` becomes a
//! field of that type. An unrestricted string is a code-injection sink:
//! `"int; system(\"rm -rf /\"); int"` would smuggle a function call into the
//! header.
//!
//! Grammar enforced here: `^[A-Za-z_][A-Za-z0-9_ *]*$`
//!
//! In English: a C identifier start character followed by zero or more
//! identifier characters, space (for `unsigned int` style names), or `*`
//! (for pointer types like `void *`). No semicolons, no parens, no quotes,
//! no comments.
//!
//! Length cap: 256 chars. Real C type names never exceed this; the limit
//! keeps a hostile generator from producing megabyte-sized type strings
//! that downstream tooling would mishandle.

use fsm_diagnostics::{Diagnostic, DiagnosticCode, Span};

/// Maximum length of an opaque C type string. Picked generously — `struct
/// some_extremely_long_name_with_modifiers *` weighs around 50 chars; 256
/// gives a 5x margin for unusual but legitimate type names while remaining
/// far smaller than any realistic attack surface.
pub const MAX_OPAQUE_TYPE_LEN: usize = 256;

/// Validate the inner string of an `opaque "…"` reference. `span` is the
/// span of the original string literal (with quotes).
pub fn validate_opaque_type(raw: &str, span: Span) -> Result<(), Diagnostic> {
    if raw.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::E0010, span)
            .with_message("invalid opaque type: empty string"));
    }
    if raw.len() > MAX_OPAQUE_TYPE_LEN {
        return Err(
            Diagnostic::new(DiagnosticCode::E0010, span).with_message(format!(
                "invalid opaque type: {} chars exceeds maximum {}",
                raw.len(),
                MAX_OPAQUE_TYPE_LEN
            )),
        );
    }
    let mut chars = raw.chars();
    let first = chars.next().expect("non-empty checked above");
    if !is_c_ident_start(first) {
        return Err(Diagnostic::new(DiagnosticCode::E0010, span)
            .with_message("invalid opaque type: must start with an ASCII letter or '_'"));
    }
    for ch in chars {
        if !is_c_ident_cont(ch) {
            return Err(
                Diagnostic::new(DiagnosticCode::E0010, span).with_message(format!(
                    "invalid opaque type: contains forbidden character {ch:?}"
                )),
            );
        }
    }
    Ok(())
}

fn is_c_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_c_ident_cont(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == ' ' || c == '*'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(s: &str) {
        assert!(
            validate_opaque_type(s, Span::new(0, s.len())).is_ok(),
            "expected {s:?} to be accepted"
        );
    }
    fn bad(s: &str) {
        let r = validate_opaque_type(s, Span::new(0, s.len()));
        assert!(r.is_err(), "expected {s:?} to be rejected");
        assert_eq!(r.unwrap_err().code, DiagnosticCode::E0010);
    }

    #[test]
    fn accepts_plain_typedef_names() {
        ok("my_struct_t");
        ok("MyStruct");
        ok("uint32_t");
        ok("_private_t");
    }

    #[test]
    fn accepts_pointer_and_qualifier_forms() {
        ok("void *");
        ok("uint8_t *");
        ok("unsigned int");
        ok("struct foo");
    }

    #[test]
    fn rejects_empty() {
        bad("");
    }

    #[test]
    fn rejects_injection_attempts() {
        bad(r#"int; system("rm -rf /"); int"#);
        bad("int(foo)");
        bad("int /* comment */ x");
        bad("int{}");
        bad(r#"int "fake""#);
    }

    #[test]
    fn rejects_leading_digit() {
        bad("32bits");
    }

    #[test]
    fn rejects_too_long() {
        let huge = "x".repeat(MAX_OPAQUE_TYPE_LEN + 1);
        bad(&huge);
    }
}
