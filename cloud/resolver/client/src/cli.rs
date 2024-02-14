use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "wwc")]
#[command(about = "World wide data center computer interface", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Provision(ProvisionCommand),
    DataCenter(DataCenterArgs),
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
#[command(flatten_help = true)]
pub struct DataCenterArgs {
    #[command(subcommand)]
    pub command: DataCenterCommand
}

#[derive(Debug, Subcommand)]
#[command(args_conflicts_with_subcommands = true)]
pub enum DataCenterCommand {
    Register(RegisterDataCenterCommand)
}

#[derive(Debug, Args)]
pub struct RegisterDataCenterCommand {
    pub host_name: String
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct ProvisionCommand {
    /// RAM of provisioned machine
    #[arg(short, long)]
    pub ram_mb: u32,
    /// CPU of provisioned machine
    #[arg(short, long)]
    pub vcpus: u32,
    /// Disk of provisioned machine
    #[arg(short, long)]
    pub disk_mb: u32,
    /// Optional host name of the data center
    pub host_name: Option<String>
}

pub fn parse_cli() -> Cli {
    Cli::parse()
}
