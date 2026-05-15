//! `fsm decompile` — IR JSON back to canonical `.fsm` source.
//!
//! v1.0 stub: round-tripping IR→source is non-trivial because the analyzer
//! drops trivia (comments, doc annotations) during lowering. Surfacing a
//! pretty stub now keeps the CLI surface complete without making promises
//! the implementation can't keep. Real round-trip lands once the IR adds
//! the doc-comment preservation slots planned for v1.1.

use std::process::ExitCode;

use crate::cli::DecompileArgs;

pub(crate) fn run(_args: DecompileArgs) -> ExitCode {
    eprintln!("error: `fsm decompile` is not implemented in v1.0");
    eprintln!("       (IR→.fsm round-trip is on the v1.1 roadmap)");
    ExitCode::from(2)
}
