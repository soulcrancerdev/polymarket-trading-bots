use crate::constants::MIN_SIZE;
use crate::order::Order;
use crate::orderbook::OrderBook;
use crate::strategies::amm::{AMMConfig, AMMManager};
use crate::strategies::base_strategy::BaseStrategy;
use crate::token::Token;
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OrderType {
    price: f64,
    side: crate::order::Side,
    token: Token,
}

impl OrderType {
    fn from_order(order: &Order) -> Self {
        Self {
            price: order.price,
            side: order.side,
            token: order.token,
        }
    }
}

pub struct AMMStrategy {
    amm_manager: AMMManager,
}

impl AMMStrategy {
    pub fn new(config_dict: &serde_json::Value) -> Self {
        let config = AMMConfig {
            p_min: config_dict["p_min"].as_f64().unwrap(),
            p_max: config_dict["p_max"].as_f64().unwrap(),
            spread: config_dict["spread"].as_f64().unwrap(),
            delta: config_dict["delta"].as_f64().unwrap(),
            depth: config_dict["depth"].as_f64().unwrap(),
            max_collateral: config_dict["max_collateral"].as_f64().unwrap(),
        };

        Self {
            amm_manager: AMMManager::new(config),
        }
    }
}

#[async_trait]
impl BaseStrategy for AMMStrategy {
    async fn get_orders(
        &self,
        orderbook: &OrderBook,
        target_prices: &HashMap<Token, f64>,
    ) -> (Vec<Order>, Vec<Order>) {
        let mut orders_to_cancel = Vec::new();
        let mut orders_to_place = Vec::new();

        let mut balances = HashMap::new();
        balances.insert("Collateral".to_string(), orderbook.balances.get("Collateral").copied().unwrap_or(0.0));
        balances.insert("TokenA".to_string(), orderbook.balances.get("TokenA").copied().unwrap_or(0.0));
        balances.insert("TokenB".to_string(), orderbook.balances.get("TokenB").copied().unwrap_or(0.0));

        let mut amm_manager = AMMManager::new(AMMConfig {
            p_min: 0.05,
            p_max: 0.95,
            spread: 0.01,
            delta: 0.01,
            depth: 0.1,
            max_collateral: 200.0,
        });
        let expected_orders = amm_manager.get_expected_orders(target_prices, &balances);
        let expected_order_types: HashSet<OrderType> = expected_orders
            .iter()
            .map(|order| OrderType::from_order(order))
            .collect();

        orders_to_cancel.extend(
            orderbook
                .orders
                .iter()
                .filter(|order| !expected_order_types.contains(&OrderType::from_order(order)))
                .cloned(),
        );

        for order_type in &expected_order_types {
            let open_orders: Vec<&Order> = orderbook
                .orders
                .iter()
                .filter(|order| OrderType::from_order(order) == *order_type)
                .collect();
            let open_size: f64 = open_orders.iter().map(|order| order.size).sum();
            let expected_size: f64 = expected_orders
                .iter()
                .filter(|order| OrderType::from_order(order) == *order_type)
                .map(|order| order.size)
                .sum();

            let new_size = if open_size > expected_size {
                orders_to_cancel.extend(open_orders.iter().cloned());
                expected_size
            } else {
                (expected_size - open_size) * 100.0 / 100.0
            };

            if new_size >= MIN_SIZE {
                orders_to_place.push(Order::new(
                    new_size,
                    order_type.price,
                    order_type.side,
                    order_type.token,
                    None,
                ));
            }
        }

        (orders_to_cancel, orders_to_place)
    }
}

