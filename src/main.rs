#[cfg(not(target_os = "linux"))]
compile_error!("Orev supports Linux only.");

mod cli;
mod config;
mod ops;
mod tool;

use clap::Parser;
use cli::{CaseCommands, Cli, Commands, SampleCommands};
use std::process::ExitCode;

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Commands::Case {
            command: CaseCommands::Add { name },
        } => ops::case::run_add(&name),
        Commands::Sample {
            command: SampleCommands::Ingest { paths, case },
        } => ops::sample::run_ingest(&paths, case.as_deref()),
        Commands::Init { path, workspace } => {
            if workspace {
                ops::init::run_workspace_init(path)
            } else {
                ops::init::run_init(path)
            }
        }
        Commands::Status => {
            ops::status::run_status();
            Ok(())
        }
        Commands::Doctor => ops::doctor::run_doctor(),
        Commands::Run {
            tool_version,
            no_version_probe,
            command,
        } => {
            return match ops::run::execute(&command, tool_version, no_version_probe) {
                Ok(code) => ExitCode::from(code as u8),
                Err(error) => {
                    eprintln!("Error: {error}");
                    ExitCode::FAILURE
                }
            };
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}
