use std::{
    cmp::min,
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, Read},
    ops::Range,
    sync::Mutex,
};

use data_center_service::protos::data_center::{
    data_center_server::{DataCenter, DataCenterServer},
    CheckResourceRequest, CheckResourceResponse, Chunk, CreateFileMetadataRequest,
    CreateFileMetadataResponse, CreateImageMetadataRequest, CreateImageMetadataResponse,
    DownloadFileRequest, DownloadFileResponse, FileMetadata, GetFileMetadataRequest,
    GetFileMetadataResponse, GetImageMetadataRequest, GetImageMetadataResponse,
    ListImageMetadataRequest, ListImageMetadataResponse, OsImageMetadata, ProvisionMachineRequest,
    ProvisionMachineResponse, Resources, UploadFileRequest, UploadFileResponse,
};
use nanoid::nanoid;
use tokio::process::{Child, Command};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};

#[derive(Default)]
struct LocalDataCenter {
    machines_by_ip: Mutex<HashMap<String, Child>>,
    images_by_id: Mutex<HashMap<String, OsImageMetadata>>,
    files_by_path: Mutex<HashMap<String, FileMetadata>>,
}

/// Data center is graph of services (want either distributed or local)
#[tonic::async_trait]
impl DataCenter for LocalDataCenter {
    type DownloadFileStream = ReceiverStream<Result<DownloadFileResponse, Status>>;

    async fn get_image_metadata(
        &self,
        request: Request<GetImageMetadataRequest>,
    ) -> Result<Response<GetImageMetadataResponse>, Status> {
        let request = request.into_inner();

        Ok(Response::new(GetImageMetadataResponse {
            image: self
                .images_by_id
                .lock()
                .expect("Should acquire lock")
                .get(&request.image_id)
                .map(|x| x.clone()),
        }))
    }

    async fn create_image_metadata(
        &self,
        request: Request<CreateImageMetadataRequest>,
    ) -> Result<Response<CreateImageMetadataResponse>, Status> {
        let image_id = nanoid!();
        let request = request.into_inner();
        let file_metadata = self
            .create_file_metadata(Request::new(CreateFileMetadataRequest {
                file_path: request.destination_file_path,
                file_size: request.file_size,
            }))
            .await?
            .into_inner()
            .metadata
            .expect("Should have file metadata");
        let image = OsImageMetadata {
            image_id: image_id.clone(),
            file_metadata: Some(file_metadata),
        };
        self.images_by_id
            .lock()
            .expect("Should acquire lock")
            .insert(image_id, image.clone());

        Ok(Response::new(CreateImageMetadataResponse {
            os_image_metadata: Some(image),
        }))
    }

    async fn check_resource(
        &self,
        _request: Request<CheckResourceRequest>,
    ) -> Result<Response<CheckResourceResponse>, Status> {
        Ok(Response::new(CheckResourceResponse {
            available_resources: Some(Resources {
                ram_mb: 512,
                disk_mb: 512,
                vcpus: 1,
            }),
        }))
    }

    async fn provision_machine(
        &self,
        _request: Request<ProvisionMachineRequest>,
    ) -> Result<Response<ProvisionMachineResponse>, Status> {
        let process = Command::new("qemu-system-x86_64")
            .arg("-accel")
            .arg("hvf")
            .arg("-cpu")
            .arg("host,-rdtscp")
            .arg("-smp")
            .arg("2")
            .arg("-m")
            .arg("4G")
            .arg("-device")
            .arg("usb-tablet")
            .arg("-nographic")
            .arg("-usb")
            .arg("-device")
            .arg("virtio-net,netdev=vmnic")
            .arg("-netdev")
            .arg("user,id=vmnic,hostfwd=tcp::9001-:22")
            .arg("-drive")
            .arg("file=/Users/evancoulson/isos/ubuntu2004.qcow2,if=virtio")
            .spawn()
            .expect("Should start child process");
        self.machines_by_ip
            .lock()
            .expect("Should acquire lock")
            .insert(String::from("127.0.0.1"), process);

        Ok(Response::new(ProvisionMachineResponse {
            machine_ip: String::from("127.0.0.1"),
        }))
    }

    async fn create_file_metadata(
        &self,
        request: Request<CreateFileMetadataRequest>,
    ) -> Result<Response<CreateFileMetadataResponse>, Status> {
        let request = request.into_inner();
        let file_metadata = FileMetadata {
            file_path: request.file_path,
            file_size: request.file_size,
        };

        self.files_by_path
            .lock()
            .expect("Should lock file")
            .insert(file_metadata.file_path.clone(), file_metadata.clone());

        Ok(Response::new(CreateFileMetadataResponse {
            metadata: Some(file_metadata),
        }))
    }

    async fn download_file(
        &self,
        request: Request<DownloadFileRequest>,
    ) -> Result<Response<Self::DownloadFileStream>, Status> {
        let request = request.into_inner();
        let (sender, receiver) = tokio::sync::mpsc::channel(16);
        let file_metadata = self
            .get_file_metadata(Request::new(GetFileMetadataRequest {
                file_path: request.source_path,
            }))
            .await
            .expect("Should get file metadata")
            .into_inner()
            .metadata
            .expect("Should have file metadata");
        let file = File::open(&file_metadata.file_path)?;
        dbg!(&file_metadata);

        tokio::spawn(async move {
            let mut chunk = [0; 4096];
            let mut reader = BufReader::new(file);
            let mut start: usize = 0;

            while start <= file_metadata.file_size as usize {
                let bytes_read = reader.read(&mut chunk).expect("Should read chunk");

                sender
                    .send(Ok(DownloadFileResponse {
                        chunk: Some(Chunk {
                            start: start as u64,
                            end: min((start + bytes_read - 1) as u64, file_metadata.file_size),
                            data: chunk.to_vec(),
                        }),
                    }))
                    .await
                    .expect("Should send chunk");

                start += bytes_read;
            }
        });

        Ok(Response::new(ReceiverStream::new(receiver)))
    }

    async fn upload_file(
        &self,
        request: Request<Streaming<UploadFileRequest>>,
    ) -> Result<Response<UploadFileResponse>, Status> {
        let mut stream = request.into_inner();
        let mut contents: Vec<u8> = Vec::new();
        let mut file_metadata = FileMetadata::default();

        while let Some(message) = stream.message().await? {
            if contents.is_empty() {
                file_metadata = self
                    .get_file_metadata(Request::new(GetFileMetadataRequest {
                        file_path: message.file_path,
                    }))
                    .await?
                    .into_inner()
                    .metadata
                    .expect("No metadata found");
                contents.resize(file_metadata.file_size as usize, 0);
            }

            let chunk = message
                .chunk
                .expect("All upload requests must have a chunk");
            contents.splice(
                Range {
                    start: chunk.start as usize,
                    end: chunk.end as usize,
                },
                chunk.data,
            );
        }

        fs::write(file_metadata.file_path, &contents).expect("Should write image");

        Ok(Response::new(UploadFileResponse {
            bytes_written: contents.len() as u64,
        }))
    }

    async fn get_file_metadata(
        &self,
        request: Request<GetFileMetadataRequest>,
    ) -> Result<Response<GetFileMetadataResponse>, Status> {
        let request = request.into_inner();

        Ok(Response::new(GetFileMetadataResponse {
            metadata: self
                .files_by_path
                .lock()
                .expect("Should acquire lock")
                .get(&request.file_path)
                .map(|x| x.clone()),
        }))
    }

    async fn list_image_metadata(
        &self,
        _request: Request<ListImageMetadataRequest>,
    ) -> Result<Response<ListImageMetadataResponse>, Status> {
        let metadata = self
            .images_by_id
            .lock()
            .expect("Should acquire lock")
            .values()
            .map(|metadata| metadata.clone())
            .collect();

        Ok(Response::new(ListImageMetadataResponse { metadata }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50052".parse()?;
    let data_center = LocalDataCenter::default();

    Server::builder()
        .add_service(DataCenterServer::new(data_center))
        .serve(addr)
        .await?;

    Ok(())
}
