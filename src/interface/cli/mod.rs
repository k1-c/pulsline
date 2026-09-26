//! The subcommands (docs/cli.md). Each runs on its own and assembles the
//! infra it needs.

use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::config::Paths;
use crate::core::message::Message;
use crate::core::usecase::timeline;
use crate::infra::dispatch::{self, Context};
use crate::infra::spool::Spool;

#[derive(Debug, Parser)]
#[command(
    name = "pulsline",
    version,
    about = "Journal your coding-agent work and see your day as a timeline"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print where Pulsline keeps its data.
    Paths,
    /// Work on the index built from the spool.
    #[command(subcommand)]
    Index(IndexCommand),
}

#[derive(Debug, Subcommand)]
enum IndexCommand {
    /// Take in what the spool gained since the last catch-up.
    CatchUp,
    /// Drop the index and build it again from the whole spool.
    Rebuild,
}

/// Parses the arguments and runs the subcommand.
pub fn run() -> ExitCode {
    crate::logging::init();
    let cli = Cli::parse();
    match execute(cli.command) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("pulsline: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn execute(command: Command) -> anyhow::Result<ExitCode> {
    let paths = Paths::resolve()?;
    match command {
        Command::Paths => {
            println!("data   {}", paths.data_dir.display());
            println!("spool  {}", paths.spool_dir().display());
            println!("index  {}", paths.index_path().display());
            Ok(ExitCode::SUCCESS)
        }
        Command::Index(sub) => {
            let request = match sub {
                IndexCommand::CatchUp => timeline::catch_up(),
                IndexCommand::Rebuild => timeline::rebuild(),
            };
            let cx = Context {
                spool: Spool::new(paths.spool_dir()),
                index_path: paths.index_path(),
            };
            let message = dispatch::execute(request.into(), &cx);
            report(&message)
        }
    }
}

fn report(message: &Message) -> anyhow::Result<ExitCode> {
    match message {
        Message::Failed { .. } => {
            eprintln!("pulsline: {}", message.describe());
            Ok(ExitCode::FAILURE)
        }
        _ => {
            println!("{}", message.describe());
            Ok(ExitCode::SUCCESS)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arguments_are_well_formed() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
