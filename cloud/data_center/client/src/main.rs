use std::{
    fs::{write, File},
    io::{Read, Seek},
    ops::Range,
};

use anyhow::{Context, Result};
use data_center_client::{
    cli::{
        parse_cli, Commands, ComputeArguments, ComputeCommands, DownloadFileArguments,
        DownloadImageArguments, GetImageMetadataArguments, OperatingSystemArguments,
        OperatingSystemCommands, ProvisionMachineArguments, StorageArguments, StorageCommands,
        UploadFileArguments, UploadImageArguments,
    },
    protos::data_center::{
        data_center_client::DataCenterClient, Chunk, CreateFileMetadataRequest,
        CreateImageMetadataRequest, DownloadFileRequest, FileMetadata, GetFileMetadataRequest,
        GetImageMetadataRequest, ListImageMetadataRequest, ProvisionMachineRequest, Resources,
        UploadFileRequest,
    },
};
use tokio_stream::Stream;
use tonic::{transport::Channel, Request};

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_cli();
    let mut client = DataCenterClient::connect(format!("http://{}", args.host_name)).await?;

    match args.command {
        Commands::Compute(arguments) => handle_compute_command(arguments, &mut client).await,
        Commands::Storage(arguments) => handle_storage_command(arguments, &mut client).await,
        Commands::Os(arguments) => handle_image_command(arguments, &mut client).await,
    }
}

async fn handle_compute_command(
    arguments: ComputeArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    match arguments.compute {
        ComputeCommands::ProvisionMachine(arguments) => provision_machine(arguments, client).await,
    }
}

async fn provision_machine(
    arguments: ProvisionMachineArguments,
    client: &mut DataCenterClient<Channel>,
) -> Result<()> {
    client
        .provision_machine(Request::new(ProvisionMachineRequest {
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

fn stream_chunks(
    file_metadata: &FileMetadata,
    source: File,
) -> impl Stream<Item = UploadFileRequest> {
    let file_path = String::from(&file_metadata.file_path);

    async_stream::stream! {
        let mut chunk = [0; 4096];
        let mut reader = std::io::BufReader::new(source);
        let mut start: usize = 0;

        loop {
            let bytes_read = reader
                .read(&mut chunk)
                .context("Should read bytes from file")
                .unwrap();

            if bytes_read == 0 {
                break;
            }

            yield UploadFileRequest {
                file_path: String::from(&file_path),
                chunk: Some(Chunk {
                    start: start as u64,
                    end: (start + bytes_read - 1) as u64,
                    data: chunk.to_vec()
                })
            };

            start += bytes_read;
        }
    }
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
