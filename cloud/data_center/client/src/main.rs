use std::{
    fs::{write, File},
    io::{Read, Seek},
    ops::Range,
};

use anyhow::{Context, Result};
use data_center_client::{
    cli::{
        parse_cli, Commands, ComputeArguments, ComputeCommands, CreateMachineArguments,
        DownloadFileArguments, DownloadImageArguments, GetImageMetadataArguments,
        InstanceArguments, InstanceCommands, MachineArguments, MachineCommands,
        OperatingSystemArguments, OperatingSystemCommands, ProvisionInstanceArguments,
        StartInstanceArguments, StopInstanceArguments, StorageArguments, StorageCommands,
        UpArguments, UpCommands, UpLocalImageArguments, UploadFileArguments, UploadImageArguments,
    },
    protos::data_center::{
        data_center_client::DataCenterClient, Chunk, CreateFileMetadataRequest,
        CreateImageMetadataRequest, CreateMachineRequest, DownloadFileRequest, FileMetadata,
        GetFileMetadataRequest, GetImageMetadataRequest, ListImageMetadataRequest,
        ListInstancesRequest, ListMachinesRequest, ProvisionInstanceRequest, Resources,
        StartInstanceRequest, StopInstanceRequest, UploadFileRequest,
    },
};
use tokio_stream::Stream;
use tonic::{transport::Channel, Request};

const ONE_MB: usize = 1048576;

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_cli();
    let mut client = DataCenterClient::connect(format!("http://{}", args.host_name)).await?;

    match args.command {
        Commands::Instance(arguments) => handle_instance_command(arguments, &mut client).await,
        Commands::Machine(arguments) => handle_machine_command(arguments, &mut client).await,
        Commands::Compute(arguments) => handle_compute_command(arguments, &mut client).await,
        Commands::Storage(arguments) => handle_storage_command(arguments, &mut client).await,
        Commands::Os(arguments) => handle_image_command(arguments, &mut client).await,
    }
}

async fn handle_machine_command(
    arguments: MachineArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    match arguments.machine {
        MachineCommands::CreateMachine(arguments) => create_machine(arguments, client).await,
        MachineCommands::ListMachines => list_machines(client).await,
    }
}

async fn handle_instance_command(
    arguments: InstanceArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    match arguments.instance {
        InstanceCommands::StopInstance(arguments) => stop_instance(arguments, client).await,
        InstanceCommands::StartInstance(arguments) => start_instance(arguments, client).await,
        InstanceCommands::ProvisionInstance(arguments) => {
            provision_instance(arguments, client).await
        }
        InstanceCommands::ListInstances => list_instances(client).await,
    }
}

async fn handle_compute_command(
    arguments: ComputeArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    match arguments.compute {
        ComputeCommands::Up(arguments) => handle_up(arguments, client).await,
    }
}

async fn handle_up(arguments: UpArguments, client: &mut DataCenterClient<Channel>) -> Result<()> {
    let resources = Resources {
        ram_mb: arguments.ram_mb,
        disk_mb: arguments.disk_mb,
        vcpus: arguments.vcpus,
    };

    match arguments.up {
        UpCommands::LocalImage(arguments) => {
            handle_up_local_image(arguments, resources, client).await
        }
    }
}

async fn handle_up_local_image(
    arguments: UpLocalImageArguments,
    resources: Resources,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    let mut source_file = File::open(&arguments.local_path).context("Should open file")?;
    let file_size = source_file
        .seek(std::io::SeekFrom::End(0))
        .context("Should seek to end")?;
    source_file
        .seek(std::io::SeekFrom::Start(0))
        .context("Should seek to start")?;
    let create_image_response = client
        .create_image_metadata(Request::new(CreateImageMetadataRequest {
            file_size,
            destination_file_path: arguments.storage_path,
        }))
        .await?;
    let image = create_image_response
        .into_inner()
        .os_image_metadata
        .expect("Should have image metadata");
    let file_metadata = &image.file_metadata.expect("Should have file metadata");
    client
        .upload_file(Request::new(stream_chunks(&file_metadata, source_file)))
        .await?;
    let create_machine_response = client
        .create_machine(Request::new(CreateMachineRequest {
            image_id: image.image_id,
            resources: Some(resources),
        }))
        .await
        .context("Failed to provision machine")?;
    let machine = create_machine_response
        .into_inner()
        .machine
        .expect("Should have machine");
    let provision_instance_response = client
        .provision_instance(Request::new(ProvisionInstanceRequest {
            machine_id: machine.machine_id,
        }))
        .await
        .context("Failed to start instance")?;
    let instance = provision_instance_response
        .into_inner()
        .instance
        .expect("Should have instance");
    dbg!(instance);

    Ok(())
}

async fn create_machine(
    arguments: CreateMachineArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    client
        .create_machine(Request::new(CreateMachineRequest {
            image_id: arguments.image_id,
            resources: Some(Resources {
                ram_mb: arguments.ram_mb,
                disk_mb: arguments.disk_mb,
                vcpus: arguments.vcpus,
            }),
        }))
        .await
        .context("Failed to provision machine")?;

    Ok(())
}

async fn stop_instance(
    arguments: StopInstanceArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    client
        .stop_instance(Request::new(StopInstanceRequest {
            instance_id: arguments.instance_id,
        }))
        .await
        .context("Failed to stop instance")?;

    Ok(())
}

async fn start_instance(
    arguments: StartInstanceArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    client
        .start_instance(Request::new(StartInstanceRequest {
            instance_id: arguments.instance_id,
        }))
        .await
        .context("Failed to start instance")?;

    Ok(())
}

async fn provision_instance(
    arguments: ProvisionInstanceArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    client
        .provision_instance(Request::new(ProvisionInstanceRequest {
            machine_id: arguments.machine_id,
        }))
        .await
        .context("Failed to start instance")?;

    Ok(())
}

async fn list_machines(client: &mut DataCenterClient<Channel>) -> Result<()> {
    let response = client
        .list_machines(Request::new(ListMachinesRequest {}))
        .await
        .context("Shoud list machines")?
        .into_inner();
    dbg!(response);

    Ok(())
}

async fn list_instances(client: &mut DataCenterClient<Channel>) -> Result<()> {
    let response = client
        .list_instances(Request::new(ListInstancesRequest {}))
        .await
        .context("Shoud list instances")?
        .into_inner();
    dbg!(response);

    Ok(())
}

async fn handle_storage_command(
    arguments: StorageArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    match arguments.storage {
        StorageCommands::UploadFile(arguments) => upload_file(arguments, client).await,
        StorageCommands::DownloadFile(arguments) => download_file(arguments, client).await,
    }
}

async fn upload_file(
    arguments: UploadFileArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    let mut source_file = File::open(&arguments.local_file_path).context("Should open file")?;
    let file_size = source_file
        .seek(std::io::SeekFrom::End(0))
        .context("Should seek to end")?;
    source_file
        .seek(std::io::SeekFrom::Start(0))
        .context("Should seek to start")?;
    let file_metadata = client
        .create_file_metadata(Request::new(CreateFileMetadataRequest {
            file_path: arguments.storage_file_path,
            file_size,
        }))
        .await?
        .into_inner()
        .metadata
        .expect("Should contain metadata");

    client
        .upload_file(Request::new(stream_chunks(&file_metadata, source_file)))
        .await?;

    Ok(())
}

async fn download_file(
    arguments: DownloadFileArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    let file_metadata = client
        .get_file_metadata(Request::new(GetFileMetadataRequest {
            file_path: arguments.storage_path,
        }))
        .await?
        .into_inner()
        .metadata
        .expect("Metadata should exist");

    write_file(&arguments.local_path, file_metadata, client).await
}

async fn write_file(
    local_path: &str,
    file_metadata: FileMetadata,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    let mut stream = client
        .download_file(Request::new(DownloadFileRequest {
            source_path: String::from(&file_metadata.file_path),
        }))
        .await?
        .into_inner();
    let mut contents = vec![0; file_metadata.file_size as usize];

    while let Some(message) = stream.message().await? {
        let chunk = message
            .chunk
            .expect("All download requests must have a chunk");
        contents.splice(
            Range {
                start: chunk.start as usize,
                end: chunk.end as usize,
            },
            chunk.data,
        );
    }

    write(local_path, &contents).expect("Should write image");

    Ok(())
}

async fn handle_image_command(
    arguments: OperatingSystemArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    match arguments.os {
        OperatingSystemCommands::UploadImage(arguments) => upload_image(arguments, client).await,
        OperatingSystemCommands::DownloadImage(arguments) => {
            download_image(arguments, client).await
        }
        OperatingSystemCommands::ListImageMetadata => list_image_metadata(client).await,
        OperatingSystemCommands::GetImageMetadata(arguments) => {
            get_image_metadata(arguments, client).await
        }
    }
}

async fn get_image_metadata(
    arguments: GetImageMetadataArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    let image = client
        .get_image_metadata(Request::new(GetImageMetadataRequest {
            image_id: arguments.image_id,
        }))
        .await
        .context("Failed to get image")?
        .into_inner()
        .image
        .context("No metadata for image")?;

    println!("{:?}", image);

    Ok(())
}

async fn upload_image(
    arguments: UploadImageArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    let mut source_file = File::open(&arguments.source_image_path).context("Should open file")?;
    let file_size = source_file
        .seek(std::io::SeekFrom::End(0))
        .context("Should seek to end")?;
    source_file
        .seek(std::io::SeekFrom::Start(0))
        .context("Should seek to start")?;
    let image = if let Some(image_id) = arguments.image_id {
        client
            .get_image_metadata(Request::new(GetImageMetadataRequest { image_id }))
            .await
            .context("Failed to get image")?
            .into_inner()
            .image
            .context("No metadata for image")?
    } else {
        client
            .create_image_metadata(Request::new(CreateImageMetadataRequest {
                file_size,
                destination_file_path: arguments.destination_image_path,
            }))
            .await?
            .into_inner()
            .os_image_metadata
            .context("Should create image")?
    };
    let file_metadata = &image.file_metadata.expect("Should have file metadata");
    client
        .upload_file(Request::new(stream_chunks(&file_metadata, source_file)))
        .await?;

    Ok(())
}

struct ChunkedReader<T>
where
    T: Read,
{
    source: T,
    write_path: String,
    position: usize,
}

impl<T> ChunkedReader<T>
where
    T: Read,
{
    fn new(source: T, write_path: String) -> ChunkedReader<T> {
        ChunkedReader {
            source,
            write_path,
            position: 0,
        }
    }
}

impl<T> Iterator for ChunkedReader<T>
where
    T: Read,
{
    type Item = UploadFileRequest;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buffer = [0; ONE_MB];

        match self.source.read(&mut buffer) {
            Ok(0) => None,
            Ok(count) => {
                let current_position = self.position;
                self.position += count;

                Some(UploadFileRequest {
                    file_path: self.write_path.clone(),
                    chunk: Some(Chunk {
                        start: current_position as u64,
                        end: (current_position + count) as u64,
                        data: buffer[..count].to_vec(),
                    }),
                })
            }
            Err(_) => None,
        }
    }
}

fn stream_chunks(
    file_metadata: &FileMetadata,
    source: File,
) -> impl Stream<Item = UploadFileRequest> {
    let file_path = String::from(&file_metadata.file_path);
    let reader = ChunkedReader::new(source, file_path);

    tokio_stream::iter(reader)
}

async fn download_image(
    arguments: DownloadImageArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    let image = client
        .get_image_metadata(Request::new(GetImageMetadataRequest {
            image_id: arguments.image_id,
        }))
        .await?
        .into_inner()
        .image
        .expect("Should have image metadata");
    let file_metadata = image.file_metadata.expect("Should have file metadata");

    write_file(&arguments.destination_path, file_metadata, client).await
}

async fn list_image_metadata(client: &mut DataCenterClient<Channel>) -> Result<()> {
    dbg!(client
        .list_image_metadata(Request::new(ListImageMetadataRequest {}))
        .await?
        .into_inner());

    Ok(())
}
