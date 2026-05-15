//! Shared safe-I/O + path-containment helpers for CLI file reads.
//!
//! ## Why this module exists (convergence, not a third variant)
//!
//! Doc 00 §G-02 / Doc 18 §10 mandate that the compiler is safe to run on a
//! shared CI host: a `.fsm` source (or any project file that travels with
//! it) posted to a shared build box must not be able to exfiltrate
//! arbitrary files or DoS the host. v1.0 hardened the DSL `import "path"`
//! surface with two converged primitives:
//!
//! - `fsm_parser::import_resolver::resolve_import` — shape-reject
//!   (`..`/NUL/absolute/UNC) → `canonicalize` (symlink-resolved) →
//!   workspace-root prefix containment.
//! - `fsm_parser::ParseLimits::DEFAULT.max_input_bytes` (1 MiB) — the
//!   single source of truth for the maximum source size the toolchain
//!   will read before tokenizing.
//!
//! v1.1's `--import-header` / `fsm.toml import_headers` (W5) added a NEW
//! file-read surface that bypassed BOTH of those (absolute paths returned
//! verbatim, no canonicalize, no containment, no size cap → path-traversal
//! + OOM-DoS; SEC-P0-1). The correct fix per the project's zero-legacy
//! principle is to **converge on the existing primitives**, not to write a
//! third copy of the containment/cap logic. This module therefore:
//!
//! 1. Does NOT re-implement containment — callers route attacker-controlled
//!    header paths through `import_resolver::resolve_import` directly (the
//!    one canonical primitive).
//! 2. Owns the ONE bounded-read helper ([`read_to_string_capped`]) reused
//!    by every untrusted-size CLI read (the header read AND the `fsm.toml`
//!    read — REL-P2-1, the same defect class the audit folds in here).
//! 3. Owns the ONE workspace-root resolver ([`workspace_root_for`]), moved
//!    here from `cmd::check` so `fsm check` and `fsm generate` share a
//!    single definition rather than drifting copies.
//!
//! The bounded read is keyed off `ParseLimits::DEFAULT.max_input_bytes` so
//! the header/`fsm.toml` size ceiling is *the same constant* the parser
//! already enforces on `.fsm` input — one knob, no divergence.

use std::io::{self, Read};
use std::path::{Path, PathBuf};

use fsm_parser::ParseLimits;

use crate::config;

/// The maximum number of bytes any untrusted-size CLI input file is read
/// into memory before we reject it. This is **the same** 1 MiB ceiling the
/// parser enforces on `.fsm` source (`ParseLimits::DEFAULT.max_input_bytes`)
/// — reusing the constant keeps the header / `fsm.toml` cap from drifting
/// away from the `.fsm` cap (zero-legacy: one source of truth).
pub const MAX_INPUT_BYTES: usize = ParseLimits::DEFAULT.max_input_bytes;

/// Read a file to a `String`, refusing to allocate more than
/// [`MAX_INPUT_BYTES`].
///
/// The read is bounded **before** the allocation, not after: a `metadata`
/// pre-check rejects an oversized regular file without opening the data
/// stream, and the actual read is additionally `take`-limited so an
/// *unsized* stream (a FIFO, `/dev/zero`, a growing file, a device that
/// reports `len() == 0`) cannot drive an unbounded `read_to_string` /
/// `String::with_capacity` to OOM the host. Either way the caller gets a
/// clean [`io::Error`] (kind `InvalidData`), never an OOM or a panic — the
/// SEC-P0-1 DoS-rejection contract.
///
/// On success the returned `String` is guaranteed `<= MAX_INPUT_BYTES`.
pub fn read_to_string_capped(path: &Path, max_bytes: usize) -> io::Result<String> {
    let mut file = std::fs::File::open(path)?;

    // Fast reject: a regular file whose advertised length already exceeds
    // the cap is refused without touching its contents. (A device / FIFO
    // typically reports len 0 here — that case is caught by the bounded
    // read below, so a zero/short metadata length is intentionally NOT
    // trusted as "small".)
    if let Ok(meta) = file.metadata() {
        if meta.is_file() && meta.len() > max_bytes as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "file is {} bytes, exceeding the {}-byte input limit \
                     (raise the limit only if this input is trusted)",
                    meta.len(),
                    max_bytes
                ),
            ));
        }
    }

    // Bounded read: take `max_bytes + 1` so we can DISTINGUISH "exactly at
    // the cap" (ok) from "over the cap" (reject) while never buffering more
    // than one byte past the limit — this is what makes `/dev/zero` and
    // other length-lying streams safe.
    let mut buf = Vec::new();
    let read = file
        .by_ref()
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut buf)?;
    if read > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "file exceeds the {}-byte input limit \
                 (raise the limit only if this input is trusted)",
                max_bytes
            ),
        ));
    }

    String::from_utf8(buf)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("invalid UTF-8: {e}")))
}

/// Find the workspace root for an input file. Walks upwards looking for
/// `fsm.toml`; if none is found, the file's parent directory is used as a
/// permissive fallback. The result is canonicalized so symlinks are
/// followed before any containment check happens downstream.
///
/// Moved here from `cmd::check` (SEC-P0-1 convergence): `fsm check` and
/// `fsm generate` MUST agree on what "the workspace root" is, so the
/// containment boundary is identical for a DSL `import "..."` and an
/// `fsm.toml import_headers` entry. One definition, no drift.
pub fn workspace_root_for(path: &Path) -> PathBuf {
    let start = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    if let Ok(Some((toml_path, _))) = config::load(&start) {
        if let Some(parent) = toml_path.parent() {
            if let Ok(canon) = parent.canonicalize() {
                return canon;
            }
        }
    }
    start.canonicalize().unwrap_or(start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn reads_a_small_file_unchanged() {
        let td = tempfile::tempdir().unwrap();
        let p = td.path().join("ok.txt");
        std::fs::write(&p, "hello\n").unwrap();
        assert_eq!(
            read_to_string_capped(&p, MAX_INPUT_BYTES).unwrap(),
            "hello\n"
        );
    }

    #[test]
    fn accepts_exactly_at_the_cap() {
        let td = tempfile::tempdir().unwrap();
        let p = td.path().join("edge.txt");
        let body = "a".repeat(64);
        std::fs::write(&p, &body).unwrap();
        // Cap == file size exactly → accepted (boundary is inclusive).
        assert_eq!(read_to_string_capped(&p, 64).unwrap(), body);
    }

    #[test]
    fn rejects_oversize_regular_file_via_metadata() {
        let td = tempfile::tempdir().unwrap();
        let p = td.path().join("big.txt");
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(&vec![b'x'; 4096]).unwrap();
        drop(f);
        let err = read_to_string_capped(&p, 1024).expect_err("must reject oversize");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("limit"));
    }

    #[test]
    fn rejects_oversize_one_byte_over_the_cap() {
        let td = tempfile::tempdir().unwrap();
        let p = td.path().join("over.txt");
        std::fs::write(&p, "a".repeat(65)).unwrap();
        let err = read_to_string_capped(&p, 64).expect_err("65 > 64 must reject");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(unix)]
    #[test]
    fn bounded_read_does_not_hang_or_oom_on_unsized_stream() {
        // `/dev/zero` reports metadata len 0 (so the fast-path does NOT
        // catch it) and yields infinite bytes. The `take`-bounded read
        // MUST terminate and reject rather than OOM — the SEC-P0-1 DoS
        // contract for `--import-header /dev/zero`.
        let p = Path::new("/dev/zero");
        if !p.exists() {
            return; // non-Linux unix without /dev/zero — skip
        }
        let err = read_to_string_capped(p, 1 << 16).expect_err("/dev/zero must be rejected");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn workspace_root_falls_back_to_parent_dir_without_fsm_toml() {
        let td = tempfile::tempdir().unwrap();
        let f = td.path().join("m.fsm");
        std::fs::write(&f, "language fsm 2.0\n").unwrap();
        let root = workspace_root_for(&f);
        // No fsm.toml anywhere → the file's own canonicalized dir.
        assert_eq!(root, td.path().canonicalize().unwrap());
    }

    #[test]
    fn workspace_root_is_fsm_toml_dir_when_present() {
        let td = tempfile::tempdir().unwrap();
        let nested = td.path().join("a/b");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(td.path().join("fsm.toml"), "[generate]\n").unwrap();
        let f = nested.join("m.fsm");
        std::fs::write(&f, "language fsm 2.0\n").unwrap();
        let root = workspace_root_for(&f);
        assert_eq!(root, td.path().canonicalize().unwrap());
    }
}
