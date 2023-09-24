use core::str::FromStr;

use anyhow::Result;
use ethbloom::{Bloom, Input};
use hex;
use sqlx::AnyPool;
use tokio::sync::mpsc::Sender;
use tokio::time::{sleep, Duration};
use tracing::info;

use crate::config;
use crate::db;
use crate::rpc::{get_block, get_logs, Block};

pub async fn listen(
    pool: &AnyPool,
    network: &config::Network,
    delivery_tx: &Sender<()>,
) -> Result<()> {
    let last_block = db::get_last_block_number(&pool, network.chain_id).await?;

    let mut next_block = match last_block {
        Some(num) => increment_block_number(&num)?,
        None => "latest".to_string(),
    };
    let mut is_caught_up = false;

    loop {
        match get_block(&network.rpc_url, &next_block).await? {
            Some(block) => {
                if process_block(&pool, &network, &block).await? {
                    delivery_tx.send(()).await?;
                }
                next_block = increment_block_number(&block.number)?;
            }
            None => is_caught_up = true,
        }

        // if the listener is offline for a bit and then is restarted,
        // it needs to catch up on all the blocks that were produced
        // during the downtime.
        //
        // only sleep if all blocks have been processed
        if is_caught_up {
            sleep(Duration::from_secs(network.block_time.into())).await;
        }
    }
}

// Returns true if there are any deliveries in the block
#[tracing::instrument(
    skip_all,
    fields(chain = network.chain_id, block = block.number)
)]
async fn process_block(
    pool: &AnyPool,
    network: &config::Network,
    block: &Block,
) -> Result<bool> {
    // Check if any of the contracts we're interested in are included in the
    // bloom filter for this block. If not, we can skip fetching the logs
    let might_have_logs = network
        .hooks
        .values()
        .flat_map(|h| h.contracts.iter().map(|c| c.to_string()))
        .any(|c| block_has_contract_log(&block, &c));

    if !might_have_logs {
        db::mark_block_processed(
            &pool,
            network.chain_id,
            &block.number,
            &Vec::new(),
        )
        .await?;

        return Ok(false);
    }

    info!("bloom filter matched. getting logs");

    let logs = get_logs(&network.rpc_url, &block.number).await?;
    let deliveries = network
        .hooks
        .iter()
        .filter_map(|(hook_id, hook)| {
            let logs = logs
                .iter()
                .filter(|log| {
                    hook.contracts.contains(&log.address.to_lowercase())
                })
                .collect::<Vec<_>>();

            match logs.len() {
                0 => None,
                _ => Some((hook_id.to_string(), logs.to_vec())),
            }
        })
        .collect::<Vec<_>>();

    db::mark_block_processed(
        &pool,
        network.chain_id,
        &block.number,
        &deliveries,
    )
    .await?;

    Ok(deliveries.len() > 0)
}

fn increment_block_number(hex: &str) -> Result<String> {
    let num = u64::from_str_radix(&hex[2..], 16)?;
    Ok(format!("0x{:x}", num + 1))
}

fn block_has_contract_log(block: &Block, contract: &str) -> bool {
    let log_bloom = Bloom::from_str(&block.logs_bloom[2..]).unwrap();
    let address = hex::decode(&contract[2..]).expect("invalid address");

    log_bloom.contains_input(Input::Raw(&address))
}
