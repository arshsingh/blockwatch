use anyhow::Result;
use tokio::sync::mpsc;
use tokio::time::{interval, sleep, Duration};
use tracing::{error, info};
use tracing_subscriber::FmtSubscriber;

use blockwatch::config::Config;
use blockwatch::db;
use blockwatch::delivery::deliver;
use blockwatch::listener::listen;

#[tokio::main]
async fn main() -> Result<()> {
    let subscriber = FmtSubscriber::new();
    let _ = tracing::subscriber::set_global_default(subscriber).map_err(|e| {
        println!("failed to initialize tracing: {:?}", e);
    });

    let config = Config::load().await?;
    let pool = db::get_db_pool(&config.database_url).await?;

    db::migrate(&pool).await?;

    info!("Starting listeners for {} networks", config.networks.len());

    // Pending deliveries channel. Each listener will try_send
    // a message on this channel to indicate that there are new
    // messages to deliver.
    let (delivery_tx, mut delivery_rx) = mpsc::channel::<()>(1);

    for network in config.networks.values() {
        let config = config.clone();
        let pool = pool.clone();
        let tx = delivery_tx.clone();
        let chain_id = network.chain_id.clone();

        tokio::spawn(async move {
            while let Err(e) = listen(&pool, &config, chain_id, &tx).await {
                error!(
                    chain_id=chain_id, error=?e,
                    "Listener failed, restarting in 5 secs",
                );
                sleep(Duration::from_secs(5)).await;
            }
        });
    }

    let mut timeout = interval(Duration::from_secs(5));
    loop {
        // Wait for a listener to indicate that there are new deliveries
        // but check every 5 seconds anyway to make sure we don't miss anything
        let _ = tokio::select! {
            _ = delivery_rx.recv() => deliver(&pool, &config).await?,
            _ = timeout.tick() => deliver(&pool, &config).await?
        };

        timeout.reset();
    }
}
