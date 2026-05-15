//! Shared helpers for `fsm-cli` integration tests.
//!
//! ## gcc gate (P1-3)
//!
//! Several integration tests drive the generated C through `gcc -Werror`.
//! Prior to P1-3 each test silently `return`-ed when `gcc` was missing from
//! PATH, which meant CI runners without `gcc` reported green while skipping
//! the only end-to-end gate. Now the helper panics by default with a clear
//! message and only skips when `FSM_SKIP_GCC_TESTS` is explicitly set in the
//! environment. CI must hard-require gcc; opting out is a developer-only
//! escape hatch.

// `tests/common/mod.rs` is the shared integration-test harness, recompiled per
// test binary; the workspace `unreachable_pub` lint (Doc 00 §11.4x) sees these
// helpers as unreachable per-binary even though they are a real cross-test API.
// Same idiom-based justification as the existing per-item `#[allow(dead_code)]`
// on `should_skip_gcc`/`workspace_root`; a single module attribute is the
// lower-noise expression of it.
#![allow(unreachable_pub)]

use std::path::PathBuf;

/// Returns `true` when the caller should short-circuit because gcc skipping
/// was opted into via `FSM_SKIP_GCC_TESTS`. Panics if gcc is not on PATH and
/// the env var is unset — silent skips would hide a missing toolchain on CI.
///
/// Usage:
///
/// ```ignore
/// if crate::common::should_skip_gcc("vending_machine_gcc") {
///     return;
/// }
/// ```
#[allow(dead_code)]
pub fn should_skip_gcc(label: &str) -> bool {
    if std::env::var("FSM_SKIP_GCC_TESTS").is_ok() {
        eprintln!("[{}] FSM_SKIP_GCC_TESTS set — skipping gcc gate", label);
        return true;
    }
    if !gcc_on_path() {
        panic!(
            "[{}] gcc is not on PATH. Install gcc, or set FSM_SKIP_GCC_TESTS=1 \
             to opt out of the gcc gate explicitly. Silent skipping is no \
             longer allowed (see audit P1-3).",
            label
        );
    }
    false
}

fn gcc_on_path() -> bool {
    std::process::Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Absolute path to the workspace root (the repo root that contains the
/// top-level `Cargo.toml`). `CARGO_MANIFEST_DIR` for this crate is
/// `<workspace>/crates/fsm-cli`, so we strip two segments.
#[allow(dead_code)]
pub fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/fsm-cli → crates/
    p.pop(); // crates/         → repo root
    p
}
