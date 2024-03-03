use std::{
    fs::File,
    io::{Read, Seek},
    ops::Range,
};

use anyhow::{Context, Result};
use data_center_client::{
    cli::{
        parse_cli, Commands, CreateImageMetadataArguments, DownloadImageArguments,
        GetImageMetadataArguments, ProvisionMachineArguments, UploadImageArguments,
    },
    protos::data_center::{
        data_center_client::DataCenterClient, Chunk, CreateImageMetadataRequest,
        DownloadFileRequest, FileMetadata, GetImageMetadataRequest, ListImageMetadataRequest,
        ProvisionMachineRequest, Resources, UploadFileRequest,
    },
};
use tokio_stream::Stream;
use tonic::{transport::Channel, Request};

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_cli();
    let mut client = DataCenterClient::connect(format!("http://{}", args.host_name)).await?;

    match args.command {
        Commands::UploadImage(arguments) => upload_image(&mut client, arguments).await,
        Commands::ProvisionMachine(arguments) => provision_machine(&mut client, arguments).await,
        Commands::GetImageMetadata(arguments) => get_image(&mut client, arguments).await,
        Commands::CreateImageMetadata(arguments) => {
            create_image_metadata(&mut client, arguments).await
        }
        Commands::DownloadImage(arguments) => download_image(&mut client, arguments).await,
        Commands::ListImageMetadata => list_image_metadata(&mut client).await,
    }
}

async fn get_image(
    client: &mut DataCenterClient<Channel>,
    arguments: GetImageMetadataArguments,
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

async fn create_image_metadata(
    client: &mut DataCenterClient<Channel>,
    arguments: CreateImageMetadataArguments,
) -> Result<()> {
    let mut file = File::open(&arguments.local_file_path).context("Should open file")?;
    let file_size = file
        .seek(std::io::SeekFrom::End(0))
        .context("Should seek to end")?;
    file.seek(std::io::SeekFrom::Start(0))
        .context("Should seek to start")?;
    let image = client
        .create_image_metadata(Request::new(CreateImageMetadataRequest {
            file_size,
            destination_file_path: arguments.storage_path,
        }))
        .await?
        .into_inner()
        .os_image_metadata
        .context("Should create image")?;

    println!("{:?}", image);

    Ok(())
}

async fn upload_image(
    client: &mut DataCenterClient<Channel>,
    arguments: UploadImageArguments,
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

async fn provision_machine(
    client: &mut DataCenterClient<Channel>,
    arguments: ProvisionMachineArguments,
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

async fn download_image(
    client: &mut DataCenterClient<Channel>,
    arguments: DownloadImageArguments,
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
        dbg!(&chunk.start, &chunk.end);
        contents.splice(
            Range {
                start: chunk.start as usize,
                end: chunk.end as usize,
            },
            chunk.data,
        );
    }

    std::fs::write(&arguments.destination_path, &contents).expect("Should write image");

    Ok(())
}

async fn list_image_metadata(client: &mut DataCenterClient<Channel>) -> Result<()> {
    dbg!(client
        .list_image_metadata(Request::new(ListImageMetadataRequest {}))
        .await?
        .into_inner());

    Ok(())
}
