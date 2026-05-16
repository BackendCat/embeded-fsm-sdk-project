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
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
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

    /// Bounded explicit-state verification: prove deadlock-freedom +
    /// report unreachable states (drives the shipped interpreter as the
    /// semantic oracle). v1.4-W2: composite / parallel / history / timer /
    /// submachine FSMs (W1 was flat single-machine only).
    Verify(VerifyArgs),

    /// Trace differential replay (v1.4-W3): capture execution traces of a
    /// suite's FSMs into a frozen `fsm-trace/v1` baseline corpus
    /// (`--record`), then on later builds re-run and report any semantic
    /// drift from that baseline (`--check`). The regression oracle that
    /// makes a refactor that silently changes runtime semantics fail. Drives
    /// the shipped interpreter via `execute_trace` — it forks no semantics.
    Baseline(BaselineArgs),
}

#[derive(Args, Debug)]
pub(crate) struct ParseArgs {
    /// Print the Concrete Syntax Tree to stdout as JSON.
    #[arg(long)]
    pub(crate) emit_cst: bool,
    /// Print the Abstract Syntax Tree to stdout as JSON.
    #[arg(long)]
    pub(crate) emit_ast: bool,
    /// Input .fsm file(s).
    #[arg(required = true)]
    pub(crate) files: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub(crate) struct CheckArgs {
    /// Emit diagnostics as a JSON array on stdout instead of human format.
    #[arg(long)]
    pub(crate) json: bool,
    /// Treat all warnings as errors.
    #[arg(long)]
    pub(crate) warn_as_error: bool,
    /// Input .fsm file(s).
    #[arg(required = true)]
    pub(crate) files: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub(crate) struct GenerateArgs {
    /// Code generation target. v1.0 ships only `c99`.
    #[arg(short, long, default_value = "c99")]
    pub(crate) target: String,
    /// Output directory.
    #[arg(short, long, default_value = "generated")]
    pub(crate) out: PathBuf,
    /// Dispatch strategy.
    #[arg(long, value_parser = ["switch", "table", "auto"], default_value = "auto")]
    pub(crate) strategy: String,
    /// Event queue capacity (must be a power of two).
    #[arg(long)]
    pub(crate) queue_size: Option<u8>,
    /// SPDX license identifier embedded in every emitted file (Doc 00 §10.4).
    #[arg(long, default_value = "MIT")]
    pub(crate) license: String,
    /// Print the computed memory budget after emission (Doc 00 §7.11).
    #[arg(long)]
    pub(crate) report_memory: bool,
    /// Also write the IR JSON next to the generated sources.
    #[arg(long)]
    pub(crate) emit_ir: bool,
    /// Import `extern` declarations from an existing C header instead of
    /// hand-writing them in the `.fsm`. Repeatable for several headers.
    /// Functions the lightweight extractor cannot model (macros, typedefs,
    /// function-pointer params, …) are skipped with a note, never
    /// misparsed. A `.fsm` `extern` of the same name wins over an imported
    /// one. Also settable via `fsm.toml` `[generate] import_headers`.
    /// (Doc 18 §5 `fsm generate` — supported C subset + C→IR type table.)
    #[arg(long = "import-header", value_name = "PATH")]
    pub(crate) import_header: Vec<PathBuf>,
    /// Input .fsm file(s).
    #[arg(required = true)]
    pub(crate) files: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub(crate) struct FmtArgs {
    /// Verify formatting without writing changes; exit 1 if any file is not
    /// canonical.
    #[arg(long)]
    pub(crate) check: bool,
    /// Read source from stdin and write the formatted output to stdout.
    #[arg(long, conflicts_with_all = ["files", "check"])]
    pub(crate) stdin: bool,
    /// Input .fsm file(s).
    pub(crate) files: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub(crate) struct TestArgs {
    /// Path to the test-suite root directory.
    #[arg(required = true)]
    pub(crate) dir: PathBuf,
    /// Print only failing tests.
    #[arg(long)]
    pub(crate) failing_only: bool,
    /// Treat traces lacking an `expected` block as passes instead of fails.
    /// Intended for partial-development workflows where authors are iterating
    /// on step sequences before recording the expected output. CI must NOT
    /// pass this flag — every shipped trace must declare its expected output.
    #[arg(long)]
    pub(crate) allow_empty_expected: bool,
}

#[derive(Args, Debug)]
pub(crate) struct DocArgs {
    /// Output Markdown file (defaults to stdout).
    #[arg(short, long)]
    pub(crate) out: Option<PathBuf>,
    /// Input .fsm file.
    #[arg(required = true)]
    pub(crate) file: PathBuf,
}

#[derive(Args, Debug)]
pub(crate) struct DecompileArgs {
    /// Path to the IR JSON file.
    #[arg(required = true)]
    pub(crate) ir: PathBuf,
    /// Output path; defaults to stdout.
    #[arg(short, long)]
    pub(crate) out: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub(crate) struct InitArgs {
    /// Project name (also the directory name to create).
    pub(crate) name: String,
}

#[derive(Args, Debug)]
pub(crate) struct VerifyArgs {
    /// Emit a machine-readable JSON result on stdout (per-property verdict,
    /// counterexample/witness trace, the bound + whether it was hit)
    /// instead of the human summary. The contract a CI / Make / factory
    /// step parses.
    #[arg(long)]
    pub(crate) json: bool,
    /// Verify a specific machine by name. Defaults to the first machine in
    /// the file.
    #[arg(long)]
    pub(crate) machine: Option<String>,
    /// Visited-configuration ceiling. Hitting it ⇒ INCONCLUSIVE (exit 2),
    /// never a false "verified". Defaults to the crate's conservative
    /// bound.
    #[arg(long)]
    pub(crate) max_states: Option<usize>,
    /// Explored-edge ceiling. Hitting it ⇒ INCONCLUSIVE (exit 2).
    /// Defaults to the crate's conservative bound.
    #[arg(long)]
    pub(crate) max_steps: Option<usize>,
    /// Input .fsm file.
    #[arg(required = true)]
    pub(crate) file: PathBuf,
}

#[derive(Args, Debug)]
pub(crate) struct BaselineArgs {
    /// Capture every FSM under the suite into the baseline corpus directory
    /// (`--corpus`), overwriting it. Use this once on a known-good build to
    /// freeze the regression oracle. Mutually exclusive with `--check`.
    #[arg(long, conflicts_with = "check")]
    pub(crate) record: bool,
    /// Re-run every FSM under the suite and compare against the frozen
    /// baseline corpus, reporting any semantic drift with a precise
    /// first-mismatch report. This is the default mode when neither
    /// `--record` nor `--check` is given.
    #[arg(long)]
    pub(crate) check: bool,
    /// Baseline corpus directory (the persisted `fsm-trace/v1` artifact).
    /// Defaults to `<suite>/baselines`.
    #[arg(long)]
    pub(crate) corpus: Option<PathBuf>,
    /// Emit a deterministic, schema-versioned (`fsm-trace-diff/v1`) JSON
    /// result on stdout instead of the human summary. The contract a CI /
    /// Make / factory step parses; on drift it carries the first-mismatch
    /// payload (which FSM, the step index, expected-vs-actual record).
    #[arg(long)]
    pub(crate) json: bool,
    /// The suite root: a directory of `.fsm` files (walked recursively).
    /// Each FSM is driven through the shipped interpreter via a recorded
    /// `.steps.json` trace placed beside it, or — absent one — its
    /// initialization step alone (still a real, drift-sensitive capture).
    #[arg(required = true)]
    pub(crate) suite: PathBuf,
}
