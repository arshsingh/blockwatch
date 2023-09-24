use anyhow::Result;
use sqlx::any::{install_default_drivers, AnyRow};
use sqlx::{AnyPool, Row};
use svix_ksuid::*;

use crate::rpc::Log;

pub struct Delivery {
    pub id: String,
    pub chain_id: i32,
    pub hook_id: String,
    pub block_number: String,
    pub logs: Vec<Log>,
    pub failed_at: Option<String>,
}

fn generate_id() -> String {
    Ksuid::new(None, None).to_string()
}

pub async fn get_db_pool(db_url: &str) -> Result<AnyPool> {
    install_default_drivers();

    AnyPool::connect(db_url)
        .await
        .map_err(|e| anyhow::anyhow!("cannot connect to the db: {}", e))
}

pub async fn migrate(pool: &AnyPool) -> Result<()> {
    sqlx::migrate!("./src/migrations")
        .run(pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to migrate db: {}", e))
}

pub async fn get_last_block_number(
    pool: &AnyPool,
    chain_id: i32,
) -> Result<Option<String>> {
    let sql = "SELECT last_block_number FROM trackers WHERE chain_id = $1;";
    let num = sqlx::query_scalar::<_, String>(sql)
        .bind(i64::from(chain_id))
        .fetch_optional(pool)
        .await?;

    Ok(num)
}

type HookID = String;

pub async fn mark_block_processed(
    pool: &AnyPool,
    chain_id: i32,
    block_number: &str,
    deliveries: &Vec<(HookID, Vec<&Log>)>,
) -> Result<()> {
    let mut tx = pool.begin().await?;

    for delivery in deliveries {
        sqlx::query(
            "INSERT INTO deliveries (id, chain_id, hook_id, block_number, logs)
             VALUES ($1, $2, $3, $4, $5);",
        )
        .bind(generate_id())
        .bind(chain_id)
        .bind(&delivery.0)
        .bind(block_number)
        .bind(serde_json::to_string(&delivery.1)?)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("INSERT INTO trackers (chain_id, last_block_number)
                 VALUES ($1, $2)
                 ON CONFLICT (chain_id) DO UPDATE SET
                    last_block_number = $2, last_block_processed_at = CURRENT_TIMESTAMP;")
        .bind(chain_id)
        .bind(block_number)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    Ok(())
}

pub async fn get_pending_deliveries(pool: &AnyPool) -> Result<Vec<Delivery>> {
    let sql =
        "SELECT id, chain_id, hook_id, block_number, logs FROM deliveries WHERE failed_at IS NULL;";
    sqlx::query(sql)
        .fetch_all(pool)
        .await?
        .into_iter()
        .map(|row: AnyRow| {
            Ok(Delivery {
                id: row.try_get("id")?,
                chain_id: row.try_get("chain_id")?,
                hook_id: row.try_get("hook_id")?,
                block_number: row.try_get("block_number")?,
                logs: serde_json::from_str(row.try_get("logs")?)?,
                failed_at: None,
            })
        })
        .collect::<Result<Vec<Delivery>>>()
}

pub async fn mark_delivery_failed(pool: &AnyPool, id: &str) -> Result<()> {
    sqlx::query(
        "UPDATE deliveries SET failed_at = current_timestamp WHERE id = $1;",
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}
