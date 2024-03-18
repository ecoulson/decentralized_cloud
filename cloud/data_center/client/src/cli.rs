use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "datacenter")]
#[command(about = "Data center", long_about = None)]
pub struct Cli {
    /// Host name of the data center
    pub host_name: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    Instance(InstanceArguments),
    Machine(MachineArguments),
    Compute(ComputeArguments),
    Storage(StorageArguments),
    Os(OperatingSystemArguments),
}

#[derive(Debug, Args)]
pub struct InstanceArguments {
    #[command(subcommand)]
    pub instance: InstanceCommands,
}

#[derive(Debug, Subcommand)]
pub enum InstanceCommands {
    ProvisionInstance(ProvisionInstanceArguments),
    StopInstance(StopInstanceArguments),
    StartInstance(StartInstanceArguments),
    ListInstances,
}

#[derive(Debug, Args)]
pub struct MachineArguments {
    #[command(subcommand)]
    pub machine: MachineCommands,
}

#[derive(Debug, Subcommand)]
pub enum MachineCommands {
    CreateMachine(CreateMachineArguments),
    ListMachines,
}

#[derive(Debug, Args)]
pub struct ComputeArguments {
    #[command(subcommand)]
    pub compute: ComputeCommands,
}

#[derive(Debug, Subcommand)]
pub enum ComputeCommands {
    Up(UpArguments),
}

#[derive(Debug, Args)]
pub struct UpArguments {
    #[command(subcommand)]
    pub up: UpCommands,
    #[arg(short, long)]
    pub ram_mb: u32,
    #[arg(short, long)]
    pub disk_mb: u32,
    #[arg(short, long)]
    pub vcpus: u32,
}

#[derive(Debug, Subcommand)]
pub enum UpCommands {
    LocalImage(UpLocalImageArguments),
}

#[derive(Debug, Args)]
pub struct UpLocalImageArguments {
    pub local_path: String,
    pub storage_path: String
}

#[derive(Debug, Args)]
pub struct StartInstanceArguments {
    pub instance_id: String,
}

#[derive(Debug, Args)]
pub struct StorageArguments {
    #[command(subcommand)]
    pub storage: StorageCommands,
}

#[derive(Debug, Subcommand)]
pub enum StorageCommands {
    UploadFile(UploadFileArguments),
    DownloadFile(DownloadFileArguments),
}

#[derive(Debug, Args)]
pub struct UploadFileArguments {
    pub local_file_path: String,
    pub storage_file_path: String,
}

#[derive(Debug, Args)]
pub struct DownloadFileArguments {
    pub storage_path: String,
    pub local_path: String,
}

#[derive(Debug, Args)]
pub struct OperatingSystemArguments {
    #[command(subcommand)]
    pub os: OperatingSystemCommands,
}

#[derive(Debug, Subcommand)]
pub enum OperatingSystemCommands {
    UploadImage(UploadImageArguments),
    DownloadImage(DownloadImageArguments),
    GetImageMetadata(GetImageMetadataArguments),
    ListImageMetadata,
}

#[derive(Debug, Args)]
pub struct GetImageMetadataArguments {
    /// Image id
    pub image_id: String,
}

#[derive(Debug, Args)]
pub struct ProvisionInstanceArguments {
    /// Machine id
    pub machine_id: String,
}

#[derive(Debug, Args)]
pub struct StopInstanceArguments {
    pub instance_id: String,
}

#[derive(Debug, Args)]
pub struct CreateMachineArguments {
    /// Image id
    pub image_id: String,
    /// Ram mb
    pub ram_mb: u32,
    /// Disk mb
    pub disk_mb: u32,
    /// VCPUs mb
    pub vcpus: u32,
}

#[derive(Debug, Args)]
pub struct UploadImageArguments {
    /// image id
    #[arg(short, long)]
    pub image_id: Option<String>,
    /// Path to image
    pub source_image_path: String,
    /// Path to image storage
    pub destination_image_path: String,
}

#[derive(Debug, Args)]
pub struct DownloadImageArguments {
    /// image id
    pub image_id: String,
    /// Path to write image to
    pub destination_path: String,
}

pub fn parse_cli() -> Cli {
    Cli::parse()
}
