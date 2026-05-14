//! `fsm fmt` — canonical formatter.
//!
//! Doc 23 §9 normative behaviour:
//!   - `fsm fmt f.fsm && fsm fmt --check f.fsm` → second invocation exits 0.
//!
//! Modes:
//!   - default     — read each file, format, write back in place.
//!   - `--check`   — read each file, format, exit 1 if any differ from source.
//!   - `--stdin`   — read from stdin, write formatted output to stdout.

use std::io::{self, Read, Write};
use std::process::ExitCode;

use fsm_formatter::{format_string, FormatOptions};

use crate::cli::FmtArgs;

pub fn run(args: FmtArgs) -> ExitCode {
    if args.stdin {
        return run_stdin();
    }

    if args.files.is_empty() {
        eprintln!("error: no input files");
        return ExitCode::from(2);
    }

    let opts = FormatOptions::default();
    let mut any_unformatted = false;
    let mut any_error = false;

    for path in &args.files {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: cannot read {}: {}", path.display(), e);
                return ExitCode::from(3);
            }
        };

        let formatted = match format_string(&src, &opts) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: formatter refused {}: {}", path.display(), e);
                any_error = true;
                continue;
            }
        };

        if args.check {
            if formatted != src {
                // Doc 18 §5 example output: one line per non-canonical file.
                println!("not formatted: {}", path.display());
                any_unformatted = true;
            }
        } else if formatted != src {
            if let Err(e) = std::fs::write(path, &formatted) {
                eprintln!("error: cannot write {}: {}", path.display(), e);
                any_error = true;
                continue;
            }
        }
    }

    if any_error {
        return ExitCode::from(2);
    }
    if any_unformatted {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

fn run_stdin() -> ExitCode {
    let mut src = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut src) {
        eprintln!("error: stdin: {}", e);
        return ExitCode::from(2);
    }
    let formatted = match format_string(&src, &FormatOptions::default()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: formatter refused stdin: {}", e);
            return ExitCode::from(1);
        }
    };
    let mut out = io::stdout().lock();
    if let Err(e) = out.write_all(formatted.as_bytes()) {
        eprintln!("error: stdout: {}", e);
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}
