use std::collections::HashMap;
use std::env::args;
use std::time::Duration;

use anyhow::{Context, Result};
use ethers_core::types::Address;
use figment::{
    providers::{Data, Env, Format, Json},
    Figment,
};
use futures::future::join_all;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    // A postgres or sqlite database URL
    pub database_url: String,

    // Blockwatch can listen to multiple networks
    //
    // The key of this map is a name for the network. It can be
    // anything and is not really used.
    pub networks: HashMap<String, Network>,
    pub hooks: HashMap<String, Hook>,
}

#[derive(Deserialize, Clone)]
pub struct Network {
    pub chain_id: i32,
    pub rpc_url: String,
    pub block_time: u64,

    // No. of blocks to fetch logs for in the eth_getLogs filter
    // This is used when there is a backlog of blocks to process,
    // for e.g. if the service was down for some time.
    //
    // The RPC node might have a limit on the number of blocks that can
    // be fetched in a single request, so this value should be set accordingly.
    //
    // Default: 2000
    pub logs_page_size: Option<u64>,
}

#[derive(Deserialize, Clone)]
pub struct Hook {
    pub chain_id: i32,
    pub contracts: Vec<Address>,
    pub url: String,

    // Timeout in seconds for the HTTP request to the hook
    // If the hook does not respond within this time, the delivery
    // is considered failed.
    //
    // Default: 5
    pub timeout: Option<u64>,
}

impl Config {
    pub async fn load() -> Result<Config> {
        let args = args().skip(1).collect::<Vec<_>>();
        let config_files = match args.len() {
            0 => vec!["blockwatch.config.json".to_string()],
            _ => args,
        };

        let config = join_all(config_files.into_iter().map(load_json_config))
            .await
            .iter()
            .fold(Figment::new(), |config, json| config.merge(json))
            .merge(Env::raw().split("__"))
            .extract()?;

        Ok(config)
    }

    pub fn get_network(&self, chain_id: i32) -> Result<&Network> {
        self.networks
            .values()
            .find(|n| n.chain_id == chain_id)
            .context("Network not found")
    }

    pub fn get_contracts(&self, chain_id: i32) -> Vec<Address> {
        self.hooks
            .values()
            .filter(|h| h.chain_id == chain_id)
            .flat_map(|h| h.contracts.clone())
            .collect::<Vec<_>>()
    }

    pub fn get_hooks(&self, chain_id: i32) -> Vec<(&String, &Hook)> {
        self.hooks
            .iter()
            .filter_map(|(hook_id, hook)| {
                if hook.chain_id == chain_id {
                    Some((hook_id, hook))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }
}

async fn load_json_config(url: String) -> Data<Json> {
    let data = match &url[..7] {
        "http://" | "https:/" => {
            let client = reqwest::Client::new();
            let res = client
                .get(&url)
                .timeout(Duration::from_secs(10))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();

            Json::string(&res)
        }
        _ => Json::file(url),
    };

    data
}
