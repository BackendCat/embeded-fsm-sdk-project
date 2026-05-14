//! Security tests for `import "..."` path validation.
//!
//! Covers both layers:
//! - shape-only [`validate_import_path`] (no filesystem)
//! - filesystem [`resolve_import`] (canonicalize + workspace-root containment)
//!
//! Per Doc 00 §7.12 G-02 / audit §P1-4 — the resolver must reject
//! workspace-relative symlinks pointing outside the workspace, not just
//! the obvious traversal shapes (`../`, `/etc/passwd`).

use fsm_diagnostics::Span;
use fsm_parser::import_resolver::{resolve_import, validate_import_path, ImportError};

#[test]
fn rejects_path_traversal_with_dotdot() {
    let span = Span::new(0, 16);
    let err = validate_import_path("../escape.fsm", span).expect_err("dotdot must be rejected");
    assert!(err.message.contains(".."));
}

#[test]
fn rejects_absolute_path() {
    let span = Span::new(0, 11);
    let err = validate_import_path("/etc/passwd", span).expect_err("absolute path rejected");
    assert!(err.message.contains("absolute"));
}

#[test]
fn rejects_symlink_escape() {
    use std::fs;
    use std::os::unix::fs::symlink;
    let td = tempfile::tempdir().expect("tempdir");
    let workspace = td.path();
    // The importing file lives at workspace/inside.fsm.
    let from_file = workspace.join("inside.fsm");
    fs::write(&from_file, "// dummy importer\n").expect("write importer");

    // A symlink inside the workspace that targets a path outside the
    // workspace. Use a sibling tempdir as the escape destination so the
    // test does not depend on /tmp/foo existing.
    let escape_dir = tempfile::tempdir().expect("escape tempdir");
    let escape_target = escape_dir.path().join("escape.fsm");
    fs::write(&escape_target, "// outside\n").expect("write escape file");

    let symlink_path = workspace.join("outside_symlink");
    symlink(&escape_target, &symlink_path).expect("create symlink");

    // Sanity: the symlink resolves to a real file outside the workspace.
    assert!(symlink_path.exists());

    let err =
        resolve_import(workspace, &from_file, "outside_symlink").expect_err("symlink must escape");
    assert_eq!(err, ImportError::OutsideWorkspace);
}

#[test]
fn accepts_workspace_relative() {
    use std::fs;
    let td = tempfile::tempdir().expect("tempdir");
    let workspace = td.path();
    let from_file = workspace.join("main.fsm");
    fs::write(&from_file, "// importer\n").expect("write importer");
    let subdir = workspace.join("subdir");
    fs::create_dir_all(&subdir).expect("mkdir");
    let valid = subdir.join("valid.fsm");
    fs::write(valid, "// target\n").expect("write target");

    let resolved =
        resolve_import(workspace, &from_file, "subdir/valid.fsm").expect("inside workspace ok");
    // Canonical path comparison — both sides must canonicalize through
    // the same tempdir prefix so this assertion is robust on macOS where
    // /tmp -> /private/tmp.
    assert!(resolved.starts_with(workspace.canonicalize().unwrap()));
    assert!(resolved.ends_with("subdir/valid.fsm"));
}

#[test]
fn rejects_unresolvable() {
    use std::fs;
    let td = tempfile::tempdir().expect("tempdir");
    let workspace = td.path();
    let from_file = workspace.join("main.fsm");
    fs::write(&from_file, "// importer\n").expect("write importer");

    let err = resolve_import(workspace, &from_file, "nonexistent.fsm")
        .expect_err("missing file must error");
    assert_eq!(err, ImportError::Unresolved);
}

#[test]
fn shape_check_works_without_filesystem() {
    // The shape-only entry point must not touch the filesystem at all —
    // exercising it with bare strings and no working directory must
    // produce the right verdicts.
    assert!(validate_import_path("common/events.fsm", Span::new(0, 17)).is_ok());
    assert!(validate_import_path("./types.fsm", Span::new(0, 11)).is_ok());
    assert!(validate_import_path("/etc/passwd", Span::new(0, 11)).is_err());
    assert!(validate_import_path("../escape.fsm", Span::new(0, 13)).is_err());
    assert!(validate_import_path("", Span::new(0, 0)).is_err());
}

#[test]
fn shape_failure_goes_through_bad_shape_in_resolve() {
    // resolve_import must reject shape violations BEFORE attempting any
    // I/O. We confirm by passing a workspace path that does not exist on
    // disk — a shape failure should win before canonicalize() is ever
    // called.
    use std::path::Path;
    let workspace = Path::new("/this/path/definitely/does/not/exist");
    let from_file = Path::new("/this/path/definitely/does/not/exist/main.fsm");
    let err =
        resolve_import(workspace, from_file, "../escape.fsm").expect_err("dotdot caught at shape");
    match err {
        ImportError::BadShape { reason } => assert!(reason.contains("..")),
        other => panic!("expected BadShape, got {other:?}"),
    }
}
