use std::time::Duration;

use anyhow::Result;
use ethbloom::Bloom;
use ethers_core::types::{
    Address, Block as PossiblyPendingBlock, Log, TxHash, H256, U256, U64,
};
use serde::Deserialize;
use serde_json::json;
use tracing::error;

#[derive(Deserialize, Debug)]
struct RPCResponse<T> {
    result: T,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub hash: H256,
    pub number: U64,
    pub logs_bloom: Bloom,
    pub timestamp: U256,
    pub transactions: Vec<TxHash>,
}

impl TryFrom<PossiblyPendingBlock<TxHash>> for Block {
    type Error = anyhow::Error;

    fn try_from(block: PossiblyPendingBlock<TxHash>) -> Result<Self> {
        let err =
            || anyhow::anyhow!("cannot derive block from pending block data");

        Ok(Self {
            hash: block.hash.ok_or_else(err)?,
            number: block.number.ok_or_else(err)?,
            logs_bloom: block.logs_bloom.ok_or_else(err)?,
            timestamp: block.timestamp,
            transactions: block.transactions,
        })
    }
}

#[tracing::instrument(skip(rpc_url))]
pub async fn send_rpc<T: serde::de::DeserializeOwned>(
    rpc_url: &str,
    method: &str,
    params: serde_json::Value,
) -> Result<T> {
    let client = reqwest::Client::new();
    let res = client
        .post(rpc_url)
        .timeout(Duration::from_secs(10))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 1 as u8,
            "method": method,
            "params": params
        }))
        .send()
        .await?;

    let status = res.status();
    if status.is_server_error() || status.is_client_error() {
        let body = res.text().await.unwrap_or("".to_string());
        error!(status = status.as_u16(), response = body, "RPC error");
        return Err(anyhow::anyhow!("RPC error"));
    }

    Ok(res.json::<RPCResponse<T>>().await?.result)
}

pub async fn get_block(
    rpc_url: &str,
    block_number: U64,
) -> Result<Option<Block>> {
    let params = json!([block_number, false]);

    send_rpc(rpc_url, "eth_getBlockByNumber", params).await
}

pub async fn get_latest_block_number(rpc_url: &str) -> Result<U64> {
    send_rpc(rpc_url, "eth_blockNumber", json!([])).await
}

pub async fn get_logs(
    rpc_url: &str,
    from_block: U64,
    to_block: U64,
    contracts: &[Address],
) -> Result<Vec<Log>> {
    let params = json!([{
        "fromBlock": from_block,
        "toBlock": to_block,
        "address": &contracts,
    }]);
    send_rpc(rpc_url, "eth_getLogs", params).await
}
