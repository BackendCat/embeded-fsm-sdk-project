//! Build script: run `fsm generate`, then compile the generated C (plus
//! the hand-written HAL + extern impls) into a static library this crate
//! links. This is the canonical "drive a C state machine from Rust"
//! integration pattern: codegen at build time, `cc` crate to compile,
//! `extern "C"` FFI at the call site (see `src/main.rs`).

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let gen_dir = out_dir.join("gen");

    // The `fsm` binary: explicit `FSM` env var wins, else `fsm` on PATH.
    // A user with `fsm` installed needs no configuration; CI / a dev
    // working tree points at the built binary via `FSM=...`.
    let fsm = env::var("FSM").unwrap_or_else(|_| "fsm".to_string());

    let fsm_src = manifest_dir.join("motor.fsm");

    println!("cargo:rerun-if-changed=motor.fsm");
    println!("cargo:rerun-if-changed=hal.c");
    println!("cargo:rerun-if-changed=motor_externs.c");
    println!("cargo:rerun-if-changed=motor_externs.h");
    println!("cargo:rerun-if-changed=ffi_helpers.c");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=FSM");

    std::fs::create_dir_all(&gen_dir).expect("create gen dir");

    // 1. Codegen: motor.fsm -> Motor.{c,h,_impl.h,_conf.h} + fsm_hal.h
    let status = Command::new(&fsm)
        .args(["generate", "--target", "c99"])
        .arg(&fsm_src)
        .arg("--out")
        .arg(&gen_dir)
        .status()
        .unwrap_or_else(|e| {
            panic!(
                "failed to spawn `{fsm} generate` ({e}). Set FSM=/abs/path/to/fsm \
                 or put `fsm` on PATH."
            )
        });
    assert!(status.success(), "`{fsm} generate` failed: {status}");

    // 2. Compile the generated C + HAL + externs into a static lib.
    //    Strict flags mirror every other FSM-Lang integration so the C
    //    contract is exercised identically from Rust.
    cc::Build::new()
        .file(gen_dir.join("Motor.c"))
        .file(manifest_dir.join("hal.c"))
        .file(manifest_dir.join("motor_externs.c"))
        .file(manifest_dir.join("ffi_helpers.c"))
        .include(&gen_dir)
        .include(&manifest_dir)
        .flag("-std=c99")
        .flag("-Wall")
        .flag("-Wextra")
        .flag("-Wpedantic")
        .flag("-Werror")
        .warnings(false) // we set our own strict flags above
        .compile("motor_fsm");

    // 3. Tell rustc where the linkable lib is (cc does this for the
    //    static lib; the search path is OUT_DIR by convention).
    println!("cargo:rustc-link-search=native={}", out_dir.display());
}
