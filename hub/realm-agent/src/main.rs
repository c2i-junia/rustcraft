use std::{path::PathBuf, sync::Arc};

use anyhow::Context;
use clap::Parser;
use rustcraft_realm_agent::{agent::Agent, config::Config, socket};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "rustcraft-realm-agent",
    about = "Runs the hosted worlds of the Rustcraft hub in Docker containers",
    version
)]
struct Cli {
    /// The configuration file (see realm-agent.example.toml).
    #[arg(long, env = "REALM_AGENT_CONFIG")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let config = Config::read(&Cli::parse().config)?;
    let docker =
        bollard::Docker::connect_with_local_defaults().context("could not connect to Docker")?;
    docker.ping().await.context("Docker does not answer")?;
    let listener = socket::bind(&config.socket)?;
    tracing::info!(socket = %config.socket.display(), "the realm agent is up");
    socket::serve(listener, Arc::new(Agent::new(config, docker))).await;
    Ok(())
}
