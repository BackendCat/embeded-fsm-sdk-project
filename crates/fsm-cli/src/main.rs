//! `fsm` — command-line entry point for FSM Studio.
//!
//! This file is intentionally tiny: clap drives argv parsing, the per-
//! subcommand modules in `cmd/` do the actual work. The exit-code mapping
//! follows Doc 18 §3 (0 ok, 1 user/diagnostic errors, 2 tool error /
//! invalid args, 3 not found, 4 config error).

#![forbid(unsafe_code)]

use std::process::ExitCode;

use clap::Parser;

mod cli;
mod cmd;
mod config;
mod diagnostics;
mod import_header;
mod safe_io;

use cli::{Cli, Command};

fn main() -> ExitCode {
    // clap handles --help / --version / unknown-flag exits itself
    // (returning exit-code 2 per its convention, which matches Doc 18
    // §3's "tool error / invalid args" bucket).
    let cli = Cli::parse();

    match cli.command {
        Command::Parse(args) => cmd::parse::run(args),
        Command::Check(args) => cmd::check::run(args),
        Command::Generate(args) => cmd::generate::run(args),
        Command::Fmt(args) => cmd::fmt::run(args),
        Command::Test(args) => cmd::test::run(args),
        Command::Doc(args) => cmd::doc::run(args),
        Command::Decompile(args) => cmd::decompile::run(args),
        Command::Init(args) => cmd::init::run(args),
    }
}
