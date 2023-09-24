use anyhow::Result;
use tokio::sync::mpsc;
use tokio::time::{interval, sleep, Duration};
use tracing::error;
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

    // Pending deliveries channel. Each listener will try_send
    // a message on this channel to indicate that there are new
    // messages to deliver.
    let (delivery_tx, mut delivery_rx) = mpsc::channel::<()>(1);

    for (_, network) in config.networks.clone() {
        let pool = pool.clone();
        let tx = delivery_tx.clone();

        tokio::spawn(async move {
            while let Err(e) = listen(&pool, &network, &tx).await {
                error!(
                    chain_id=network.chain_id, error=?e,
                    "listener failed, restarting in 5 secs",
                );
                sleep(Duration::from_secs(5)).await;
            }
        });
    }

    loop {
        // Wait for a listener to indicate that there are new deliveries
        // but check every 5 seconds anyway to make sure we don't miss anything
        let mut timeout = interval(Duration::from_secs(5));

        let _ = tokio::select! {
            _ = delivery_rx.recv() => deliver(&pool, &config).await?,
            _ = timeout.tick() => deliver(&pool, &config).await?
        };
    }
}
