use std::collections::HashMap;
use std::env::args;

use anyhow::Result;
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
pub struct Config {
    pub database_url: String,
    pub networks: HashMap<String, Network>,
}

#[derive(Deserialize, Clone)]
pub struct Network {
    pub rpc_url: String,
    pub chain_id: i32,
    pub block_time: u8,
    pub hooks: HashMap<String, Hook>,
}

#[derive(Deserialize, Clone)]
pub struct Hook {
    #[serde(deserialize_with = "contracts_to_lowercase")]
    pub contracts: Vec<String>,
    pub url: String,
}

impl Config {
    pub async fn load() -> Result<Config> {
        let args: Vec<String> = args().collect();
        let config_file = match args.len() {
            2 => args.get(1).unwrap(),
            _ => "blockwatch.toml",
        };

        let config: Config = Figment::new()
            .merge(Toml::file(config_file))
            .merge(Env::raw().split("__"))
            .extract()?;

        Ok(config)
    }

    pub fn get_hook(&self, chain_id: i32, hook_id: &str) -> Option<&Hook> {
        self.networks
            .values()
            .find(|n| n.chain_id == chain_id)
            .and_then(|n| n.hooks.get(hook_id))
    }
}

fn contracts_to_lowercase<'de, D>(
    deserializer: D,
) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Vec<String> = Deserialize::deserialize(deserializer)?;
    Ok(s.into_iter().map(|s| s.to_lowercase()).collect())
}
