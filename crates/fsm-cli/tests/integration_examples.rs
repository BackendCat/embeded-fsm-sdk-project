//! Behavioural acceptance for the worked build-ecosystem integrations
//! (ROADMAP v1.1 W6, SUBAGENT_CONVENTIONS §5.4).
//!
//! Each `examples/integration/<eco>/` is an "integration" only if it
//! genuinely **builds and runs** — a doc-only integration is incomplete
//! (§5.4). These tests therefore shell out to the real build tools:
//!
//!   - `make/`       : `make FSM=<bin> run` — exit 0 + verified banner.
//!   - `cmake/`      : `cmake` configure + build + run the binary.
//!   - `cargo-rust/` : `cargo build` + run (standalone crate), `FSM` env
//!                     fed to its build.rs; asserts the FFI lifecycle.
//!   - `platformio/` : `pio run` when `pio` is on PATH; otherwise the
//!                     DOCUMENTED fallback — compile the project's
//!                     generated C + driver with host
//!                     `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror`
//!                     and run it (the portable-C subset is the real
//!                     guarantee; PlatformIO is just the wrapper).
//!
//! Tool policy (mirrors the P1-3 gcc precedent — hard-require, never a
//! silent skip): `make`, `cmake`, `cargo` and `gcc` are verified present
//! on this box and used for real; their absence is a loud panic with a
//! fix hint. Only `pio` may be absent → loud, documented gcc-equivalent
//! fallback (printed, never a silent pass).
//!
//! FAIL-on-main: `examples/integration/` and this test do not exist on
//! `main`, so every assertion here trivially fails there; the value is
//! that they exercise the four build ecosystems end-to-end on this branch.

use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::cargo::cargo_bin;

/// Repo (worktree) root: `CARGO_MANIFEST_DIR` is `<root>/crates/fsm-cli`.
fn repo_root() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop(); // crates/fsm-cli -> crates
    p.pop(); // crates         -> root
    p
}

fn integration_dir(eco: &str) -> PathBuf {
    repo_root().join("examples/integration").join(eco)
}

/// Path to the freshly-built `fsm` binary under test (NOT whatever is on
/// PATH) — passed to each ecosystem via its `FSM` knob so the test
/// exercises this branch's codegen.
fn fsm_bin() -> PathBuf {
    cargo_bin("fsm")
}

/// Hard-require a tool the way `common::should_skip_gcc` hard-requires
/// gcc: a missing tool is a loud panic with a fix hint, never a silent
/// skip. `make`/`cmake`/`cargo`/`gcc` are all present on CI/this box.
fn require_tool(tool: &str, version_arg: &str) {
    let ok = Command::new(tool)
        .arg(version_arg)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    assert!(
        ok,
        "[integration_examples] required tool `{tool}` is not on PATH. \
         W6 integrations build for real; install `{tool}`. (Only `pio` \
         is allowed absent — it has a documented gcc fallback.)"
    );
}

fn pio_on_path() -> bool {
    Command::new("pio")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run a command in `dir`, return (success, stdout, stderr) with a clear
/// panic if the process could not even be spawned.
fn run(dir: &Path, program: &str, args: &[&str]) -> (bool, String, String) {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn `{program} {args:?}` in {dir:?}: {e}"));
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Run a command with an explicit `FSM` env var (the binary under test).
fn run_with_fsm(dir: &Path, program: &str, args: &[&str], fsm: &Path) -> (bool, String, String) {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .env("FSM", fsm)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn `{program} {args:?}` in {dir:?}: {e}"));
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

const VERIFIED_BANNER: &str = "OK Motor FSM: lifecycle verified";

// ───────────────────────── make ─────────────────────────

#[test]
fn make_example_builds_and_runs() {
    require_tool("make", "--version");
    let dir = integration_dir("make");
    assert!(dir.join("Makefile").is_file(), "make/Makefile missing");
    let fsm = fsm_bin();

    // Idempotent: clean any prior artifacts so the build is genuine.
    let _ = run_with_fsm(&dir, "make", &["clean"], &fsm);

    let (ok, stdout, stderr) = run_with_fsm(&dir, "make", &["run"], &fsm);
    assert!(
        ok,
        "`make run` failed.\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    assert!(
        stdout.contains(VERIFIED_BANNER),
        "`make run` did not print the verified banner.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

// ───────────────────────── cmake ────────────────────────

#[test]
fn cmake_example_builds_and_runs() {
    require_tool("cmake", "--version");
    require_tool("make", "--version"); // default CMake generator here
    let dir = integration_dir("cmake");
    assert!(
        dir.join("CMakeLists.txt").is_file(),
        "cmake/CMakeLists.txt missing"
    );
    let fsm = fsm_bin();

    // Fresh out-of-source build tree under the system temp dir so the
    // test never pollutes the worktree (and is hermetic across reruns).
    let build = tempfile::tempdir().expect("tempdir");
    let build_arg = build.path().to_str().unwrap();
    let dfsm = format!("-DFSM={}", fsm.display());

    let (ok, so, se) = run(&dir, "cmake", &["-S", ".", "-B", build_arg, &dfsm]);
    assert!(ok, "cmake configure failed.\nstdout: {so}\nstderr: {se}");

    let (ok, so, se) = run(&dir, "cmake", &["--build", build_arg]);
    assert!(ok, "cmake build failed.\nstdout: {so}\nstderr: {se}");

    let bin = build.path().join("motor_app");
    assert!(bin.is_file(), "cmake did not produce motor_app at {bin:?}");
    let (ok, so, se) = run(&dir, bin.to_str().unwrap(), &[]);
    assert!(
        ok && so.contains(VERIFIED_BANNER),
        "cmake-built motor_app did not run clean.\nstdout: {so}\nstderr: {se}"
    );
}

// ──────────────────────── cargo-rust ────────────────────

#[test]
fn cargo_rust_example_builds_and_runs() {
    require_tool("cargo", "--version");
    require_tool("gcc", "--version"); // build.rs uses the cc crate -> gcc
    let dir = integration_dir("cargo-rust");
    assert!(
        dir.join("Cargo.toml").is_file(),
        "cargo-rust/Cargo.toml missing"
    );
    let fsm = fsm_bin();

    // `cargo run` here both builds (build.rs runs `fsm generate` + cc)
    // and executes the FFI driver; a non-zero exit is a failed assertion
    // inside src/main.rs (a real behavioural failure, not a build error).
    let (ok, stdout, stderr) = run_with_fsm(&dir, "cargo", &["run", "--quiet"], &fsm);
    assert!(
        ok,
        "`cargo run` (cargo-rust) failed.\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
    );
    assert!(
        stdout.contains("OK Rust<->C FSM: lifecycle verified over FFI"),
        "cargo-rust did not print the FFI verified banner.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

/// The cargo-rust crate MUST be its own workspace root (empty
/// `[workspace]` table) so it builds standalone when copied out of this
/// repo and is never absorbed by the parent FSM workspace.
#[test]
fn cargo_rust_example_is_standalone_workspace() {
    let toml = std::fs::read_to_string(integration_dir("cargo-rust").join("Cargo.toml"))
        .expect("read cargo-rust Cargo.toml");
    assert!(
        toml.contains("[workspace]"),
        "cargo-rust/Cargo.toml must declare an empty [workspace] table so \
         it is not absorbed by the parent workspace; got:\n{toml}"
    );
}

// ──────────────────────── platformio ────────────────────

#[test]
fn platformio_example_builds_pio_or_gcc_fallback() {
    let dir = integration_dir("platformio");
    assert!(
        dir.join("platformio.ini").is_file(),
        "platformio/platformio.ini missing"
    );
    let fsm = fsm_bin();

    if pio_on_path() {
        // PlatformIO present: build every env for real.
        eprintln!("[integration_examples] `pio` found — running `pio run`");
        let (ok, so, se) = run_with_fsm(&dir, "pio", &["run"], &fsm);
        assert!(
            ok,
            "`pio run` failed.\n--- stdout ---\n{so}\n--- stderr ---\n{se}"
        );
        return;
    }

    // DOCUMENTED fallback (loud, never silent): PlatformIO is just the
    // build/flash wrapper; the guarantee that matters is that the SAME
    // generated C + driver compiles + runs under a strict C99 toolchain.
    // gcc IS required on this box (P1-3 precedent) — its absence is a
    // hard panic, not a skip.
    eprintln!(
        "[integration_examples] `pio` NOT on PATH — falling back to the \
         documented host-gcc equivalent: compiling platformio/'s generated \
         C + hal.c + motor_externs.c + main.c with \
         `gcc -std=c99 -Wall -Wextra -Wpedantic -Werror` and running it. \
         (Install `pip install platformio` to exercise the real `pio run`.)"
    );
    require_tool("gcc", "--version");

    let work = tempfile::tempdir().expect("tempdir");
    let gen = work.path().join("gen");
    std::fs::create_dir_all(&gen).unwrap();

    // 1. fsm generate (the same step platformio.ini's pre-build hook runs).
    let src = dir.join("src");
    let (ok, so, se) = {
        let out = Command::new(&fsm)
            .args(["generate", "--target", "c99"])
            .arg(src.join("motor.fsm"))
            .arg("--out")
            .arg(&gen)
            .output()
            .expect("spawn fsm generate");
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    };
    assert!(ok, "fsm generate (platformio fallback) failed.\n{so}\n{se}");

    // 2. Strict host gcc compile + link of the project's portable C +
    //    the bare-main driver (src/main.c — the `native` env's TU).
    let bin = work.path().join("pio_fallback_app");
    let gen_s = gen.to_str().unwrap();
    let src_s = src.to_str().unwrap();
    let inc_gen = format!("-I{gen_s}");
    let inc_src = format!("-I{src_s}");
    let motor_c = gen.join("Motor.c");
    let hal_c = src.join("hal.c");
    let ext_c = src.join("motor_externs.c");
    let main_c = src.join("main.c");
    let out = Command::new("gcc")
        .args([
            "-std=c99",
            "-Wall",
            "-Wextra",
            "-Wpedantic",
            "-Werror",
            &inc_gen,
            &inc_src,
        ])
        .arg(&motor_c)
        .arg(&hal_c)
        .arg(&ext_c)
        .arg(&main_c)
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("spawn gcc");
    assert!(
        out.status.success(),
        "platformio gcc-fallback compile failed.\n--- gcc stderr ---\n{}\n--- gcc stdout ---\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout),
    );

    // 3. Run it — exit 0 == the embedded-portable C is behaviourally sound.
    let out = Command::new(&bin).output().expect("run pio fallback bin");
    let so = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && so.contains(VERIFIED_BANNER),
        "platformio gcc-fallback binary did not run clean (code {:?}).\nstdout: {}\nstderr: {}",
        out.status.code(),
        so,
        String::from_utf8_lossy(&out.stderr),
    );
}

/// Cheap structural guard (secondary to the behavioural tests above):
/// every example dir + its entry file is present, so a future refactor
/// that deletes one fails loudly here too.
#[test]
fn all_integration_examples_present() {
    for (eco, marker) in [
        ("make", "Makefile"),
        ("cmake", "CMakeLists.txt"),
        ("cargo-rust", "Cargo.toml"),
        ("platformio", "platformio.ini"),
    ] {
        let d = integration_dir(eco);
        assert!(d.is_dir(), "missing examples/integration/{eco}/");
        assert!(
            d.join(marker).is_file(),
            "missing examples/integration/{eco}/{marker}"
        );
        assert!(
            d.join("README.md").is_file(),
            "missing examples/integration/{eco}/README.md"
        );
        assert!(
            d.join("motor.fsm").is_file() || d.join("src/motor.fsm").is_file(),
            "missing motor.fsm in examples/integration/{eco}/"
        );
    }
}
