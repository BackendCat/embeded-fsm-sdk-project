//! `fsm` — command-line entry point for FSM Studio.
//!
//! Phase 0 scaffold. Subcommand surface matches the v1.0 scope agreed in the
//! task brief (`check`, `generate`, `fmt`, `parse`, `test`, `doc`, `decompile`,
//! `init`). Each handler prints a "not yet implemented" notice to stderr and
//! returns success — the binary exists so build/test scripts have something
//! to invoke. Real wiring lands in Phase 1.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "fsm",
    version,
    about = "FSM Studio toolchain (v1.0 scaffold — subcommands are stubs)",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Parse one or more .fsm files and report syntax diagnostics.
    Parse {
        /// Input .fsm file(s).
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },

    /// Parse + semantic analysis + nondeterminism check. No output files.
    Check {
        /// Input .fsm file(s).
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },

    /// Compile and emit target source files.
    Generate {
        /// Code generation target. v1.0 ships only `c99`.
        #[arg(short, long, default_value = "c99")]
        target: String,
        /// Output directory.
        #[arg(short, long, default_value = "generated/")]
        out: PathBuf,
        /// Input .fsm file(s).
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },

    /// Format .fsm sources to canonical style.
    Fmt {
        /// Verify formatting without writing changes.
        #[arg(long)]
        check: bool,
        /// Input .fsm file(s).
        files: Vec<PathBuf>,
    },

    /// Run the conformance test suite.
    Test {
        /// Path to the test-suite root.
        #[arg(long)]
        suite: Option<PathBuf>,
    },

    /// Generate documentation from /// doc comments in .fsm sources.
    Doc {
        /// Output format (`html` or `markdown`).
        #[arg(long, default_value = "html")]
        format: String,
        /// Output directory.
        #[arg(long, default_value = "docs-out/")]
        out: PathBuf,
        /// Input .fsm file(s).
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },

    /// Decompile an IR JSON document back into .fsm source.
    Decompile {
        /// Path to the IR JSON file.
        #[arg(required = true)]
        ir: PathBuf,
        /// Output path; defaults to stdout.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },

    /// Initialize a new FSM Studio project in the current directory.
    Init {
        /// Project name (defaults to current directory name).
        #[arg(long)]
        name: Option<String>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let subcommand = match &cli.command {
        Command::Parse { .. } => "parse",
        Command::Check { .. } => "check",
        Command::Generate { .. } => "generate",
        Command::Fmt { .. } => "fmt",
        Command::Test { .. } => "test",
        Command::Doc { .. } => "doc",
        Command::Decompile { .. } => "decompile",
        Command::Init { .. } => "init",
    };

    eprintln!("TODO: {subcommand} not yet implemented");
    ExitCode::SUCCESS
}
