//! Chain configuration
use crate::incidentio::IncidentIOConfig;
use crate::slack::SlackConfig;
use std::collections::HashMap;
use std::{fs, fs::File, io::Write, path::Path, time::Duration};

use serde_derive::{Deserialize, Serialize};

use crate::error::Error;

pub mod default {
    use super::*;

    pub fn refresh() -> Duration {
        Duration::from_secs(30)
    }
    pub fn filter_status() -> Vec<i32> {
        vec![1, 2, 3, 4, 5]
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slack: Option<SlackConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incident_io: Option<IncidentIOConfig>,
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub chains: Vec<ChainConfig>,
}

impl Config {
    pub fn chains_map(&self) -> HashMap<&String, &ChainConfig> {
        self.chains.iter().map(|c| (&c.id, c)).collect()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChainConfig {
    pub id: String,
    pub grpc_addr: tendermint_rpc::Url,
    #[serde(default = "default::refresh", with = "humantime_serde")]
    pub refresh: Duration,
    #[serde(default = "default::filter_status")]
    pub filter_status: Vec<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_type: Option<Vec<String>>,
    #[serde(default)]
    pub mainnet: bool,
}

/// Attempt to load and parse the TOML config file as a `Config`.
pub fn load(path: impl AsRef<Path>) -> Result<Config, Error> {
    let config_toml = fs::read_to_string(&path).map_err(Error::config_io)?;

    let config = toml::from_str::<Config>(&config_toml[..]).map_err(Error::config_decode)?;
    Ok(config)
}

/// Serialize the given `Config` as TOML to the given config file.
pub fn store(config: &Config, path: impl AsRef<Path>) -> Result<(), Error> {
    let mut file = if path.as_ref().exists() {
        fs::OpenOptions::new().write(true).truncate(true).open(path)
    } else {
        File::create(path)
    }
    .map_err(Error::config_io)?;

    store_writer(config, &mut file)
}

/// Serialize the given `Config` as TOML to the given writer.
pub(crate) fn store_writer(config: &Config, mut writer: impl Write) -> Result<(), Error> {
    let toml_config = toml::to_string_pretty(&config).map_err(Error::config_encode)?;

    writeln!(writer, "{}", toml_config).map_err(Error::config_io)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{load, store_writer};
    use test_log::test;

    #[test]
    fn parse_valid_config() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/config/fixtures/chains.toml"
        );

        let config = load(path);
        println!("{:?}", config);
        assert!(config.is_ok());
    }

    #[test]
    fn parse_invalid_config() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/config/fixtures/chains-fail.toml"
        );

        let config = load(path);
        println!("{:?}", config);
        assert!(config.is_err());
    }

    #[test]
    fn serialize_valid_config() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/config/fixtures/chains.toml"
        );

        let config = load(path).expect("could not parse config");

        let mut buffer = Vec::new();
        store_writer(&config, &mut buffer).unwrap();
    }
}
