//! Diagnostics on stderr, filtered by `$PULSLINE_LOG` (a `tracing` filter,
//! such as `debug` or `pulsline=trace`). Warnings only by default: stdout is
//! for output other programs may read.

use std::io::IsTerminal;

use tracing_subscriber::EnvFilter;

/// The environment variable holding the log filter.
pub const LOG_ENV: &str = "PULSLINE_LOG";

/// Starts logging to stderr.
pub fn init() {
    let filter = EnvFilter::try_from_env(LOG_ENV).unwrap_or_else(|_| EnvFilter::new("warn"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .try_init();
}
