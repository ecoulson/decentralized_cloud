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
    ProvisionMachine(ProvisionMachineArguments),
    UploadImage(UploadImageArguments),
    DownloadImage(DownloadImageArguments),
    GetImageMetadata(GetImageMetadataArguments),
    CreateImageMetadata(CreateImageMetadataArguments),
    ListImageMetadata
}

#[derive(Debug, Args)]
pub struct GetImageMetadataArguments {
    /// Image id
    pub image_id: String
}

#[derive(Debug, Args)]
pub struct CreateImageMetadataArguments {
    /// Local path file
    pub local_file_path: String,
    /// Storage path
    pub storage_path: String
}

#[derive(Debug, Args)]
pub struct ProvisionMachineArguments {
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
    pub destination_path: String
}

pub fn parse_cli() -> Cli {
    Cli::parse()
}
