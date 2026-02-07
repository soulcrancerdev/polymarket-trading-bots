use crate::order::Order;
use crate::orderbook::OrderBook;
use async_trait::async_trait;
use std::collections::HashMap;
use crate::token::Token;

#[async_trait]
pub trait BaseStrategy: Send + Sync {
    async fn get_orders(
        &self,
        orderbook: &OrderBook,
        token_prices: &HashMap<Token, f64>,
    ) -> (Vec<Order>, Vec<Order>);
}

