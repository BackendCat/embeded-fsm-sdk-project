//! One module per subcommand. Each exposes a single `run(args) -> ExitCode`
//! function. The router in `main.rs` calls them by name.

pub(crate) mod check;
pub(crate) mod decompile;
pub(crate) mod doc;
pub(crate) mod fmt;
pub(crate) mod generate;
pub(crate) mod init;
pub(crate) mod parse;
pub(crate) mod test;
