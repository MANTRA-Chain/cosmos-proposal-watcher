use crate::worker::ProposalStatus;
use log::{error, info};
use reqwest;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct IncidentIOConfig {
    pub url: String,
    pub token: Option<String>,
}

#[derive(Debug, Serialize)]
struct AlertEvent {
    title: String,
    description: String,
    deduplication_key: String,
    status: String,
    metadata: HashMap<String, String>,
}

struct AlertParams {
    deduplication_key: String,
    status: String,
    title: String,
    description: String,
    proposal_status: Option<ProposalStatus>,
    is_mainnet: bool,
}

impl IncidentIOConfig {
    pub async fn send_alert(
        &self,
        chain_id: String,
        proposal_list: Vec<u64>,
        proposal_status: ProposalStatus,
        is_mainnet: bool,
    ) {
        let client = reqwest::Client::new();
        let token = match &self.token {
            Some(t) => t,
            None => {
                error!("IncidentIO token not configured");
                return;
            }
        };

        for proposal_id in proposal_list {
            // Send resolved alerts for previous states
            match proposal_status {
                ProposalStatus::Voting => {
                    // Resolve previous Deposit alert
                    let params = AlertParams {
                        deduplication_key: format!("{}-{}-Deposit", chain_id, proposal_id),
                        status: "resolved".to_string(),
                        title: format!(
                            "[{}] Proposal {} moved from Deposit to Voting",
                            chain_id, proposal_id
                        ),
                        description: format!(
                            "Proposal {} on chain {} has entered voting period",
                            proposal_id, chain_id
                        ),
                        proposal_status: None,
                        is_mainnet,
                    };
                    self.send_single_alert(&client, &self.url, token, params)
                        .await;
                }
                ProposalStatus::Passed | ProposalStatus::Rejected | ProposalStatus::Failed => {
                    // Resolve previous Voting alert
                    let status_str = format!("{:?}", proposal_status);
                    let params = AlertParams {
                        deduplication_key: format!("{}-{}-Voting", chain_id, proposal_id),
                        status: "resolved".to_string(),
                        title: format!(
                            "[{}] Proposal {} voting ended - {}",
                            chain_id, proposal_id, status_str
                        ),
                        description: format!(
                            "Proposal {} on chain {} voting period ended with status: {}",
                            proposal_id, chain_id, status_str
                        ),
                        proposal_status: None,
                        is_mainnet,
                    };
                    self.send_single_alert(&client, &self.url, token, params)
                        .await;
                }
                _ => {}
            }

            // Send firing alert for current state
            let current_key = format!("{}-{}-{:?}", chain_id, proposal_id, proposal_status);
            let title = format!(
                "[{}] Proposal {} - Status: {:?}",
                chain_id, proposal_id, proposal_status
            );
            let description = match proposal_status {
                ProposalStatus::Deposit => format!(
                    "New proposal {} submitted on chain {} - currently in deposit period",
                    proposal_id, chain_id
                ),
                ProposalStatus::Voting => format!(
                    "Proposal {} on chain {} has entered voting period - action may be required",
                    proposal_id, chain_id
                ),
                ProposalStatus::Passed => format!(
                    "Proposal {} on chain {} has PASSED - review for implementation",
                    proposal_id, chain_id
                ),
                ProposalStatus::Rejected => format!(
                    "Proposal {} on chain {} was REJECTED",
                    proposal_id, chain_id
                ),
                ProposalStatus::Failed => {
                    format!("Proposal {} on chain {} has FAILED", proposal_id, chain_id)
                }
            };

            let params = AlertParams {
                deduplication_key: current_key.clone(),
                status: "firing".to_string(),
                title,
                description,
                proposal_status: Some(proposal_status.clone()),
                is_mainnet,
            };
            self.send_single_alert(&client, &self.url, token, params)
                .await;

            // For Passed status, immediately resolve it
            if matches!(proposal_status, ProposalStatus::Passed) {
                let params = AlertParams {
                    deduplication_key: current_key,
                    status: "resolved".to_string(),
                    title: format!(
                        "[{}] Proposal {} - Passed (Acknowledged)",
                        chain_id, proposal_id
                    ),
                    description: format!(
                        "Proposal {} on chain {} passed status has been acknowledged",
                        proposal_id, chain_id
                    ),
                    proposal_status: None,
                    is_mainnet,
                };
                self.send_single_alert(&client, &self.url, token, params)
                    .await;
            }
        }
    }

    async fn send_single_alert(
        &self,
        client: &reqwest::Client,
        url: &str,
        token: &str,
        params: AlertParams,
    ) {
        let mut metadata = HashMap::new();
        metadata.insert("team".to_string(), "governance".to_string());
        metadata.insert("service".to_string(), "cosmos-proposal-watcher".to_string());
        metadata.insert("mainnet".to_string(), params.is_mainnet.to_string());

        // Add severity based on status and proposal status
        let severity = if let Some(ProposalStatus::Rejected | ProposalStatus::Failed) =
            params.proposal_status
        {
            "urgent"
        } else {
            "warning"
        };
        metadata.insert("severity".to_string(), severity.to_string());

        let alert = AlertEvent {
            title: params.title,
            description: params.description,
            deduplication_key: params.deduplication_key.clone(),
            status: params.status,
            metadata,
        };

        match client
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&alert)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    info!(
                        "IncidentIO alert sent successfully: {} - {}",
                        params.deduplication_key, alert.status
                    );
                } else {
                    error!(
                        "IncidentIO alert failed: {} - Status: {}",
                        params.deduplication_key,
                        response.status()
                    );
                }
            }
            Err(e) => {
                error!("Failed to send IncidentIO alert: {}", e);
            }
        }
    }
}
