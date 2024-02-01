use dcns_resolver::{
    dcns_resolver_server::{DcnsResolver, DcnsResolverServer},
    ProvisionRequest, ProvisionResponse,
};
use tonic::{transport::Server, Request, Response, Status};

pub mod dcns_resolver {
    tonic::include_proto!("dcns_resolver");
}

#[derive(Default)]
struct LocalDcnsResolver {}

#[tonic::async_trait]
impl DcnsResolver for LocalDcnsResolver {
    async fn provision_machine(
        &self,
        request: Request<ProvisionRequest>,
    ) -> Result<Response<ProvisionResponse>, Status> {
        Ok(Response::new(ProvisionResponse {
            machine_ip: String::from("0.0.0.0"),
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
