use std::time::Duration;

use anyhow::Result;
use serde_json::json;
use sqlx::AnyPool;
use tracing::error;

use crate::config;
use crate::db;

pub async fn deliver(pool: &AnyPool, config: &config::Config) -> Result<()> {
    let deliveries = db::get_pending_deliveries(&pool).await?;

    // TODO: deliveries are made sequentially right now but can be done
    // concurrently. however, it won't make a difference on sqlite since
    // the tx will block all writes
    for delivery in deliveries {
        let hook = config.get_hook(delivery.chain_id, &delivery.hook_id);

        if hook.is_none() {
            error!(
                chain_id = delivery.chain_id,
                hook_id = delivery.hook_id,
                "network/hook config not found, dropping delivery"
            );
            continue;
        }

        let _ = deliver_block(pool, hook.unwrap(), &delivery).await;
    }

    Ok(())
}

#[tracing::instrument(
    skip_all, err,
    fields(chain = delivery.chain_id, block = delivery.block_number)
)]
async fn deliver_block(
    pool: &AnyPool,
    hook: &config::Hook,
    delivery: &db::Delivery,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM deliveries WHERE id = $1")
        .bind(&delivery.id)
        .execute(&mut *tx)
        .await?;

    let payload = json!({
        "id": delivery.id,
        "chain_id": delivery.chain_id,
        "block_number": delivery.block_number,
        "logs": delivery.logs
    });

    match send_webhook(&hook.url, payload).await {
        Ok(_) => tx.commit().await?,
        Err(_) => {
            tx.rollback().await?;
            db::mark_delivery_failed(pool, &delivery.id).await?;
        }
    }

    Ok(())
}

#[tracing::instrument(skip(body), err)]
async fn send_webhook(url: &str, body: serde_json::Value) -> Result<()> {
    let client = reqwest::Client::new();
    client
        .post(url)
        .json(&body)
        .timeout(Duration::from_secs(5))
        .send()
        .await?
        .error_for_status()?;

    Ok(())
}
