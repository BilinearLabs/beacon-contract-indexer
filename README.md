# beacon-contract-indexer

Indexes the Ethereum deposit contract parallelizing the requests using a producer-consumer architecture.
Tasks are spawned asynchronously by the producer, and the consumer consumes them in order using a buffer
to store out-of-order events.

Requests can be parallelized across multiple RPCs, meaning you can pass the `rpc` flag as many times as you want.
Note that once it reaches the head of the blockchain, it will keep waiting for new blocks and indexing what's left. With a single beefy RPC it can process around 40k blocks per second in the mainnet deposit contract.

Run:
```
cargo run -- --rpc=https://[your-rpc-endpoint]
```

And for other networks other than mainnet:
```
cargo run -- \
--rpc=https://[your-rpc-endpoint] \
--deposit_address=0x \
--start_block=0
```