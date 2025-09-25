use crate::create_grpc_client;
use crate::incidentio::IncidentIOConfig;
use crate::slack::SlackConfig;
use anyhow::{Context, Result};
use chrono::prelude::*;
use cosmos_sdk_proto::cosmos::base::query::v1beta1::PageRequest;
use cosmos_sdk_proto::cosmos::gov::v1::{
    query_client::QueryClient, Proposal, QueryProposalsRequest,
};
use enum_iterator::{all, Sequence};
use http::uri::Uri;
use itertools::Itertools;
use log::{error, info};
use std::fs::{File, OpenOptions};
use std::io;
use std::io::prelude::*;
use std::path::Path;
use std::time::Duration;
use tendermint_rpc::Url;

#[derive(Sequence, Debug, Clone)]
pub enum ProposalStatus {
    Deposit = 1, // 1
    Voting,
    Passed,
    Rejected,
    Failed,
}

/// Configuration for tracking proposal status
pub struct TrackingConfig {
    pub grpc_addr: Url,
    pub chain_id: String,
    pub refresh: Duration,
    pub filter_status: Vec<i32>,
    pub filter_type: Option<Vec<String>>,
    pub slack_config: Option<SlackConfig>,
    pub incidentio_config: Option<IncidentIOConfig>,
    pub is_mainnet: bool,
}

/// Fetches on-chain proposal of given proposal_status and chain
pub async fn get_proposals(proposal_status: i32, grpc_addr: &Uri) -> Result<Vec<Proposal>> {
    let mut client = create_grpc_client(grpc_addr.clone(), QueryClient::new)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create gRPC client for {}: {}", grpc_addr, e))?;
    let request = QueryProposalsRequest {
        proposal_status,
        pagination: Some(PageRequest {
            limit: 1000,
            ..Default::default()
        }),
        ..Default::default()
    };

    client
        .proposals(request)
        .await
        .map(|response| response.into_inner().proposals)
        .map_err(|e| anyhow::anyhow!("Failed to fetch proposals from {}: {}", grpc_addr, e))
}

pub async fn track_proposal_status(config: TrackingConfig) -> Result<()> {
    let grpc_uri: Uri = config.grpc_addr.to_string().parse().context(format!(
        "Failed to parse gRPC address: {}",
        config.grpc_addr
    ))?;
    let chain_id_path = config.chain_id.to_owned();
    let deposit_path = chain_id_path.clone() + "_deposit_id";
    let voting_path = chain_id_path.clone() + "_voting_id";
    let passed_path = chain_id_path.clone() + "_passed_id";
    let rejected_path = chain_id_path.clone() + "_rejected_id";
    let failed_path = chain_id_path.clone() + "_failed_id";
    let mut collect_interval = tokio::time::interval(config.refresh.to_owned());
    'out: loop {
        collect_interval.tick().await;
        for proposal_status in all::<ProposalStatus>().collect::<Vec<_>>() {
            if !config
                .filter_status
                .contains(&(proposal_status.clone() as i32))
            {
                info!("[{}] [{:?}] filtered out", config.chain_id, proposal_status);
                continue;
            }
            let proposal_status_path = match proposal_status {
                ProposalStatus::Deposit => Path::new(&deposit_path),
                ProposalStatus::Voting => Path::new(&voting_path),
                ProposalStatus::Passed => Path::new(&passed_path),
                ProposalStatus::Rejected => Path::new(&rejected_path),
                ProposalStatus::Failed => Path::new(&failed_path),
            };
            let proposals = match get_proposals(proposal_status.clone() as i32, &grpc_uri).await {
                Ok(proposal) => proposal,
                Err(error) => {
                    error!(
                        "[{}] Failed to get {:?} proposals from {}: {} - will retry next refresh",
                        config.chain_id, proposal_status, grpc_uri, error
                    );
                    continue 'out;
                }
            };
            let mut last_proposals_ids = read(proposal_status_path).unwrap();
            let now = Utc::now().timestamp();

            let new_proposals_id_list: Vec<u64> = proposals
                .clone()
                .iter()
                // Filter for new proposals that meet our criteria:
                // 1. Recently changed status (within the time window)
                // 2. Not already tracked in our local state file
                .filter(|&x| {
                    // Extract the relevant timestamp based on the proposal status:
                    // - Deposit: when the proposal was submitted
                    // - Voting: when voting started
                    // - Passed/Rejected/Failed: when voting ended
                    let t = match proposal_status {
                        ProposalStatus::Deposit => x.submit_time.as_ref().unwrap().seconds,
                        ProposalStatus::Voting => x.voting_start_time.as_ref().unwrap().seconds,
                        ProposalStatus::Passed => x.voting_end_time.as_ref().unwrap().seconds,
                        ProposalStatus::Rejected => x.voting_end_time.as_ref().unwrap().seconds,
                        ProposalStatus::Failed => x.voting_end_time.as_ref().unwrap().seconds,
                    };
                    // Check if the proposal status changed recently:
                    // - Must be within 2x the refresh interval (buffer for timing issues)
                    // - AND not already in our tracked proposals list (avoid duplicate alerts)
                    t > (now - config.refresh.as_secs() as i64 * 2)
                        && !last_proposals_ids.contains(&x.id)
                })
                .filter(|&x| {
                    if let Some(ref filter_type) = config.filter_type {
                        x.messages
                            .iter()
                            .any(|msg| filter_type.contains(&msg.type_url))
                    } else {
                        true
                    }
                })
                .collect::<Vec<_>>()
                .iter()
                .map(|&x| x.id)
                .collect();
            info!(
                "[{}] [{:?}] last_proposals_id={:?}",
                config.chain_id, proposal_status, last_proposals_ids
            );

            if !new_proposals_id_list.is_empty() {
                info!(
                    "[{}][{:?}] New proposal(s) are found!",
                    config.chain_id, proposal_status
                );
                info!(
                    "[{}][{:?}] NEW_PROPOSAL_ID_LIST={:?}",
                    config.chain_id, proposal_status, new_proposals_id_list
                );
                if let Some(ref slack_config) = config.slack_config {
                    slack_config
                        .send_alert(
                            config.chain_id.clone(),
                            new_proposals_id_list.clone(),
                            proposal_status.clone(),
                            config.is_mainnet,
                        )
                        .await
                }
                if let Some(ref incidentio_config) = config.incidentio_config {
                    incidentio_config
                        .send_alert(
                            config.chain_id.clone(),
                            new_proposals_id_list.clone(),
                            proposal_status,
                            config.is_mainnet,
                        )
                        .await
                }
            }
            last_proposals_ids.extend(&new_proposals_id_list);
            write(
                last_proposals_ids.iter().join(",").as_str(),
                proposal_status_path,
            )
            .unwrap();
        }
    }
}

fn read(path: &Path) -> io::Result<Vec<u64>> {
    let mut f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    let mut s = String::new();
    match f.read_to_string(&mut s) {
        Ok(_) => {
            let id_list: Vec<u64> = s
                .split(",")
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| s.parse().unwrap())
                .collect();
            Ok(id_list)
        }
        Err(err) => {
            error!("Err={:?}", err);
            Ok(vec![])
        }
    }
}

// A simple implementation of `% echo s > path`
fn write(s: &str, path: &Path) -> io::Result<()> {
    let mut f = File::create(path)?;

    f.write_all(s.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_crypto_provider;

    #[tokio::test]
    async fn test_get_proposals() {
        init_crypto_provider();

        let grpc_addr: Uri = "https://grpc.mantrachain.io".parse().unwrap();
        for proposal_status in all::<ProposalStatus>().collect::<Vec<_>>() {
            match proposal_status {
                ProposalStatus::Deposit => println!("Deposit"),
                ProposalStatus::Voting => println!("Voting"),
                ProposalStatus::Passed => println!("Passed"),
                ProposalStatus::Rejected => println!("Rejected"),
                ProposalStatus::Failed => println!("Failed"),
            };
            let proposals = get_proposals(proposal_status.clone() as i32, &grpc_addr)
                .await
                .unwrap();
            for p in proposals.into_iter() {
                let proposal_message_type: Vec<String> =
                    p.messages.iter().map(|msg| msg.type_url.clone()).collect();
                println!(
                    "[{:?}] proposal id: {:?} proposal type: {}",
                    proposal_status,
                    p.id,
                    proposal_message_type.join(", ")
                );
            }
        }
    }
    #[tokio::test]
    async fn test_get_proposals_filter_type() {
        init_crypto_provider();

        let grpc_addr: Uri = "https://grpc.mantrachain.io".parse().unwrap();
        let filter_type = ["/cosmos.upgrade.v1beta1.MsgSoftwareUpgrade".to_string()];
        for proposal_status in all::<ProposalStatus>().collect::<Vec<_>>() {
            match proposal_status {
                ProposalStatus::Deposit => println!("Deposit"),
                ProposalStatus::Voting => println!("Voting"),
                ProposalStatus::Passed => println!("Passed"),
                ProposalStatus::Rejected => println!("Rejected"),
                ProposalStatus::Failed => println!("Failed"),
            };
            let proposals = get_proposals(proposal_status.clone() as i32, &grpc_addr)
                .await
                .unwrap();

            let proposals_filtered_type = proposals
                .iter()
                .filter(|&x| {
                    x.messages
                        .iter()
                        .any(|msg| filter_type.contains(&msg.type_url))
                })
                .collect::<Vec<_>>();
            for p in proposals_filtered_type.into_iter() {
                let proposal_message_type: Vec<String> =
                    p.messages.iter().map(|msg| msg.type_url.clone()).collect();
                println!(
                    "[{:?}] proposal id: {:?} proposal type: {}",
                    proposal_status,
                    p.id,
                    proposal_message_type.join(", ")
                );
            }
        }
    }
}
