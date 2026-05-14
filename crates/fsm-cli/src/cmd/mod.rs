//! One module per subcommand. Each exposes a single `run(args) -> ExitCode`
//! function. The router in `main.rs` calls them by name.

pub mod check;
pub mod decompile;
pub mod doc;
pub mod fmt;
pub mod generate;
pub mod init;
pub mod parse;
pub mod test;
