//! Parse-Format-Parse round-trip.
//!
//! For every fixture: parse → format → parse the formatted output. Assert
//! the second parse produces zero diagnostics, i.e. the formatter never
//! introduces a syntax error.
//!
//! This is the "semantic-neutral" half of Doc 19 §1.4 — combined with the
//! idempotency property, it guarantees the formatter is a closed operator
//! on the language.

use fsm_formatter::{format_string, FormatOptions};
use fsm_parser::parse;
use std::fs;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn formatted_output_re_parses_with_no_diagnostics() {
    let opts = FormatOptions::default();
    let mut count = 0;
    for entry in fs::read_dir(fixtures_dir()).unwrap() {
        let p = entry.unwrap().path();
        if p.extension().and_then(|s| s.to_str()) != Some("fsm") {
            continue;
        }
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let src = fs::read_to_string(&p).unwrap();
        // First, ensure the source itself parses cleanly — otherwise the
        // formatter wouldn't run.
        let orig_parse = parse(&src);
        assert!(
            orig_parse.errors.is_empty(),
            "fixture {name} fails first-pass parse: {:?}",
            orig_parse.errors
        );

        let once = format_string(&src, &opts).expect("first format");
        let reparse = parse(&once);
        assert!(
            reparse.errors.is_empty(),
            "fixture {name} introduced parse errors after formatting:\n{once}\n  diags: {:?}",
            reparse.errors
        );
        count += 1;
    }
    assert!(count >= 15, "expected at least 15 fixtures, saw {count}");
}
