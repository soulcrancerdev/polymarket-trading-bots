// Main entry point - sets up the market maker & runs it
mod app;
mod args;
mod clob_api;
mod constants;
mod contracts;
mod ct_helpers;
mod gas;
mod lifecycle;
mod market;
mod metrics;
mod order;
mod orderbook;
mod price_feed;
mod strategy;
mod strategies;
mod token;
mod utils;

use anyhow::Result;
use app::App;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init(); // Init logging - gotta see what's happening
    let args: Vec<String> = std::env::args().collect();
    let app = App::new(args[1..].to_vec()).await?; // Create app w/ CLI args
    app.main().await?; // Run the main loop
    Ok(())
}

