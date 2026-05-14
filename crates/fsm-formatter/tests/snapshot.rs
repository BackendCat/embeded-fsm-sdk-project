//! Golden snapshots for representative fixtures.
//!
//! Five sources spanning the major Doc 04 grammar families. The expected
//! output lives in `tests/snapshots/`; running `cargo insta review` after
//! a deliberate behaviour change re-baselines them.

use fsm_formatter::{format_string, FormatOptions};
use std::fs;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn fmt(name: &str) -> String {
    let path = fixtures_dir().join(name);
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    format_string(&src, &FormatOptions::default()).unwrap_or_else(|e| panic!("format {name}: {e}"))
}

#[test]
fn snapshot_minimal_machine() {
    insta::assert_snapshot!("02_minimal_machine", fmt("02_minimal_machine.fsm"));
}

#[test]
fn snapshot_context_block_alignment() {
    insta::assert_snapshot!("03_context_block", fmt("03_context_block.fsm"));
}

#[test]
fn snapshot_composite_state() {
    insta::assert_snapshot!("09_composite_state", fmt("09_composite_state.fsm"));
}

#[test]
fn snapshot_parallel_regions() {
    insta::assert_snapshot!("10_parallel_regions", fmt("10_parallel_regions.fsm"));
}

#[test]
fn snapshot_full_example() {
    insta::assert_snapshot!("15_full_example", fmt("15_full_example.fsm"));
}
