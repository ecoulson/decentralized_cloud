use std::collections::HashMap;

use service::protos::{
    data_center::{
        data_center_client::DataCenterClient, CheckResourceRequest, ProvisionMachineRequest,
        Resources,
    },
    dcns_resolver::{
        dcns_resolver_server::{DcnsResolver, DcnsResolverServer},
        DataCenter, ProvisionRequest, ProvisionResponse, RegisterDataCenterRequest,
        RegisterDataCenterResponse,
    },
};
use tokio::sync::Mutex;
use tonic::{
    transport::{Channel, Server},
    Request, Response, Status,
};

#[derive(Default)]
struct LocalDcnsResolver {
    data_centers_by_host_name: Mutex<HashMap<String, DataCenter>>,
}

impl LocalDcnsResolver {
    async fn check_resources(
        &self,
        client: &mut DataCenterClient<Channel>,
        provision_request: &ProvisionRequest,
    ) -> bool {
        let request = Request::new(CheckResourceRequest {});
        let response = client
            .check_resource(request)
            .await
            .expect("Should check request")
            .into_inner();

        if let Some(resources) = response.available_resources {
            if provision_request.ram_mb <= resources.ram_mb
                && provision_request.disk_mb <= resources.disk_mb
                && provision_request.vcpus <= resources.vcpus
            {
                return true;
            }
        }

        return false;
    }

    async fn provision_in_data_center(
        &self,
        client: &mut DataCenterClient<Channel>,
        request: &ProvisionRequest,
    ) -> ProvisionResponse {
        let response = client
            .provision_machine(ProvisionMachineRequest {
                resources: Some(Resources {
                    ram_mb: request.ram_mb,
                    disk_mb: request.disk_mb,
                    vcpus: request.vcpus,
                }),
            })
            .await
            .expect("Failed to provision machine")
            .into_inner();

        ProvisionResponse {
            machine_ip: String::from(response.machine_ip),
        }
    }
}

#[tonic::async_trait]
impl DcnsResolver for LocalDcnsResolver {
    async fn provision_machine(
        &self,
        request: Request<ProvisionRequest>,
    ) -> Result<Response<ProvisionResponse>, Status> {
        let request = request.into_inner();
        let data_centers_by_host_name = self.data_centers_by_host_name.lock().await;
        let data_center = data_centers_by_host_name
            .get(&request.host_name)
            .expect("Should get data center");
        let mut client = DataCenterClient::connect(format!("http://{}", data_center.host_name))
            .await
            .expect("Should connect to data center");

        if !self.check_resources(&mut client, &request).await {
            todo!();
        };

        return Ok(Response::new(
            self.provision_in_data_center(&mut client, &request).await,
        ));
    }

    async fn register_data_center(
        &self,
        request: Request<RegisterDataCenterRequest>,
    ) -> Result<Response<RegisterDataCenterResponse>, Status> {
        let request = request.into_inner();
        self.data_centers_by_host_name.lock().await.insert(
            String::from(&request.host_name),
            DataCenter {
                host_name: String::from(&request.host_name),
            },
        );

        Ok(Response::new(RegisterDataCenterResponse {
            data_center: Some(DataCenter {
                host_name: String::from(&request.host_name),
            }),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let dcns_resolver = LocalDcnsResolver::default();

    Server::builder()
        .add_service(DcnsResolverServer::new(dcns_resolver))
        .serve(addr)
        .await?;

    Ok(())
}
