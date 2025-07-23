use alloy::rpc::types::Log;
use alloy::transports::http::reqwest::Url;
use beacon_contract_indexer::cli::Cli;
use beacon_contract_indexer::contract::DepositContract;
use beacon_contract_indexer::tasks;
use clap::Parser;
use std::error::Error;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let cli = Cli::parse();
    let rpcs: Vec<Url> = cli
        .rpcs
        .into_iter()
        .map(|u| u.parse())
        .collect::<Result<_, _>>()?;

    // We consider we processed all the blocks up to the start block.
    let processed_to = cli.start_block.saturating_sub(1);

    // Channel where event producer and consumer communicate.
    let (tx, rx) =
        mpsc::channel::<(u64, u64, Vec<(DepositContract::DepositEvent, Log)>)>(cli.max_queue_size);

    // Spawm the producer
    let producer_handle = tasks::spawn_producer(
        rpcs,
        cli.deposit_address.parse()?,
        tx.clone(),
        cli.parallel_requests_per_rpc,
        cli.block_range,
        processed_to,
    );

    // Spawn the consumer.
    let consumer_handle = tasks::spawn_consumer(rx, processed_to);

    // Wait for Ctrl+C then shut down gracefully.
    tokio::signal::ctrl_c().await?;
    println!("Ctrl+C received, shutting down...");

    // Close channel so consumer finishes.
    drop(tx);

    Ok(())
}
