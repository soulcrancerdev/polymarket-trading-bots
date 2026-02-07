use prometheus::{Counter, Gauge, Histogram, Registry, HistogramOpts, Opts};
use std::sync::Arc;

lazy_static::lazy_static! {
    pub static ref CHAIN_REQUESTS_COUNTER: Counter = Counter::with_opts(
        Opts::new("market_maker_chain_requests_counter", "Counts the chain executions")
            .namespace("market_maker")
    ).unwrap();

    pub static ref KEEPER_BALANCE_AMOUNT: Gauge = Gauge::with_opts(
        Opts::new("market_maker_balance_amount", "Balance of the bot")
            .namespace("market_maker")
    ).unwrap();

    pub static ref CLOB_REQUESTS_LATENCY: Histogram = Histogram::with_opts(
        HistogramOpts::new("market_maker_clob_requests_latency", "Latency of the clob requests")
            .namespace("market_maker")
    ).unwrap();

    pub static ref GAS_STATION_LATENCY: Histogram = Histogram::with_opts(
        HistogramOpts::new("market_maker_gas_station_latency", "Latency of the gas station")
            .namespace("market_maker")
    ).unwrap();
}

pub fn register_metrics(registry: &Registry) {
    registry.register(Box::new(CHAIN_REQUESTS_COUNTER.clone())).unwrap();
    registry.register(Box::new(KEEPER_BALANCE_AMOUNT.clone())).unwrap();
    registry.register(Box::new(CLOB_REQUESTS_LATENCY.clone())).unwrap();
    registry.register(Box::new(GAS_STATION_LATENCY.clone())).unwrap();
}

