use crate::order::{Order, Side};
use crate::orderbook::OrderBook;
use crate::strategies::bands::Bands;
use crate::strategies::base_strategy::BaseStrategy;
use crate::token::{Token, COLLATERAL};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct BandsStrategy {
    bands: Bands,
}

impl BandsStrategy {
    pub fn new(config: &serde_json::Value) -> Self {
        let bands_config = config["bands"].as_array().unwrap();
        Self {
            bands: Bands::new(bands_config),
        }
    }
}

#[async_trait]
impl BaseStrategy for BandsStrategy {
    async fn get_orders(
        &self,
        orderbook: &OrderBook,
        target_prices: &HashMap<Token, f64>,
    ) -> (Vec<Order>, Vec<Order>) {
        let mut orders_to_place = Vec::new();
        let mut orders_to_cancel = Vec::new();

        for token in [Token::A, Token::B] {
            let orders = self.orders_by_corresponding_buy_token(&orderbook.orders, token);
            orders_to_cancel.extend(self.bands.cancellable_orders(&orders, target_prices[&token]));
        }

        let open_orders: Vec<Order> = orderbook
            .orders
            .iter()
            .filter(|order| !orders_to_cancel.contains(order))
            .cloned()
            .collect();

        let balance_locked_by_open_buys: f64 = open_orders
            .iter()
            .filter(|order| order.side == Side::Buy)
            .map(|order| order.size * order.price)
            .sum();

        let free_collateral_balance = orderbook
            .balances
            .get(COLLATERAL)
            .copied()
            .unwrap_or(0.0)
            - balance_locked_by_open_buys;

        let mut free_collateral = free_collateral_balance;

        for token in [Token::A, Token::B] {
            let orders = self.orders_by_corresponding_buy_token(&orderbook.orders, token);
            let balance_locked_by_open_sells: f64 = orders
                .iter()
                .filter(|order| order.side == Side::Sell)
                .map(|order| order.size)
                .sum();

            let complement_token = token.complement();
            let free_token_balance = orderbook
                .balances
                .get(&complement_token.value().to_string())
                .copied()
                .unwrap_or(0.0)
                - balance_locked_by_open_sells;

            let new_orders = self.bands.new_orders(
                &orders,
                free_collateral,
                free_token_balance,
                target_prices[&token],
                token,
            );

            let collateral_used: f64 = new_orders
                .iter()
                .filter(|order| order.side == Side::Buy)
                .map(|order| order.size * order.price)
                .sum();

            free_collateral -= collateral_used;
            orders_to_place.extend(new_orders);
        }

        (orders_to_cancel, orders_to_place)
    }
}

impl BandsStrategy {
    fn orders_by_corresponding_buy_token(&self, orders: &[Order], buy_token: Token) -> Vec<Order> {
        orders
            .iter()
            .filter(|order| {
                (order.side == Side::Buy && order.token == buy_token)
                    || (order.side == Side::Sell && order.token != buy_token)
            })
            .cloned()
            .collect()
    }
}

