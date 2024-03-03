use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "dcns")]
#[command(about = "Data center network cli", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    List,
    Register(RegisterDataCenterCommand)
}

#[derive(Debug, Args)]
pub struct RegisterDataCenterCommand {
    pub host_name: String
}

pub fn parse_cli() -> Cli {
    Cli::parse()
}
