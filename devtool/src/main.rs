use anyhow::Result;
use cargo::Config as CargoConfig;
use clap::{Parser, Subcommand};

pub mod cargo_container;
pub mod cef;
mod commands;

#[derive(Parser)]
#[clap(name = "devtool", bin_name = "cargo do")]
struct App {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Build,
    Gen,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cargo_config = CargoConfig::default()?;
    let cargo = cargo_container::CargoContainer::new(&cargo_config)?;

    // for consistency, let's be sure to be in the workspace root directory
    std::env::set_current_dir(cargo.workspace.root())?;

    let args = App::parse();

    match args.command {
        Command::Build => commands::build::command(cargo).await?,
        Command::Gen => commands::gen::command(cargo).await?,
    }

    Ok(())
}
