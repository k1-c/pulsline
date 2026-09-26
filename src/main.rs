//! The `pulsline` binary: the subcommands, the hook, and (later) the TUI and
//! the MCP server all start here.

use std::process::ExitCode;

fn main() -> ExitCode {
    pulsline::interface::cli::run()
}
