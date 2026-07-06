pub mod cli;
pub mod commands;
pub mod config;
pub mod errors;
pub mod git;
pub mod github;
pub mod models;
pub mod niri;
pub mod output;
pub mod paths;
pub mod process;
pub mod store;

use clap::Parser;

pub fn run() -> i32 {
    let cli = cli::Cli::parse();
    let env = commands::RealEnv::new();

    match commands::dispatch(cli.command, &env) {
        Ok(()) => 0,
        Err(errors::AppError::Command(err)) => {
            commands::print_command_error(&err, err.json);
            err.exit_code
        }
        Err(err) => {
            eprintln!("{err}");
            1
        }
    }
}

