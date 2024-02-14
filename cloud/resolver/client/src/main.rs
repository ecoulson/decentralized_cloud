use client::{
    cli::{
        parse_cli, Commands, DataCenterArgs, DataCenterCommand, ProvisionCommand,
        RegisterDataCenterCommand,
    },
    protos::dcns_resolver::{
        dcns_resolver_client::DcnsResolverClient, ProvisionRequest, RegisterDataCenterRequest,
    },
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_cli();

    match args.command {
        Commands::DataCenter(args) => handle_data_center_commands(args).await?,
        Commands::Provision(provision_command) => provision(provision_command).await?,
    }

    Ok(())
}

async fn handle_data_center_commands(
    args: DataCenterArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    match args.command {
        DataCenterCommand::Register(command) => register_data_center(command).await?,
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

async fn provision(command: ProvisionCommand) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = DcnsResolverClient::connect("http://[::1]:50051").await?;

    let Some(host_name) = command.host_name else {
        unimplemented!();
    };

    let request = tonic::Request::new(ProvisionRequest {
        ram_mb: command.ram_mb,
        disk_mb: command.disk_mb,
        vcpus: command.vcpus,
        host_name,
    });
    let response = client.provision_machine(request).await?;
    dbg!(response);

    Ok(())
}
