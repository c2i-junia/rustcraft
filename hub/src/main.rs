use std::{net::SocketAddr, sync::Arc, time::Duration};

use clap::Parser;
use rustcraft_hub::{
    admin,
    config::{self, Cli, Command, ServeArgs},
    connect, maintenance,
    realms::{
        agent::SocketAgent,
        monitor::{self, Timings},
        probe::UdpProbe,
        Hosting,
    },
    router, AppState,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    match Cli::parse().command {
        Command::Serve(args) => serve(args).await,
        Command::Admin { database, command } => {
            let db = connect(&database.database_url()?).await?;
            println!("{}", admin::run(&db, command).await?);
            Ok(())
        }
        Command::GenerateKey { path } => {
            config::generate_key(&path)?;
            println!("Wrote a new signing key to {}", path.display());
            Ok(())
        }
    }
}

async fn serve(args: ServeArgs) -> anyhow::Result<()> {
    let signing_key = config::read_signing_key(&args.signing_key_file)?;
    let ip_secret = config::read_ip_secret(&args.ip_secret_file)?;
    let db = connect(&args.database.database_url()?).await?;
    let realms = Hosting::new(
        Arc::new(SocketAgent::new(args.agent_socket)),
        Arc::new(UdpProbe {
            host: args.realm_host.clone(),
            timeout: Duration::from_secs(2),
        }),
        args.realm_host,
        args.realm_base_port,
        args.realm_slots,
        args.max_running,
    )?;
    let state = AppState {
        db,
        name: args.name.as_str().into(),
        public_url: args.public_url.trim_end_matches('/').into(),
        signing_key: Arc::new(signing_key),
        ip_secret: ip_secret.into(),
        trusted_ranges: args.trusted_ranges.into(),
        trusted_proxies: args.trusted_proxies.into(),
        hasher: Default::default(),
        logins: Default::default(),
        signups: Default::default(),
        certificates: Default::default(),
        realms: Arc::new(realms),
    };
    maintenance::spawn(state.clone());
    monitor::spawn(state.clone(), Timings::default());
    let listener = tokio::net::TcpListener::bind(args.listen).await?;
    tracing::info!(listen = %args.listen, "the hub is up");
    let app = router(state).into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, app).await?;
    Ok(())
}
