use cosmos_proposal_watcher::{config, init_crypto_provider, worker, DEFAULT_CONFIG_PATH};
use env_logger::Builder;
use log::{error, info, LevelFilter};
use std::env;
use std::path::PathBuf;
use std::result::Result;
use std::time::Duration;
use structopt::StructOpt;

/// Helper sub-commands
#[derive(Debug, StructOpt)]
#[structopt(
    name = "proposal-watcher",
    about = "watcher for cosmos-sdk chain proposal"
)]
enum ProposalWatcher {
    #[structopt(name = "start", about = "start proposal watcher process")]
    Start {
        #[structopt(short)]
        config_path: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() {
    let mut builder = Builder::new();
    builder.filter_level(LevelFilter::Info).init();

    let opt = ProposalWatcher::from_args();
    let result = match opt {
        ProposalWatcher::Start { config_path } => start(config_path).await,
    };
    if let Err(e) = result {
        error!("{}", e);
        std::process::exit(1);
    }
}

async fn start(config_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let default_path = format!(
        "{}/{}",
        std::env::current_exe()?.parent().unwrap().to_str().unwrap(),
        DEFAULT_CONFIG_PATH
    );
    let cp = config_path.unwrap_or_else(|| default_path.into());
    info!("config file: {}", cp.display());
    if !cp.exists() {
        Err("missing chains.toml file".into())
    } else {
        let mut config = config::load(cp).expect("could not parse config");
        if config.slack.is_some() && config.slack.as_ref().unwrap().webhook_url.is_none() {
            config.slack.as_mut().unwrap().webhook_url = env::var("SLACK_WEBHOOK_URL").ok();
        }
        if config.incident_io.is_some() && config.incident_io.as_ref().unwrap().token.is_none() {
            config.incident_io.as_mut().unwrap().token = env::var("INCIDENT_IO_TOKEN").ok();
        }
        init_crypto_provider();
        info!("Started proposal watcher server");
        tokio::task::spawn(proposal_status_collector(config.clone())).await?;
        loop {
            std::thread::sleep(Duration::new(30, 0));
        }
    }
}

async fn proposal_status_collector(config: config::Config) {
    for chain_config in config.chains.iter() {
        tokio::task::spawn(worker::track_proposal_status(
            chain_config.grpc_addr.clone(),
            chain_config.id.clone(),
            chain_config.refresh,
            chain_config.filter_status.clone(),
            chain_config.filter_type.clone(),
            config.slack.clone(),
            config.incident_io.clone(),
        ));
    }
}
