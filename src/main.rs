use clap::Parser;
use std::process;

use akashic::cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();
    let exit_code = match cli.command {
        Commands::Inspect { path, json } => {
            if let Err(err) = akashic::cli::run_inspect(&path, json) {
                eprintln!("Error: {}", err);
                1
            } else {
                0
            }
        }
        Commands::Lint { path, json } => match akashic::cli::run_lint(&path, json) {
            Ok(healthy) => {
                if healthy {
                    0
                } else {
                    1
                }
            }
            Err(err) => {
                eprintln!("Error: {}", err);
                1
            }
        },
        Commands::Query {
            vault,
            seed,
            mode,
            as_of,
            scope,
            json,
        } => match akashic::cli::run_query(&vault, &seed, mode, as_of.as_deref(), &scope, json) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("Error: {}", err);
                1
            }
        },
        Commands::Plan { vault, out } => {
            if let Err(err) = akashic::cli::run_plan(&vault, out.as_deref()) {
                eprintln!("Error: {}", err);
                1
            } else {
                0
            }
        }
        Commands::Apply {
            vault,
            plan,
            dry_run,
        } => {
            if let Err(err) = akashic::cli::run_apply(&vault, &plan, dry_run) {
                eprintln!("Error: {}", err);
                1
            } else {
                0
            }
        }
        Commands::Export {
            vault,
            format,
            out,
            json,
        } => {
            if let Err(err) = akashic::cli::run_export(&vault, &format, &out, json) {
                eprintln!("Error: {}", err);
                1
            } else {
                0
            }
        }
    };

    if exit_code != 0 {
        process::exit(exit_code);
    }
}
