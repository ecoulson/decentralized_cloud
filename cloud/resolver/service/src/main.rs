use resolver_service::protos::resolver::{
    dcns_resolver_server::{DcnsResolver, DcnsResolverServer},
    DataCenter, ListDataCentersRequest, ListDataCentersResponse, RegisterDataCenterRequest,
    RegisterDataCenterResponse,
};
use std::sync::Mutex;
use tonic::{transport::Server, Request, Response, Status};

#[derive(Default)]
struct LocalDcnsResolver {
    data_centers: Mutex<Vec<DataCenter>>,
}

#[tonic::async_trait]
impl DcnsResolver for LocalDcnsResolver {
    async fn list_data_centers(
        &self,
        _request: Request<ListDataCentersRequest>,
    ) -> Result<Response<ListDataCentersResponse>, Status> {
        Ok(Response::new(ListDataCentersResponse {
            data_center: self
                .data_centers
                .lock()
                .expect("Should fetch lock")
                .to_vec(),
        }))
    }

    async fn register_data_center(
        &self,
        request: Request<RegisterDataCenterRequest>,
    ) -> Result<Response<RegisterDataCenterResponse>, Status> {
        let request = request.into_inner();
        self.data_centers
            .lock()
            .expect("Should fetch lock")
            .push(DataCenter {
                host_name: String::from(&request.host_name),
            });

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
