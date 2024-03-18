use core::panic;
use std::{
    cmp::min,
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, Read},
    ops::Range,
    process::Stdio,
    sync::Mutex,
};

use data_center_service::protos::data_center::{
    data_center_server::{DataCenter, DataCenterServer},
    CheckResourceRequest, CheckResourceResponse, Chunk, CreateFileMetadataRequest,
    CreateFileMetadataResponse, CreateImageMetadataRequest, CreateImageMetadataResponse,
    CreateMachineRequest, CreateMachineResponse, DownloadFileRequest, DownloadFileResponse,
    FileMetadata, GetFileMetadataRequest, GetFileMetadataResponse, GetImageMetadataRequest,
    GetImageMetadataResponse, Instance, InstanceState, ListImageMetadataRequest,
    ListImageMetadataResponse, ListInstancesRequest, ListInstancesResponse, ListMachinesRequest,
    ListMachinesResponse, Machine, OsImageMetadata, ProvisionInstanceRequest,
    ProvisionInstanceResponse, Resources, StartInstanceRequest, StartInstanceResponse,
    StopInstanceRequest, StopInstanceResponse, UploadFileRequest, UploadFileResponse,
};
use nanoid::nanoid;
use tokio::process::{Child, Command};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{transport::Server, Request, Response, Status, Streaming};

#[derive(Default)]
struct LocalDataCenter {
    machines_by_id: Mutex<HashMap<String, Machine>>,
    instances_by_instance_id: Mutex<HashMap<String, Instance>>,
    processes_by_instance_id: Mutex<HashMap<String, Child>>,
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

    async fn create_machine(
        &self,
        request: Request<CreateMachineRequest>,
    ) -> Result<Response<CreateMachineResponse>, Status> {
        let request = request.into_inner();
        let resources = request.resources.expect("should have resources");
        let image = self
            .images_by_id
            .lock()
            .expect("Should acquire lock")
            .get(&request.image_id)
            .expect("Should find image")
            .clone();
        let machine_id = nanoid!();
        let machine = Machine {
            machine_id: machine_id.clone(),
            resources: Some(resources),
            image_metadata: Some(image),
        };
        self.machines_by_id
            .lock()
            .expect("Should acquire lock")
            .insert(machine_id, machine.clone());

        Ok(Response::new(CreateMachineResponse {
            machine: Some(machine),
        }))
    }

    async fn start_instance(
        &self,
        request: Request<StartInstanceRequest>,
    ) -> Result<Response<StartInstanceResponse>, Status> {
        let request = request.into_inner();
        let mut instance = self
            .instances_by_instance_id
            .lock()
            .expect("Should acquire lock")
            .get(&request.instance_id)
            .expect("Should find instance")
            .clone();
        let machine = instance.machine.clone().expect("Machine should exist");
        let process = self.start_instance_process(&machine);
        instance.set_state(InstanceState::Started);
        instance.process_id = String::from(process.id().expect("Should have pid").to_string());
        self.instances_by_instance_id
            .lock()
            .expect("Should acquire lock")
            .insert(instance.instance_id.clone(), instance.clone());
        self.processes_by_instance_id
            .lock()
            .expect("Should acquire lock")
            .insert(instance.instance_id.clone(), process);

        Ok(Response::new(StartInstanceResponse {
            instance: Some(instance),
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
            version: 0,
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
        let mut contents: Vec<u8> = Vec::with_capacity(0);
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

    async fn provision_instance(
        &self,
        request: Request<ProvisionInstanceRequest>,
    ) -> Result<Response<ProvisionInstanceResponse>, Status> {
        let request = request.into_inner();
        let machine_table = self.machines_by_id.lock().expect("Should acquire lock");
        let machine = machine_table
            .get(&request.machine_id)
            .expect("Should find machine id");
        let process = self.start_instance_process(machine);
        let process_id = process
            .id()
            .expect("Process should have a pid while running");
        let instance = Instance {
            process_id: process_id.to_string(),
            instance_id: nanoid!(),
            ip_address: String::from("192.168.0.1"),
            machine: Some(machine.clone()),
            state: InstanceState::Started as i32,
        };
        self.instances_by_instance_id
            .lock()
            .expect("Should acquire lock")
            .insert(String::from(&instance.instance_id), instance.clone());
        self.processes_by_instance_id
            .lock()
            .expect("Should acquire lock")
            .insert(String::from(&instance.instance_id), process);

        Ok(Response::new(ProvisionInstanceResponse {
            instance: Some(instance),
        }))
    }

    async fn stop_instance(
        &self,
        request: Request<StopInstanceRequest>,
    ) -> Result<Response<StopInstanceResponse>, Status> {
        let request = request.into_inner();
        let instance = self
            .instances_by_instance_id
            .lock()
            .expect("Should acquire lock")
            .get(&request.instance_id)
            .expect("Instance should exist")
            .clone();
        self.processes_by_instance_id
            .lock()
            .expect("Should acquire lock")
            .remove(&instance.instance_id);
        self.instances_by_instance_id
            .lock()
            .expect("Should acquire lock")
            .get_mut(&request.instance_id)
            .expect("Instance should exist")
            .set_state(InstanceState::Stopped);

        Ok(Response::new(StopInstanceResponse {}))
    }

    async fn list_machines(
        &self,
        _request: Request<ListMachinesRequest>,
    ) -> Result<Response<ListMachinesResponse>, Status> {
        Ok(Response::new(ListMachinesResponse {
            machine: self
                .machines_by_id
                .lock()
                .expect("Should acquire lock")
                .values()
                .map(|machine| machine.clone())
                .collect(),
        }))
    }

    async fn list_instances(
        &self,
        _request: Request<ListInstancesRequest>,
    ) -> Result<Response<ListInstancesResponse>, Status> {
        Ok(Response::new(ListInstancesResponse {
            instance: self
                .instances_by_instance_id
                .lock()
                .expect("Should acquire lock")
                .values()
                .map(|instance| instance.clone())
                .collect(),
        }))
    }
}

impl LocalDataCenter {
    fn start_instance_process(&self, machine: &Machine) -> Child {
        let Some(image_metadata) = &machine.image_metadata else {
            panic!("Should have image metadata");
        };
        let Some(file_metadata) = &image_metadata.file_metadata else {
            panic!("Should have file metadata")
        };

        Command::new("qemu-system-x86_64")
            .stdout(Stdio::null())
            .stdin(Stdio::null())
            .arg("-accel")
            .arg("hvf")
            .arg("-cpu")
            .arg("host,-rdtscp")
            .arg("-smp")
            .arg("2")
            .arg("-m")
            .arg(format!(
                "{}G",
                machine
                    .resources
                    .as_ref()
                    .expect("Should have resources")
                    .ram_mb
                    / 1024
            ))
            .arg("-device")
            .arg("usb-tablet")
            .arg("-nographic")
            .arg("-usb")
            .arg("-device")
            .arg("virtio-net,netdev=vmnic")
            .arg("-netdev")
            .arg("user,id=vmnic,hostfwd=tcp::9001-:22")
            .arg("-drive")
            .arg(format!("file={},if=virtio", &file_metadata.file_path))
            .kill_on_drop(true)
            .spawn()
            .expect("Should start child process")
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
