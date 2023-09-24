use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::error;

#[derive(Deserialize, Debug)]
struct RPCResponse<T> {
    result: T,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub number: String,
    pub logs_bloom: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Log {
    pub log_index: String,
    pub transaction_hash: String,
    pub block_hash: String,
    pub block_number: String,
    pub address: String,
    pub data: String,
    pub topics: Vec<String>,
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
    block_number: &str,
) -> Result<Option<Block>> {
    let params = json!([block_number, false]);
    let block: Option<Block> =
        send_rpc(rpc_url, "eth_getBlockByNumber", params).await?;

    Ok(block)
}

pub async fn get_logs(rpc_url: &str, block_number: &str) -> Result<Vec<Log>> {
    let params = json!([{
        "fromBlock": block_number,
        "toBlock": block_number,
    }]);
    let logs: Vec<Log> = send_rpc(rpc_url, "eth_getLogs", params).await?;

    Ok(logs)
}
