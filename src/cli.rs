use clap::Parser;

#[derive(Parser)]
#[command(name = "Beacon Contract Indexer")]
#[command(about = "Indexes all deposit events from the Ethereum deposit contract", long_about = None)]
pub struct Cli {
    /// RPC URLs for the Ethereum L1 network. Provide multiple times, e.g.:
    /// --rpc https://mainnet.infura.io/v3/XXX --rpc https://eth.llamarpc.com
    #[arg(long = "rpc", required = true)]
    pub rpcs: Vec<String>,

    /// Optional "username:password" credentials for Basic Auth.
    #[arg(long = "credentials")]
    pub credentials: Option<String>,

    /// Deposit contract address.
    #[arg(
        long = "deposit-address",
        default_value = "0x00000000219ab540356cBB839Cbe05303d7705Fa"
    )]
    pub deposit_address: String,

    /// Number of blocks to process per query.
    #[arg(long = "block-range", default_value = "1000")]
    pub block_range: u64,

    /// Maximum queue size for the channel.
    #[arg(long = "max-queue-size", default_value = "100")]
    pub max_queue_size: usize,

    /// Parallel number of requests per RPC.
    #[arg(long = "parallel-requests-per-rpc", default_value = "5")]
    pub parallel_requests_per_rpc: usize,

    /// Start block.
    #[arg(long = "start-block", default_value = "11052984")]
    pub start_block: u64,
}
