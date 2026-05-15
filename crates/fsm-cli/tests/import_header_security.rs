//! Security tests for the `--import-header` / `fsm.toml import_headers`
//! file-read surface (SEC-P0-1, Doc 00 §G-02 / Doc 18 §10).
//!
//! These mirror `crates/fsm-parser/tests/import_security.rs` (the DSL
//! `import "..."` security suite) for the *header* surface. The threat
//! model is identical: the compiler runs on shared CI, and a project file
//! that travels with a (possibly hostile) repo must not be able to read an
//! arbitrary host file or OOM the host.
//!
//! Trust model under test (the deliberate SEC-P0-1 absolute-path decision):
//!
//! - `fsm.toml import_headers` is **attacker-controlled** (the `fsm.toml`
//!   ships with the repo) → routed through the SAME containment check as
//!   a DSL `import "..."` (shape-reject `..`/NUL/absolute → canonicalize →
//!   workspace-root prefix). Escapes are rejected with a clean non-zero
//!   exit, NEVER an arbitrary-file read.
//! - `--import-header` is **trusted invocation input** (same trust as the
//!   `.fsm` path arg) → absolute vendored-HAL paths are the documented
//!   normal use and remain allowed; only a NUL-byte shape reject + the
//!   universal DoS size cap apply.
//!
//! Every test asserts BEHAVIOUR (exit code + the build did NOT read the
//! target / did NOT OOM), never symbol-presence.

#[path = "common/mod.rs"]
mod common;

use std::fs;
use std::time::{Duration, Instant};

use assert_cmd::Command as Assert;

/// A tiny machine that declares no externs (imports come from the header).
const FSM_SRC: &str = "language fsm 2.0\n\nmachine M {\n  events { GO }\n  \
                        initial S\n  state S { on GO -> S }\n}\n";

/// `fsm.toml import_headers = ["../../../etc/passwd"]` — the canonical
/// path-traversal exfiltration attempt. MUST be rejected at the shape
/// stage (before any I/O), exit 1, and the build must NOT succeed (it
/// never reads /etc/passwd).
#[test]
fn fsm_toml_import_headers_dotdot_traversal_is_rejected_exit_1() {
    let td = tempfile::tempdir().unwrap();
    fs::write(td.path().join("m.fsm"), FSM_SRC).unwrap();
    fs::write(
        td.path().join("fsm.toml"),
        "[generate]\nimport_headers = [\"../../../../../../etc/passwd\"]\n",
    )
    .unwrap();
    let out = td.path().join("out");

    let assert = Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg(td.path().join("m.fsm"))
        .arg("--out")
        .arg(&out)
        .assert()
        .failure()
        .code(1);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("not a safe path") || stderr.contains(".."),
        "expected a shape/containment rejection diagnostic, got:\n{stderr}"
    );
    // The exfiltration target was never read into the generated tree.
    assert!(
        !out.join("M.c").exists(),
        "generate must not have proceeded"
    );
}

/// `fsm.toml import_headers = ["/etc/passwd"]` — absolute path from the
/// attacker-controlled config. MUST be rejected (absolute paths fail the
/// shape check), exit 1.
#[test]
fn fsm_toml_import_headers_absolute_path_is_rejected_exit_1() {
    let td = tempfile::tempdir().unwrap();
    fs::write(td.path().join("m.fsm"), FSM_SRC).unwrap();
    fs::write(
        td.path().join("fsm.toml"),
        "[generate]\nimport_headers = [\"/etc/passwd\"]\n",
    )
    .unwrap();
    let out = td.path().join("out");

    let assert = Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg(td.path().join("m.fsm"))
        .arg("--out")
        .arg(&out)
        .assert()
        .failure()
        .code(1);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("not a safe path") || stderr.contains("absolute"),
        "expected an absolute-path rejection, got:\n{stderr}"
    );
    assert!(!out.join("M.c").exists());
}

/// A workspace-relative symlink in the repo that points OUTSIDE the
/// workspace, referenced from `fsm.toml import_headers`. This is the
/// symlink-escape the shape check alone cannot catch — only
/// canonicalize-plus-prefix does. Mirrors `import_security.rs::
/// rejects_symlink_escape`. MUST be rejected exit 1; the outside file is
/// never imported.
#[test]
#[cfg(unix)]
fn fsm_toml_import_headers_symlink_escape_is_rejected_exit_1() {
    use std::os::unix::fs::symlink;

    let workspace = tempfile::tempdir().unwrap();
    fs::write(workspace.path().join("m.fsm"), FSM_SRC).unwrap();

    // A real header OUTSIDE the workspace (a "secret" the attacker wants).
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret.h");
    fs::write(&secret, "void leaked_secret_fn(void);\n").unwrap();

    // A symlink INSIDE the workspace pointing at it.
    let link = workspace.path().join("looks_local.h");
    symlink(&secret, &link).expect("symlink");
    assert!(link.exists(), "sanity: symlink resolves");

    fs::write(
        workspace.path().join("fsm.toml"),
        "[generate]\nimport_headers = [\"looks_local.h\"]\n",
    )
    .unwrap();
    let out = workspace.path().join("out");

    let assert = Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg(workspace.path().join("m.fsm"))
        .arg("--out")
        .arg(&out)
        .assert()
        .failure()
        .code(1);
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("outside the workspace"),
        "expected an OutsideWorkspace containment rejection, got:\n{stderr}"
    );
    // Proof the escape did NOT exfiltrate: the symlinked extern never
    // reached the generated tree (generate aborted before codegen).
    assert!(!out.join("M.c").exists());
}

/// NUL byte in an `fsm.toml import_headers` entry — defensive shape reject
/// applied to BOTH trust levels. exit 1.
#[test]
fn fsm_toml_import_headers_nul_byte_is_rejected_exit_1() {
    let td = tempfile::tempdir().unwrap();
    fs::write(td.path().join("m.fsm"), FSM_SRC).unwrap();
    // A NUL in a TOML string literal is written via the \u escape.
    fs::write(
        td.path().join("fsm.toml"),
        "[generate]\nimport_headers = [\"ev\\u0000il.h\"]\n",
    )
    .unwrap();
    let out = td.path().join("out");

    Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg(td.path().join("m.fsm"))
        .arg("--out")
        .arg(&out)
        .assert()
        .failure()
        .code(1);
    assert!(!out.join("M.c").exists());
}

/// The explicit, named opt-in `[generate] allow_unscoped_import_headers =
/// true` downgrades `fsm.toml import_headers` to the trusted-invoker trust
/// level so a vendored HAL at an ABSOLUTE out-of-tree path is permitted —
/// proving the escape hatch is real, deliberate, and NOT a silent allow
/// (it is off by default; the prior tests prove the default rejects).
#[test]
fn opt_in_allows_absolute_vendored_header_from_fsm_toml() {
    // The "vendored HAL" lives OUTSIDE the project tree.
    let vendor = tempfile::tempdir().unwrap();
    let hal = vendor.path().join("vendor_hal.h");
    fs::write(&hal, "uint16_t vendor_read(uint8_t ch);\n").unwrap();

    let proj = tempfile::tempdir().unwrap();
    fs::write(
        proj.path().join("m.fsm"),
        "language fsm 2.0\n\nmachine M {\n  events { GO }\n  initial S\n  \
         state S { on GO -> S : vendor_read(0) }\n}\n",
    )
    .unwrap();
    fs::write(
        proj.path().join("fsm.toml"),
        format!(
            "[generate]\nallow_unscoped_import_headers = true\n\
             import_headers = [\"{}\"]\n",
            hal.display()
        ),
    )
    .unwrap();
    let out = proj.path().join("out");

    Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg(proj.path().join("m.fsm"))
        .arg("--out")
        .arg(&out)
        .assert()
        .success();
    // The out-of-tree vendored extern WAS imported (opt-in honoured).
    let impl_h = fs::read_to_string(out.join("M_impl.h")).unwrap();
    assert!(
        impl_h.contains("uint16_t vendor_read(uint8_t ch);"),
        "opt-in must permit the out-of-tree vendored header; impl.h:\n{impl_h}"
    );
}

/// `--import-header /etc/passwd` (an ABSOLUTE CLI path). Per the deliberate
/// trust decision the CLI flag is trusted invocation input, so the path is
/// NOT containment-rejected — but `/etc/passwd` is not a parseable C
/// header, so the extractor simply yields zero externs and `generate`
/// succeeds WITHOUT leaking its contents into the output. This proves the
/// CLI-flag absolute path stays usable (no functional regression) while the
/// real exfiltration surface (`fsm.toml`, tested above) is the one locked.
#[test]
fn cli_flag_absolute_path_is_allowed_but_does_not_leak() {
    let td = tempfile::tempdir().unwrap();
    fs::write(td.path().join("m.fsm"), FSM_SRC).unwrap();
    let out = td.path().join("out");

    Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg("--import-header")
        .arg("/etc/passwd")
        .arg(td.path().join("m.fsm"))
        .arg("--out")
        .arg(&out)
        .assert()
        .success();
    // Generated; but /etc/passwd content did not become externs (it is
    // not C function declarations) — no exfiltration via the CLI surface.
    let impl_h = fs::read_to_string(out.join("M_impl.h")).unwrap();
    assert!(
        !impl_h.contains("root:") && !impl_h.contains("/bin/"),
        "no /etc/passwd content may appear in generated C; impl.h:\n{impl_h}"
    );
}

/// DoS: a header file LARGER than the 1 MiB input cap. MUST be rejected
/// with a clean non-zero exit (3, like any other unreadable header) and
/// MUST NOT OOM. Uses a real on-disk oversized file.
#[test]
fn oversized_header_is_rejected_not_ooming() {
    let td = tempfile::tempdir().unwrap();
    fs::write(td.path().join("m.fsm"), FSM_SRC).unwrap();
    // 2 MiB of valid-ASCII filler — comfortably over the 1 MiB cap. Cheap
    // to create; the point is the read is refused BEFORE allocation.
    let big = td.path().join("huge.h");
    fs::write(&big, vec![b'/'; 2 * 1024 * 1024]).unwrap();
    let out = td.path().join("out");

    let start = Instant::now();
    let assert = Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg("--import-header")
        .arg(&big)
        .arg(td.path().join("m.fsm"))
        .arg("--out")
        .arg(&out)
        .assert()
        .failure()
        .code(3);
    // Bounded behaviour: rejection is near-instant (metadata fast-path),
    // never a slow OOM grind.
    assert!(
        start.elapsed() < Duration::from_secs(30),
        "oversized-header rejection must be bounded/fast"
    );
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("limit"),
        "expected a size-limit diagnostic, got:\n{stderr}"
    );
    assert!(!out.join("M.c").exists());
}

/// DoS: `--import-header /dev/zero` — an UNSIZED stream that yields
/// infinite bytes (metadata reports len 0, so only the bounded read saves
/// us). MUST terminate and reject, NEVER hang or OOM. The defining
/// SEC-P0-1 DoS case.
#[test]
#[cfg(unix)]
fn dev_zero_header_is_bounded_not_infinite() {
    if !std::path::Path::new("/dev/zero").exists() {
        return;
    }
    let td = tempfile::tempdir().unwrap();
    fs::write(td.path().join("m.fsm"), FSM_SRC).unwrap();
    let out = td.path().join("out");

    let start = Instant::now();
    Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg("--import-header")
        .arg("/dev/zero")
        .arg(td.path().join("m.fsm"))
        .arg("--out")
        .arg(&out)
        .timeout(Duration::from_secs(60))
        .assert()
        .failure()
        .code(3);
    assert!(
        start.elapsed() < Duration::from_secs(60),
        "/dev/zero must be bounded — a hang here is the SEC-P0-1 DoS"
    );
    assert!(!out.join("M.c").exists());
}

/// REL-P2-1 (folded in): a `fsm.toml` LARGER than the cap is rejected with
/// a clean exit-4 config error, not an unbounded `read_to_string` OOM.
#[test]
fn oversized_fsm_toml_is_rejected_exit_4_not_ooming() {
    let td = tempfile::tempdir().unwrap();
    fs::write(td.path().join("m.fsm"), FSM_SRC).unwrap();
    // 2 MiB of TOML comment lines — syntactically valid TOML, but over the
    // read cap, so it is refused before the parser ever sees it.
    let mut huge = String::with_capacity(2 * 1024 * 1024 + 16);
    huge.push_str("[generate]\n");
    while huge.len() < 2 * 1024 * 1024 {
        huge.push_str("# filler comment line to inflate the toml file\n");
    }
    fs::write(td.path().join("fsm.toml"), &huge).unwrap();
    let out = td.path().join("out");

    let start = Instant::now();
    let assert = Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg(td.path().join("m.fsm"))
        .arg("--out")
        .arg(&out)
        .assert()
        .failure()
        .code(4);
    assert!(
        start.elapsed() < Duration::from_secs(30),
        "oversized fsm.toml rejection must be bounded"
    );
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).into_owned();
    assert!(
        stderr.contains("limit"),
        "expected a size-limit diagnostic for fsm.toml, got:\n{stderr}"
    );
}

/// Positive control mirroring `import_security.rs::accepts_workspace_
/// relative`: a legitimate IN-workspace relative header listed in
/// `fsm.toml import_headers` still imports cleanly WITH containment
/// enforced (the hardening did not break the valid path). The behavioural
/// end-to-end gcc+RUN proof lives in `import_header_e2e.rs`; this is the
/// fast CLI-level confirmation that containment accepts the good case.
#[test]
fn in_workspace_relative_header_still_imports_with_containment_on() {
    let td = tempfile::tempdir().unwrap();
    fs::write(
        td.path().join("hal.h"),
        "uint16_t adc_get(uint8_t ch);\nvoid led(bool enable);\n",
    )
    .unwrap();
    fs::write(
        td.path().join("fsm.toml"),
        "[generate]\nimport_headers = [\"hal.h\"]\n",
    )
    .unwrap();
    fs::write(
        td.path().join("m.fsm"),
        "language fsm 2.0\n\nmachine M {\n  events { GO }\n  initial S\n  \
         state S { on GO -> S : led(); adc_get(0) }\n}\n",
    )
    .unwrap();
    let out = td.path().join("out");

    Assert::cargo_bin("fsm")
        .unwrap()
        .arg("generate")
        .arg(td.path().join("m.fsm"))
        .arg("--out")
        .arg(&out)
        .assert()
        .success();

    let impl_h = fs::read_to_string(out.join("M_impl.h")).unwrap();
    assert!(
        impl_h.contains("uint16_t adc_get(uint8_t ch);")
            && impl_h.contains("void led(bool enable);"),
        "a legitimate in-workspace header must still import; impl.h:\n{impl_h}"
    );
}
