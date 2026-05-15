//! Full-chain end-to-end for `examples/deferred/` — the v1.1 deferred-
//! event feature exercised through the *shipped CLI*, not internal APIs.
//!
//! Chain (SUBAGENT_CONVENTIONS §5.4 — behaviour, not symbol presence):
//!   1. `fsm check  examples/deferred/deferred.fsm`   → exit 0
//!   2. `fsm generate --target c99 ... --out <tmp>`   → exit 0
//!   3. `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror -c <tmp>/*.c` → 0
//!   4. `fsm test examples/deferred/`                 → the .trace passes
//!      against the simulator (defer/replay records match the
//!      hand-verified `expected` block — exit 0)
//!
//! Step 4 is the byte-identity gate: the `.trace` `expected` block was
//! captured from the simulator and the codegen behavioural-acceptance test
//! (`fsm-codegen-c/tests/defer_codegen_runs.rs`) asserts the generated C
//! reproduces the same hold/replay/exactly-once behaviour. Together they
//! prove simulator ≡ codegen on `defer`.
//!
//! Regression contract (§5.1): on `main` `fsm check` rejects the Printer
//! with FSM-E0903 (`defer` unsupported), so steps 1-4 cannot pass. They
//! pass only after the v1.1 defer-runtime wave.

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::Command as Assert;

fn gcc_available() -> bool {
    Command::new("gcc")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn workspace_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p
}

#[test]
fn deferred_example_check_generate_gcc_and_trace_all_pass() {
    let root = workspace_root();
    let fsm_src = root.join("examples/deferred/deferred.fsm");
    let example_dir = root.join("examples/deferred");
    assert!(
        fsm_src.is_file(),
        "fixture missing at {}",
        fsm_src.display()
    );

    // 1. `fsm check` — analyzer must accept `defer` (FSM-E0903 retired).
    Assert::cargo_bin("fsm")
        .unwrap()
        .arg("check")
        .arg(&fsm_src)
        .assert()
        .success();

    // 2. `fsm generate` — emit C99 into a temp dir.
    let tmp = tempfile::tempdir().expect("tempdir");
    let out_dir = tmp.path();
    Assert::cargo_bin("fsm")
        .unwrap()
        .args(["generate", "--target", "c99", "--out"])
        .arg(out_dir)
        .arg(&fsm_src)
        .assert()
        .success();

    for f in &[
        "fsm_hal.h",
        "Printer.h",
        "Printer.c",
        "Printer_conf.h",
        "Printer_impl.h",
    ] {
        assert!(
            out_dir.join(f).is_file(),
            "missing generated file {f} in {}",
            out_dir.display()
        );
    }

    // 3. gcc -Werror compile of the generated translation unit. The defer
    //    apparatus (defer table, _deferred[] buffer, release/drain) must
    //    survive -Wall -Wextra -Wpedantic -Werror under C99.
    if !gcc_available() {
        eprintln!("[deferred_example_e2e] gcc not on PATH — skipping gcc + run steps");
        return;
    }

    let conf = std::fs::read_to_string(out_dir.join("Printer_conf.h")).expect("read conf");
    assert!(
        conf.contains("PRINTER_DEFER_CAPACITY"),
        "conf header must define PRINTER_DEFER_CAPACITY; got:\n{conf}"
    );

    let gcc = Command::new("gcc")
        .current_dir(out_dir)
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            "-I.",
            "-c",
            "Printer.c",
        ])
        .output()
        .expect("invoke gcc");

    if !gcc.status.success() || !gcc.stderr.is_empty() {
        let src = std::fs::read_to_string(out_dir.join("Printer.c")).unwrap_or_default();
        eprintln!("=== generated Printer.c ===\n{src}");
        eprintln!(
            "=== gcc stderr ===\n{}",
            String::from_utf8_lossy(&gcc.stderr)
        );
        panic!(
            "gcc -Werror failed on generated Printer.c: {:?}",
            gcc.status.code()
        );
    }

    // 4. `fsm test examples/deferred/` — the .trace must pass against the
    //    simulator (defer + replay records match `expected`). This is the
    //    sim==codegen byte-identity gate.
    Assert::cargo_bin("fsm")
        .unwrap()
        .arg("test")
        .arg(&example_dir)
        .assert()
        .success();
}
