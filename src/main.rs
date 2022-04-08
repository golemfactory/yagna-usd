#![recursion_limit = "512"]

use anyhow::Result;

use std::env;
use structopt::{clap, StructOpt};

mod appkey;
mod command;
mod platform;
mod status;
mod utils;

#[allow(clippy::large_enum_variant)]
#[derive(StructOpt)]
enum Commands {
    /// Show provider status
    Status,

    #[structopt(setting = structopt::clap::AppSettings::Hidden)]
    Complete(CompleteCommand),
}

#[derive(StructOpt)]
/// Generates autocomplete script from given shell
pub struct CompleteCommand {
    /// Describes which shell to produce a completions file for
    #[structopt(
    parse(try_from_str),
    possible_values = &clap::Shell::variants(),
    case_insensitive = true
    )]
    shell: clap::Shell,
}

#[derive(StructOpt)]
#[structopt(rename_all = "kebab-case")]
#[structopt(about = clap::crate_description!())]
#[structopt(global_setting = clap::AppSettings::ColoredHelp)]
#[structopt(global_setting = clap::AppSettings::DeriveDisplayOrder)]
struct StartupConfig {
    #[structopt(flatten)]
    commands: Commands,
}

async fn my_main() -> Result</*exit code*/ i32> {
    dotenv::dotenv().ok();

    if env::var_os(env_logger::DEFAULT_FILTER_ENV).is_none() {
        env::set_var(env_logger::DEFAULT_FILTER_ENV, "info");
    }
    env_logger::init();

    let cli_args: StartupConfig = StartupConfig::from_args();

    match cli_args.commands {
        Commands::Status => status::run().await,
        Commands::Complete(complete) => {
            let binary_name = clap::crate_name!();
            println!(
                "# generating {} completions for {}",
                binary_name, complete.shell
            );
            StartupConfig::clap().gen_completions_to(
                binary_name,
                complete.shell,
                &mut std::io::stdout(),
            );
            Ok(0)
        }
    }
}

#[actix_rt::main]
async fn main() {
    std::process::exit(match my_main().await {
        Ok(code) => code,
        Err(e) => {
            log::error!("{:?}", e);
            1
        }
    });
}
