use crate::contract::DepositContract;
use crate::contract::DepositContract::DepositEvent;
use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::client::RpcClient;
use alloy::rpc::types::{BlockNumberOrTag, Log};
use alloy::transports::http::reqwest::Url;
use alloy::transports::layers::RetryBackoffLayer;
use futures::stream::{self, TryStreamExt};
use std::collections::BTreeMap;
use std::error::Error;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::task::{self, JoinHandle};
use tokio::time::sleep;

type SenderType = mpsc::Sender<(u64, u64, Vec<(DepositEvent, Log)>)>;
type ReceiverType = mpsc::Receiver<(u64, u64, Vec<(DepositEvent, Log)>)>;

// Spawns a producer task that fetches deposit events in parallel
// spawning up to `parallel_requests_per_rpc` requests per RPC in
// ranges of `block_range` blocks. Note that the channel size is
// used to handle the backpressure.
pub fn spawn_producer(
    rpcs: Vec<Url>,
    deposit_address: Address,
    tx: SenderType,
    parallel_requests_per_rpc: usize,
    block_range: u64,
    start_processed: u64,
) -> JoinHandle<Result<(), Box<dyn Error + Send + Sync>>> {
    let max_retry: u32 = 100;
    let backoff: u64 = 2000;
    let cups: u64 = 100;
    let rr_counter = Arc::new(AtomicUsize::new(0));
    let providers: Vec<Arc<_>> = rpcs
        .into_iter()
        .map(|url| {
            Arc::new(
                ProviderBuilder::new().connect_client(
                    RpcClient::builder()
                        .layer(RetryBackoffLayer::new(max_retry, backoff, cups))
                        .http(url),
                ),
            )
        })
        .collect();

    let deposit_contracts: Arc<Vec<_>> = Arc::new(
        providers
            .iter()
            .map(|p| Arc::new(DepositContract::new(deposit_address, Arc::clone(p))))
            .collect(),
    );

    let parallel_queries = parallel_requests_per_rpc * providers.len();

    task::spawn(async move {
        let mut processed_to = start_processed;

        loop {
            // Query finalized block height.
            let finalized_block = providers[0]
                .get_block_by_number(BlockNumberOrTag::Finalized)
                .await?
                .ok_or("Finalized block is None")?
                .header
                .number;

            let remaining = finalized_block.saturating_sub(processed_to);
            if remaining == 0 {
                sleep(Duration::from_secs(1)).await;
                continue;
            }

            // Build chunk start heights.
            let chunk_starts: Vec<u64> = ((processed_to + 1)..=finalized_block)
                .step_by(block_range as usize)
                .collect();

            // Iterate over chunks in parallel.
            stream::iter(
                chunk_starts
                    .into_iter()
                    .map(Ok::<u64, Box<dyn Error + Send + Sync>>),
            )
            .try_for_each_concurrent(parallel_queries, |chunk_start| {
                let tx = tx.clone();
                let deposit_contracts = Arc::clone(&deposit_contracts);
                let rr_counter = Arc::clone(&rr_counter);
                async move {
                    let chunk_end = std::cmp::min(chunk_start + block_range - 1, finalized_block);

                    // Select next RPC with a simple round-robin.
                    let idx = rr_counter.fetch_add(1, Ordering::Relaxed) % deposit_contracts.len();
                    let deposit_contract = Arc::clone(&deposit_contracts[idx]);

                    println!(
                        "[{:?}-{:?}] Fetching events for block range (RPC #{})",
                        chunk_start, chunk_end, idx
                    );

                    let events = deposit_contract
                        .DepositEvent_filter()
                        .from_block(chunk_start)
                        .to_block(chunk_end)
                        .query()
                        .await?;

                    tx.send((chunk_start, chunk_end, events)).await?;
                    Ok::<(), Box<dyn Error + Send + Sync>>(())
                }
            })
            .await?;

            processed_to = finalized_block;
        }
    })
}

// Spawns a consumer task that orders the events by block numbers. Out of order
// events are buffered in a BTreeMap. The moment the next batch by order becomes
// available, it is processed and cleared from the buffer.
pub fn spawn_consumer(
    mut rx: ReceiverType,
    start_processed: u64,
) -> JoinHandle<Result<(), Box<dyn Error + Send + Sync>>> {
    task::spawn(async move {
        let mut last_processed = start_processed;

        // For periodic throughput reporting.
        let mut window_start = Instant::now();
        let mut window_blocks: u64 = 0;

        let mut buffer: BTreeMap<u64, (u64, Vec<(DepositEvent, Log)>)> = BTreeMap::new();

        while let Some((chunk_start, chunk_end, events)) = rx.recv().await {
            buffer.insert(chunk_start, (chunk_end, events));

            while let Some((end, ev)) = buffer.remove(&(last_processed + 1)) {
                println!(
                    "[{:?}-{:?}] Processing events for block range. Found {} deposits",
                    last_processed + 1,
                    end,
                    ev.len()
                );

                for (event, log) in ev {
                    println!(
                        "pubkey: {:?} withdrawal_credentials: {:?} amount: {:?} signature: {:?} index: {:?} block_number: {:?} transaction_hash: {:?} log_index: {:?}",
                        event.pubkey,
                        event.withdrawal_credentials,
                        event.amount,
                        event.signature,
                        event.index,
                        log.block_number.unwrap_or_default(),
                        log.transaction_hash.unwrap_or_default(),
                        log.log_index.unwrap_or_default()
                    );
                }

                // Compute how many blocks were processed in this chunk and add to window.
                let blocks_this_chunk = end - last_processed;
                window_blocks += blocks_this_chunk;

                last_processed = end;

                // If 5 seconds have passed, print average throughput for the window.
                let elapsed = window_start.elapsed();
                if elapsed.as_secs() >= 5 {
                    let blocks_per_sec = window_blocks as f64 / elapsed.as_secs_f64();
                    //println!(
                    //    "{} blocks in {:?} -> {:.2} blocks/sec",
                    //    window_blocks, elapsed, blocks_per_sec
                    //);
                    // Reset window counters.
                    window_start = Instant::now();
                    window_blocks = 0;
                }
            }
        }
        Ok::<(), Box<dyn Error + Send + Sync>>(())
    })
}
