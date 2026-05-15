//! `fsm init <name>` — scaffold a new FSM Studio project.
//!
//! Layout:
//!   `<name>/`
//!     `fsm.toml`
//!     `examples/motor.fsm`
//!     `.gitignore`
//!
//! Designed to be the smallest thing that lets a new user run
//! `cd <name> && fsm check examples/motor.fsm` and see exit-0.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::cli::InitArgs;

const EXAMPLE_FSM: &str = include_str!("../../templates/motor.fsm");
const EXAMPLE_TOML: &str = include_str!("../../templates/fsm.toml");
const EXAMPLE_GITIGNORE: &str = "generated/\ntarget/\n";

pub(crate) fn run(args: InitArgs) -> ExitCode {
    let root = PathBuf::from(&args.name);
    if root.exists() {
        eprintln!("error: {} already exists", root.display());
        return ExitCode::from(2);
    }
    if let Err(e) = fs::create_dir_all(root.join("examples")) {
        eprintln!("error: mkdir: {}", e);
        return ExitCode::from(2);
    }
    let writes = [
        (root.join("fsm.toml"), EXAMPLE_TOML),
        (root.join("examples/motor.fsm"), EXAMPLE_FSM),
        (root.join(".gitignore"), EXAMPLE_GITIGNORE),
    ];
    for (path, content) in writes {
        if let Err(e) = fs::write(&path, content) {
            eprintln!("error: write {}: {}", path.display(), e);
            return ExitCode::from(2);
        }
    }
    println!("created project {} at {}", args.name, root.display());
    ExitCode::SUCCESS
}
