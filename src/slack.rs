use crate::worker::ProposalStatus;
use log::{error, info};
use serde_derive::{Deserialize, Serialize};
use slack_hook::{PayloadBuilder, Slack};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SlackConfig {
    pub webhook_url: Option<String>,
    pub channel: String,
    pub assignee: String,
}

impl SlackConfig {
    pub async fn send_alert(
        &self,
        chain_id: String,
        proposal_list: Vec<u64>,
        proposal_status: ProposalStatus,
        is_mainnet: bool,
    ) {
        let list: Vec<String> = proposal_list
            .iter()
            .map(|x| format!("proposal_id={}", x))
            .collect();
        let proposal_id = list.join(",");

        let network_type = if is_mainnet {
            ":warning: MAINNET"
        } else {
            ":test_tube: TESTNET"
        };

        let slack = Slack::new(self.webhook_url.clone().unwrap()).unwrap();
        let p = PayloadBuilder::new()
            .text(format!(
                "[{}] {}\nNew proposal(s) found: {}\nStatus: {:?}\n{}",
                chain_id, network_type, proposal_id, proposal_status, self.assignee
            ))
            .channel(format!("#{}", self.channel))
            .username("New Proposal Alert")
            .link_names(true)
            .icon_emoji(":black_question_mark:")
            .build()
            .unwrap();
        let res = slack.send(&p).await;
        match res {
            Ok(()) => info!("Sent alert"),
            Err(x) => error!("ERR: {:?}", x),
        }
    }
}
