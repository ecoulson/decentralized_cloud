use dcns_resolver::{
    dcns_resolver_client::DcnsResolverClient, RegisterDataCenterRequest,
};

pub mod dcns_resolver {
    tonic::include_proto!("dcns_resolver");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DcnsResolverClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(RegisterDataCenterRequest {});
    let response = client.register_data_center(request).await?;
    dbg!(response);

    Ok(())
}
