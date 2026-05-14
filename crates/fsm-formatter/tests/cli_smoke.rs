//! Error-propagation smoke test.
//!
//! `format_string` MUST return [`FormatError::ParseFailed`] on malformed
//! input. The wrapped diagnostics MUST be non-empty.
//!
//! This is the contract the CLI's `fsm fmt` subcommand depends on: bad
//! input has to surface as a non-zero exit, not silently produce garbage.

use fsm_formatter::{format_string, FormatError, FormatOptions};

#[test]
fn malformed_input_returns_parse_failed() {
    let bad = "machine { this is not fsm";
    let opts = FormatOptions::default();
    let err = format_string(bad, &opts).expect_err("malformed input must error");
    match err {
        FormatError::ParseFailed(diags) => {
            assert!(!diags.is_empty(), "must surface at least one diagnostic");
        }
    }
}

#[test]
fn missing_language_header_is_a_diagnostic() {
    let no_header = "machine M { }\n";
    let opts = FormatOptions::default();
    let err = format_string(no_header, &opts).expect_err("must error — header is required");
    let FormatError::ParseFailed(diags) = err;
    // Diagnostic E0010 from Doc 10 — "expected top-level/header".
    let codes: Vec<String> = diags.iter().map(|d| format!("{:?}", d.code)).collect();
    assert!(
        codes.iter().any(|c| c.contains("E0010")),
        "expected E0010 in diagnostics, got: {:?}",
        codes
    );
}

#[test]
fn well_formed_minimal_input_succeeds() {
    let good = "language fsm 2.0\n";
    let opts = FormatOptions::default();
    let out = format_string(good, &opts).expect("clean source must format");
    assert_eq!(out, "language fsm 2.0\n");
}
