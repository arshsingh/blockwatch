use anyhow::{anyhow, Result};
use ethbloom::Input;
use ethers_core::types::{Address, U64};
use sqlx::AnyPool;
use tokio::sync::mpsc::Sender;
use tokio::time::{sleep, Duration};
use tracing::info;

use crate::config;
use crate::db;
use crate::rpc::{get_block, get_latest_block_number, get_logs, Block};

pub async fn listen(
    pool: &AnyPool,
    config: &config::Config,
    chain_id: i32,
    delivery_tx: &Sender<()>,
) -> Result<()> {
    let network = config.get_network(chain_id)?;

    let latest_block = get_latest_block_number(&network.rpc_url).await?;
    let mut next_block =
        match db::get_last_block_number(&pool, chain_id).await? {
            Some(num) => num + 1,
            None => latest_block,
        };

    // if the listener is offline for a bit and then is restarted,
    // it needs to catch up on all the blocks that were produced
    // during the downtime.
    while next_block < latest_block {
        let to_block = next_block + network.logs_page_size.unwrap_or(2000);

        if process_block_range(&pool, config, chain_id, next_block, to_block)
            .await?
        {
            delivery_tx.send(()).await?;
        }

        // this means we've caught up to the latest block, but we might
        // have already processed blocks after latest_block that were produced
        // during this catch up process. so we need recalculate next_block
        // based on the value in the database.
        if to_block > latest_block {
            next_block = db::get_last_block_number(&pool, chain_id)
                .await?
                .ok_or(anyhow!("last_block_number not found in db"))
                .map(|num| num + 1)?
        } else {
            next_block = to_block + 1;
        }
    }

    loop {
        if let Some(block) = get_block(&network.rpc_url, next_block).await? {
            if process_block(&pool, config, chain_id, &block).await? {
                delivery_tx.send(()).await?;
            }
            next_block = block.number + 1;
        }

        sleep(Duration::from_secs(network.block_time.into())).await;
    }
}

#[tracing::instrument(skip(pool, config))]
async fn process_block_range(
    pool: &AnyPool,
    config: &config::Config,
    chain_id: i32,
    from_block: U64,
    to_block: U64,
) -> Result<bool> {
    let network = config.get_network(chain_id)?;
    let contracts = config.get_contracts(chain_id);

    let blocks = get_logs(&network.rpc_url, from_block, to_block, &contracts)
        .await?
        .into_iter()
        .fold(std::collections::HashMap::new(), |mut acc, log| {
            // if block_number is none, then it is a removed log
            // and we don't need to process it
            if let Some(block_number) = log.block_number {
                acc.entry(block_number).or_insert_with(Vec::new).push(log);
            }

            acc
        });

    let mut last_block_with_deliveries: Option<U64> = None;
    let mut deliveries_count = 0;

    for (block_number, logs) in blocks.into_iter() {
        let deliveries = config
            .get_hooks(chain_id)
            .iter()
            .filter_map(|(hook_id, hook)| {
                let logs = logs
                    .iter()
                    .filter(|log| hook.contracts.contains(&log.address))
                    .collect::<Vec<_>>();

                match logs.len() {
                    0 => None,
                    _ => Some((hook_id.to_string(), block_number, logs)),
                }
            })
            .collect::<Vec<_>>();

        if deliveries.len() > 0 {
            db::mark_block_processed(
                &pool,
                chain_id,
                block_number,
                &deliveries,
            )
            .await?;
            last_block_with_deliveries = Some(block_number);
            deliveries_count += deliveries.len();
        }
    }

    info!("Saved {} deliveries", deliveries_count);

    // If the last block that we saved deliveries for was not the to_block,
    // we still need to mark it as processed so that we don't try to fetch logs
    // for it again
    if last_block_with_deliveries != Some(to_block) {
        db::mark_block_processed(&pool, chain_id, to_block, &vec![]).await?;
    }

    Ok(deliveries_count > 0)
}

// Returns true if there are any deliveries in the block
#[tracing::instrument(
    skip(pool, config, block)
    fields(block = block.number.as_u64())
)]
async fn process_block(
    pool: &AnyPool,
    config: &config::Config,
    chain_id: i32,
    block: &Block,
) -> Result<bool> {
    // Check if any of the contracts we're interested in are included in the
    // bloom filter for this block. If not, we can skip fetching the logs
    let might_have_logs = config
        .get_contracts(chain_id)
        .iter()
        .any(|c| block_has_contract_log(&block, c));

    if !might_have_logs {
        db::mark_block_processed(&pool, chain_id, block.number, &Vec::new())
            .await?;

        return Ok(false);
    }

    process_block_range(&pool, config, chain_id, block.number, block.number)
        .await
}

fn block_has_contract_log(block: &Block, contract: &Address) -> bool {
    block
        .logs_bloom
        .contains_input(Input::Raw(contract.as_bytes()))
}
