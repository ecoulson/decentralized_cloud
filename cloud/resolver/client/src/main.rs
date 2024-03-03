use resolver_client::{
    cli::{parse_cli, Commands, RegisterDataCenterCommand},
    protos::resolver::{
        dcns_resolver_client::DcnsResolverClient, ListDataCentersRequest, RegisterDataCenterRequest,
    },
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_cli();

    match args.command {
        Commands::Register(args) => register_data_center(args).await?,
        Commands::List => list_data_centers().await?,
    }

    Ok(())
}

async fn register_data_center(
    command: RegisterDataCenterCommand,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DcnsResolverClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(RegisterDataCenterRequest {
        host_name: String::from(command.host_name),
    });
    let response = client.register_data_center(request).await?;
    dbg!(response);

    Ok(())
}

async fn list_data_centers() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DcnsResolverClient::connect("http://[::1]:50051").await?;
    let request = tonic::Request::new(ListDataCentersRequest {});
    let response = client.list_data_centers(request).await?;
    dbg!(response);

    Ok(())
}
