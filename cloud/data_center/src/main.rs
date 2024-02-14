use std::{collections::HashMap, sync::Mutex};

use data_center::protos::data_center::{
    data_center_server::{DataCenter, DataCenterServer},
    CheckResourceRequest, CheckResourceResponse, ProvisionMachineRequest, ProvisionMachineResponse,
    Resources,
};
use tokio::process::{Child, Command};
use tonic::{transport::Server, Request, Response, Status};

#[derive(Default)]
struct LocalDataCenter {
    machines_by_ip: Mutex<HashMap<String, Child>>,
}

#[tonic::async_trait]
impl DataCenter for LocalDataCenter {
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
