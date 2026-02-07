use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "poly-market-maker")]
pub struct Args {
    #[arg(long, required = true)]
    pub private_key: String,

    #[arg(long, required = true)]
    pub rpc_url: String,

    #[arg(long, required = true)]
    pub clob_api_url: String,

    #[arg(long, default_value = "30")]
    pub sync_interval: u64,

    #[arg(long, default_value = "15.0")]
    pub min_size: f64,

    #[arg(long, default_value = "0.01")]
    pub min_tick: f64,

    #[arg(long, default_value = "5")]
    pub refresh_frequency: u64,

    #[arg(long, default_value = "web3")]
    pub gas_strategy: String,

    #[arg(long)]
    pub gas_station_url: Option<String>,

    #[arg(long)]
    pub fixed_gas_price: Option<u64>,

    #[arg(long, default_value = "9008")]
    pub metrics_server_port: u16,

    #[arg(long, required = true)]
    pub condition_id: String,

    #[arg(long, required = true)]
    pub strategy: String,

    #[arg(long, required = true)]
    pub strategy_config: String,
}

pub fn get_args(args: Vec<String>) -> Args {
    Args::parse_from(args)
}

