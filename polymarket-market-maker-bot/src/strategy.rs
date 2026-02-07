use crate::order::Order;
use crate::orderbook::OrderBook;
use crate::price_feed::PriceFeed;
use crate::strategies::{AMMStrategy, BandsStrategy, BaseStrategy};
use crate::token::Token;
use std::collections::HashMap;
use std::sync::Arc;

pub enum Strategy {
    AMM,
    BANDS,
}

impl Strategy {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "amm" => Some(Strategy::AMM),
            "bands" => Some(Strategy::BANDS),
            _ => None,
        }
    }
}

pub struct StrategyManager {
    strategy: Box<dyn BaseStrategy>,
    price_feed: Arc<dyn PriceFeed>,
}

impl StrategyManager {
    pub fn new(
        strategy: Strategy,
        config_path: &str,
        price_feed: Arc<dyn PriceFeed>,
    ) -> anyhow::Result<Self> {
        let config = std::fs::read_to_string(config_path)?;
        let config_json: serde_json::Value = serde_json::from_str(&config)?;

        let strategy: Box<dyn BaseStrategy> = match strategy {
            Strategy::AMM => Box::new(AMMStrategy::new(&config_json)),
            Strategy::BANDS => Box::new(BandsStrategy::new(&config_json)),
        };

        Ok(Self {
            strategy,
            price_feed,
        })
    }

    pub async fn synchronize(&self, orderbook: &OrderBook) -> anyhow::Result<(Vec<Order>, Vec<Order>)> {
        if orderbook.balances.values().any(|&v| v == 0.0) {
            return Err(anyhow::anyhow!("Balances invalid/non-existent"));
        }

        let total_balance: f64 = orderbook.balances.values().sum();
        if total_balance == 0.0 {
            return Err(anyhow::anyhow!("Zero Balances"));
        }

        let price_a = self.price_feed.get_price(Token::A).await;
        let price_b = 1.0 - price_a;

        let mut token_prices = HashMap::new();
        token_prices.insert(Token::A, (price_a * 100.0).round() / 100.0);
        token_prices.insert(Token::B, (price_b * 100.0).round() / 100.0);

        let (orders_to_cancel, orders_to_place) = self.strategy.get_orders(orderbook, &token_prices).await;

        Ok((orders_to_cancel, orders_to_place))
    }
}

