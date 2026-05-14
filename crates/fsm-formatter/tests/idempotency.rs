//! Idempotency property test.
//!
//! For every `.fsm` fixture under `tests/fixtures/`:
//!   1. Format the source → `once`.
//!   2. Format `once`     → `twice`.
//!   3. Assert `once == twice` (the cardinal Doc 19 §1.2 property).
//!
//! Additional sanity:
//!   - The single-pass output must also re-parse cleanly (no diagnostics
//!     introduced by formatting). Implemented in `round_trip.rs`.

use fsm_formatter::{format_string, FormatOptions};
use std::fs;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn each_fixture() -> impl Iterator<Item = (String, String)> {
    let dir = fixtures_dir();
    fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}"))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("fsm") {
                return None;
            }
            let name = path.file_name()?.to_string_lossy().to_string();
            let src = fs::read_to_string(&path).ok()?;
            Some((name, src))
        })
}

#[test]
fn every_fixture_is_idempotent() {
    let opts = FormatOptions::default();
    let mut count = 0;
    for (name, src) in each_fixture() {
        let once =
            format_string(&src, &opts).unwrap_or_else(|e| panic!("[{name}] first format: {e}"));
        let twice =
            format_string(&once, &opts).unwrap_or_else(|e| panic!("[{name}] second format: {e}"));
        assert_eq!(once, twice, "fixture {name} is not idempotent");
        count += 1;
    }
    assert!(count >= 15, "expected at least 15 fixtures, saw {count}");
}
