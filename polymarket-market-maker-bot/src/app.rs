use crate::args::Args;
use crate::clob_api::ClobApi;
use crate::constants::MAX_DECIMALS;
use crate::contracts::Contracts;
use crate::gas::{GasStation, GasStrategy};
use crate::lifecycle::Lifecycle;
use crate::market::Market;
use std::str::FromStr;
use crate::metrics::{register_metrics, KEEPER_BALANCE_AMOUNT};
use crate::order::{Order, Side};
use crate::orderbook::OrderBookManager;
use crate::price_feed::{PriceFeed, PriceFeedClob};
use crate::strategy::{Strategy, StrategyManager};
use crate::token::{Token, COLLATERAL};
use anyhow::Result;
use ethers::prelude::*;
use prometheus::Registry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::sleep;

// Main app struct - holds all the pieces together
pub struct App {
    sync_interval: u64, // How often we sync (in secs)
    clob_api: ClobApi, // API client for CLOB
    market: Market, // Market we're trading on
    price_feed: Arc<PriceFeedClob>, // Price feed for getting midpoints
    order_book_manager: OrderBookManager, // Manages orderbook state
    strategy_manager: StrategyManager, // Strategy logic (AMM/Bands)
    contracts: Contracts, // Ethereum contract interactions
    address: Address, // Our wallet address
}

impl App {
    pub async fn new(args: Vec<String>) -> Result<Self> {
        let args = crate::args::get_args(args); // Parse CLI args

        let registry = Registry::new();
        register_metrics(&registry); // Set up Prometheus metrics

        let provider = Provider::<Http>::try_from(&args.rpc_url)?; // Connect to RPC
        let wallet = LocalWallet::from_str(&args.private_key)?; // Load wallet from key
        let address = wallet.address(); // Get our address

        let clob_api = ClobApi::new(
            args.clob_api_url.clone(),
            provider.get_chainid().await?.as_u64(),
            args.private_key.clone(),
        );

        let collateral_address = clob_api.get_collateral_address().await; // Get collateral token addr
        let market = Market::new(args.condition_id.clone(), collateral_address.clone()); // Create market

        let gas_strategy = GasStrategy::from_str(&args.gas_strategy)
            .unwrap_or(GasStrategy::Web3); // Default to web3 if invalid
        let gas_station = GasStation::new(
            gas_strategy,
            args.fixed_gas_price,
            args.gas_station_url,
        ); // Set up gas pricing

        let contracts = Contracts::new(provider.clone(), gas_station, address); // Contract wrapper

        let price_feed = Arc::new(PriceFeedClob::new(market.clone(), clob_api.clone())); // Price feed
        let price_feed_for_strategy = Arc::clone(&price_feed) as Arc<dyn PriceFeed>; // Trait object for strategy

        let strategy = Strategy::from_str(&args.strategy)
            .ok_or_else(|| anyhow::anyhow!("Invalid strategy"))?; // Parse strategy type
        let strategy_manager = StrategyManager::new(
            strategy,
            &args.strategy_config,
            price_feed_for_strategy,
        )?; // Create strategy manager

        let mut order_book_manager = OrderBookManager::new(args.refresh_frequency); // Create orderbook manager
        let clob_api_for_orders = clob_api.clone();
        let market_for_orders = market.clone();
        order_book_manager.get_orders_with(move || {
            // Fetch orders from CLOB API - gotta convert JSON to Order structs
            let rt = tokio::runtime::Runtime::new().unwrap();
            let orders = rt.block_on(
                clob_api_for_orders.get_orders(&market_for_orders.condition_id)
            ).unwrap_or_default();
            orders
                .into_iter()
                .map(|order_dict| {
                    // Calculate remaining size (original - matched)
                    let size = order_dict.get("original_size")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0) - 
                        order_dict.get("size_matched")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    Order::new(
                        size,
                        order_dict.get("price").and_then(|v| v.as_f64()).unwrap_or(0.0),
                        Side::from_str(order_dict.get("side").and_then(|v| v.as_str()).unwrap_or("BUY")).unwrap_or(Side::Buy),
                        market_for_orders.token(order_dict.get("asset_id").and_then(|v| v.as_u64()).unwrap_or(0)).unwrap_or(Token::A),
                        order_dict.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                    )
                })
                .collect()
        });

        let contracts_for_balances = contracts.clone();
        let address_for_balances = address;
        let clob_api_for_balances = clob_api.clone();
        let market_for_balances = market.clone();
        order_book_manager.get_balances_with(move || {
            // Fetch balances from chain - tbd: implement actual contract calls
            let rt = tokio::runtime::Runtime::new().unwrap();
            let mut balances = HashMap::new();
            // TODO: Gotta fetch real balances from contracts_for_balances
            balances.insert(COLLATERAL.to_string(), 0.0);
            balances.insert("TokenA".to_string(), 0.0);
            balances.insert("TokenB".to_string(), 0.0);
            balances
        });

        let clob_api_for_place = clob_api.clone();
        let market_for_place = market.clone();
        order_book_manager.place_orders_with(move |order: Order| {
            // Place order via CLOB API - returns order ID if successful
            let rt = tokio::runtime::Runtime::new().unwrap();
            let order_id = rt.block_on(
                clob_api_for_place.place_order(
                    order.price,
                    order.size,
                    order.side.value(),
                    market_for_place.token_id(order.token),
                )
            );
            order_id.map(|id| Order::new(order.size, order.price, order.side, order.token, Some(id))) // Add ID to order
        });

        let clob_api_for_cancel = clob_api.clone();
        order_book_manager.cancel_orders_with(move |order: &Order| {
            if let Some(ref id) = order.id {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(clob_api_for_cancel.cancel_order(id))
            } else {
                true
            }
        });

        let clob_api_for_cancel_all = clob_api.clone();
        order_book_manager.cancel_all_orders_with(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(clob_api_for_cancel_all.cancel_all_orders())
        });

        order_book_manager.start();

        Ok(Self {
            sync_interval: args.sync_interval,
            clob_api,
            market,
            price_feed,
            order_book_manager,
            strategy_manager,
            contracts,
            address,
        })
    }

    pub async fn main(&self) -> Result<()> {
        let mut lifecycle = Lifecycle::new();

        lifecycle.on_startup(|| {
            Box::pin(async {
                log::info!("Running startup callback...");
                // TODO: Gotta approve tokens before we can trade
                sleep(tokio::time::Duration::from_secs(5)).await; // Wait for orderbook to populate
                log::info!("Startup complete!");
            })
        });

        let order_book_manager = &self.order_book_manager;
        let strategy_manager = &self.strategy_manager;
        lifecycle.every(self.sync_interval, move || {
            // Main sync loop - runs every sync_interval seconds
            let order_book_manager = order_book_manager;
            let strategy_manager = strategy_manager;
            Box::pin(async move {
                log::debug!("Synchronizing orderbook...");
                if let Ok(orderbook) = order_book_manager.get_order_book().await {
                    // Get current orderbook state
                    if let Ok((orders_to_cancel, orders_to_place)) =
                        strategy_manager.synchronize(&orderbook).await
                    {
                        // Strategy decides what to do
                        if !orders_to_cancel.is_empty() {
                            log::info!("About to cancel {} existing orders!", orders_to_cancel.len());
                            order_book_manager.cancel_orders(orders_to_cancel).await; // Cancel stale orders
                        }
                        if !orders_to_place.is_empty() {
                            log::info!("About to place {} new orders!", orders_to_place.len());
                            order_book_manager.place_orders(orders_to_place).await; // Place new ones
                        }
                    }
                }
                log::debug!("Synchronized orderbook!");
            })
        });

        let order_book_manager_for_shutdown = &self.order_book_manager;
        lifecycle.on_shutdown(|| {
            // Cleanup on shutdown - gotta cancel all orders before exit
            let order_book_manager = order_book_manager_for_shutdown;
            Box::pin(async move {
                log::info!("Keeper shutting down...");
                order_book_manager.cancel_all_orders().await; // Cancel everything
                log::info!("Keeper is shut down!");
            })
        });

        lifecycle.run().await;
        Ok(())
    }
}

