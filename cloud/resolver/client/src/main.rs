use dcns_resolver::{dcns_resolver_client::DcnsResolverClient, ProvisionRequest};

pub mod dcns_resolver {
    tonic::include_proto!("dcns_resolver");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DcnsResolverClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(ProvisionRequest {
        ram_mb: 512,
        disk_mb: 1024,
    });
    let response = client.provision_machine(request).await?;
    dbg!(response);

    Ok(())
}
