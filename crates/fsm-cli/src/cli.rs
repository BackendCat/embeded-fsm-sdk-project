//! clap derive surface for the `fsm` binary.
//!
//! Each variant of [`Command`] carries a strongly typed args struct, and the
//! corresponding `cmd::<name>::run(args)` consumes that struct directly.
//! Keeping the argument types out of `main` lets us write subcommand tests
//! that construct args directly without going through clap parsing.
//!
//! Subcommand surface and flag names follow `docs/18-CLI-Specification.md`
//! verbatim. Some flags listed there for v1.1+ subcommands (`simulate`, `lsp`,
//! `ir`, `completions`) are intentionally NOT exposed here — Doc 20 §8 and
//! Doc 00 §7.2 defer those to post-v1.0.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "fsm",
    version,
    about = "FSM Studio toolchain — parse, check, generate, format, test",
    long_about = None,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Parse one or more .fsm files and report syntax diagnostics.
    Parse(ParseArgs),

    /// Parse + semantic analysis + nondeterminism check. No output files.
    Check(CheckArgs),

    /// Compile and emit target source files.
    Generate(GenerateArgs),

    /// Format .fsm sources to canonical style.
    Fmt(FmtArgs),

    /// Run the conformance test suite.
    Test(TestArgs),

    /// Generate Markdown documentation from .fsm sources.
    Doc(DocArgs),

    /// Decompile an IR JSON document back into .fsm source. (v1.0 stub.)
    Decompile(DecompileArgs),

    /// Initialize a new FSM Studio project in the current directory.
    Init(InitArgs),
}

#[derive(Args, Debug)]
pub struct ParseArgs {
    /// Print the Concrete Syntax Tree to stdout as JSON.
    #[arg(long)]
    pub emit_cst: bool,
    /// Print the Abstract Syntax Tree to stdout as JSON.
    #[arg(long)]
    pub emit_ast: bool,
    /// Input .fsm file(s).
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct CheckArgs {
    /// Emit diagnostics as a JSON array on stdout instead of human format.
    #[arg(long)]
    pub json: bool,
    /// Treat all warnings as errors.
    #[arg(long)]
    pub warn_as_error: bool,
    /// Input .fsm file(s).
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct GenerateArgs {
    /// Code generation target. v1.0 ships only `c99`.
    #[arg(short, long, default_value = "c99")]
    pub target: String,
    /// Output directory.
    #[arg(short, long, default_value = "generated")]
    pub out: PathBuf,
    /// Dispatch strategy.
    #[arg(long, value_parser = ["switch", "table", "auto"], default_value = "auto")]
    pub strategy: String,
    /// Event queue capacity (must be a power of two).
    #[arg(long)]
    pub queue_size: Option<u8>,
    /// SPDX license identifier embedded in every emitted file (Doc 00 §10.4).
    #[arg(long, default_value = "MIT")]
    pub license: String,
    /// Print the computed memory budget after emission (Doc 00 §7.11).
    #[arg(long)]
    pub report_memory: bool,
    /// Also write the IR JSON next to the generated sources.
    #[arg(long)]
    pub emit_ir: bool,
    /// Input .fsm file(s).
    #[arg(required = true)]
    pub files: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct FmtArgs {
    /// Verify formatting without writing changes; exit 1 if any file is not
    /// canonical.
    #[arg(long)]
    pub check: bool,
    /// Read source from stdin and write the formatted output to stdout.
    #[arg(long, conflicts_with_all = ["files", "check"])]
    pub stdin: bool,
    /// Input .fsm file(s).
    pub files: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct TestArgs {
    /// Path to the test-suite root directory.
    #[arg(required = true)]
    pub dir: PathBuf,
    /// Print only failing tests.
    #[arg(long)]
    pub failing_only: bool,
    /// Treat traces lacking an `expected` block as passes instead of fails.
    /// Intended for partial-development workflows where authors are iterating
    /// on step sequences before recording the expected output. CI must NOT
    /// pass this flag — every shipped trace must declare its expected output.
    #[arg(long)]
    pub allow_empty_expected: bool,
}

#[derive(Args, Debug)]
pub struct DocArgs {
    /// Output Markdown file (defaults to stdout).
    #[arg(short, long)]
    pub out: Option<PathBuf>,
    /// Input .fsm file.
    #[arg(required = true)]
    pub file: PathBuf,
}

#[derive(Args, Debug)]
pub struct DecompileArgs {
    /// Path to the IR JSON file.
    #[arg(required = true)]
    pub ir: PathBuf,
    /// Output path; defaults to stdout.
    #[arg(short, long)]
    pub out: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    /// Project name (also the directory name to create).
    pub name: String,
}
