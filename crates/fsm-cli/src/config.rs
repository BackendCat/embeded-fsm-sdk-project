//! `fsm.toml` loader.
//!
//! Walks upwards from a starting directory looking for `fsm.toml`. The
//! nearest match wins; parents are NOT merged (Doc 18 §6.1 "Multiple
//! fsm.toml files in the directory hierarchy" — "Only the nearest …
//! shadows all parents"). The CLI uses the loaded config as a baseline and
//! lets explicit CLI flags override at the call site.
//!
//! Only the fields the v1.0 subcommands actually consume are parsed. The
//! struct is `#[serde(default, deny_unknown_fields = false)]` so a user can
//! keep forward-looking sections (`[simulate]`, `[lsp]`, `[generate.cpp]`)
//! in the same file without us screaming at them.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

/// Top-level fsm.toml schema. Mirrors the example in Doc 18 §6.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct FsmToml {
    pub compiler: CompilerSection,
    pub generate: GenerateSection,
    pub format: FormatSection,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct CompilerSection {
    pub max_errors: Option<u32>,
    pub warn_as_error: Option<bool>,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct GenerateSection {
    pub target: Option<String>,
    pub strategy: Option<String>,
    pub out: Option<PathBuf>,
    pub queue_size: Option<u8>,
    pub queue_overflow: Option<String>,
    pub isr_safe: Option<bool>,
    pub license: Option<String>,
    pub report_memory: Option<bool>,
    /// C headers whose function declarations are imported as `extern`s for
    /// every generate invocation in this project (Doc 18 §5/§6). CLI
    /// `--import-header` flags are *appended* to this list (both sources
    /// contribute; neither shadows the other, mirroring how multiple
    /// `--import-header` flags accumulate).
    #[serde(default)]
    pub import_headers: Vec<PathBuf>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct FormatSection {
    pub indent_size: Option<u8>,
    pub bracket_style: Option<String>,
}

/// Errors surfaced from [`load`]. Distinct from a missing file (which is
/// silently mapped to `Ok(None)`); a *malformed* file is an error worth
/// failing the run because the user almost certainly wanted it loaded.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse error in {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
}

/// Walk upwards from `start_dir` until we find an `fsm.toml`, then parse it.
///
/// Returns `Ok(None)` if no `fsm.toml` is found anywhere on the way up.
/// Returns `Err(...)` if a file is found but cannot be read or parsed —
/// that is a user-actionable problem and should surface to exit-code 4.
pub fn load(start_dir: &Path) -> Result<Option<(PathBuf, FsmToml)>, ConfigError> {
    let mut cur = Some(start_dir);
    while let Some(dir) = cur {
        let candidate = dir.join("fsm.toml");
        if candidate.is_file() {
            let raw = std::fs::read_to_string(&candidate).map_err(|e| ConfigError::Io {
                path: candidate.clone(),
                source: e,
            })?;
            let cfg = toml::from_str::<FsmToml>(&raw).map_err(|e| ConfigError::Parse {
                path: candidate.clone(),
                source: e,
            })?;
            return Ok(Some((candidate, cfg)));
        }
        cur = dir.parent();
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn missing_file_yields_none() {
        let td = tempfile::tempdir().unwrap();
        let res = load(td.path()).unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn parses_minimal_section() {
        let td = tempfile::tempdir().unwrap();
        fs::write(
            td.path().join("fsm.toml"),
            "[generate]\ntarget = \"c99\"\nstrategy = \"switch\"\n",
        )
        .unwrap();
        let (_, cfg) = load(td.path()).unwrap().expect("config loaded");
        assert_eq!(cfg.generate.target.as_deref(), Some("c99"));
        assert_eq!(cfg.generate.strategy.as_deref(), Some("switch"));
    }

    #[test]
    fn walks_upwards_until_match() {
        let td = tempfile::tempdir().unwrap();
        let nested = td.path().join("a/b/c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(td.path().join("fsm.toml"), "[compiler]\nmax_errors = 5\n").unwrap();
        let (path, cfg) = load(&nested).unwrap().expect("walked up");
        assert_eq!(path.parent().unwrap(), td.path());
        assert_eq!(cfg.compiler.max_errors, Some(5));
    }

    #[test]
    fn parse_error_propagates() {
        let td = tempfile::tempdir().unwrap();
        fs::write(td.path().join("fsm.toml"), "not = toml = at = all").unwrap();
        let err = load(td.path()).unwrap_err();
        assert!(matches!(err, ConfigError::Parse { .. }));
    }
}
