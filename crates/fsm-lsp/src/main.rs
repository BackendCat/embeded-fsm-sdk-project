//! `fsm-lang-server` — the LSP server binary (Doc 14 §1, Doc 26 §2).
//!
//! L1 ships the stdio transport only (the Doc 14 §1 default). `--version`
//! is supported because editors probe it. `--port N` (TCP alternate, Doc
//! 14 §1) is deliberately NOT implemented in L1 — it is listed in Doc 14
//! but is not part of the Doc 26 §8 L1 deliverable; adding it now would be
//! out-of-scope. A clear message tells the user it is unimplemented rather
//! than silently ignoring the flag.

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("--version") | Some("-V") => {
            // Editors parse this to gate features by server version.
            println!("fsm-lang-server {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help") | Some("-h") => {
            print!(
                "fsm-lang-server {} — FSM Studio Language Server\n\
                 \n\
                 USAGE:\n    \
                 fsm-lang-server            Run over stdio (default)\n    \
                 fsm-lang-server --version  Print version\n",
                env!("CARGO_PKG_VERSION")
            );
            ExitCode::SUCCESS
        }
        Some("--port") => {
            // Doc 14 §1 TCP alternate is a post-L1 deliverable. Fail loud
            // rather than silently fall through to stdio (silent-wrong is
            // a cardinal sin on this project).
            eprintln!(
                "error: --port (TCP transport) is not implemented in this \
                 server build; only the stdio transport is available"
            );
            ExitCode::from(2)
        }
        Some(other) => {
            eprintln!("error: unknown argument {other:?}; try --help");
            ExitCode::from(2)
        }
        None => {
            // Default + only L1 transport: stdio. tokio multi-thread
            // runtime (Doc 26 §2.1 feature set).
            let rt = match tokio::runtime::Runtime::new() {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("error: failed to start async runtime: {e}");
                    return ExitCode::FAILURE;
                }
            };
            rt.block_on(fsm_lsp::run_stdio());
            ExitCode::SUCCESS
        }
    }
}
