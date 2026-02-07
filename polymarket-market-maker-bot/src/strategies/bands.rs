use crate::constants::{MAX_DECIMALS, MIN_SIZE, MIN_TICK};
use crate::order::{Order, Side};
use crate::token::Token;
use crate::utils::math_round_down;

pub struct Band {
    min_margin: f64,
    avg_margin: f64,
    max_margin: f64,
    min_amount: f64,
    avg_amount: f64,
    max_amount: f64,
}

impl Band {
    pub fn new(
        min_margin: f64,
        avg_margin: f64,
        max_margin: f64,
        min_amount: f64,
        avg_amount: f64,
        max_amount: f64,
    ) -> Self {
        assert!(min_amount >= 0.0);
        assert!(avg_amount >= 0.0);
        assert!(max_amount >= 0.0);
        assert!(min_amount <= avg_amount);
        assert!(avg_amount <= max_amount);
        assert!(min_margin <= avg_margin);
        assert!(avg_margin <= max_margin);
        assert!(min_margin < max_margin);

        Self {
            min_margin,
            avg_margin,
            max_margin,
            min_amount,
            avg_amount,
            max_amount,
        }
    }

    pub fn excessive_orders(
        &self,
        orders: &[Order],
        target_price: f64,
        is_first_band: bool,
        is_last_band: bool,
    ) -> Vec<Order> {
        let orders_in_band: Vec<&Order> = orders
            .iter()
            .filter(|order| self.includes(order, target_price))
            .collect();
        let orders_total_size: f64 = orders_in_band.iter().map(|order| order.size).sum();

        let mut orders_in_band = orders_in_band.clone();
        if is_first_band {
            orders_in_band.sort_by(|a, b| {
                (b.price - target_price).abs().partial_cmp(&(a.price - target_price).abs()).unwrap()
            });
        } else if is_last_band {
            orders_in_band.sort_by(|a, b| {
                (a.price - target_price).abs().partial_cmp(&(b.price - target_price).abs()).unwrap()
            });
        } else {
            orders_in_band.sort_by(|a, b| a.size.partial_cmp(&b.size).unwrap());
        }

        let mut orders_for_cancellation = Vec::new();
        let mut band_amount: f64 = orders_in_band.iter().map(|order| order.size).sum();

        while band_amount > self.max_amount {
            if let Some(order) = orders_in_band.pop() {
                orders_for_cancellation.push((*order).clone());
                band_amount -= order.size;
            } else {
                break;
            }
        }

        orders_for_cancellation
    }

    pub fn includes(&self, order: &Order, target_price: f64) -> bool {
        let price = if order.side == Side::Buy {
            order.price
        } else {
            math_round_down(1.0 - order.price, MAX_DECIMALS)
        };

        price > self.min_price(target_price) && price <= self.max_price(target_price)
    }

    fn apply_margin(price: f64, margin: f64) -> f64 {
        math_round_down(price - margin, MAX_DECIMALS)
    }

    pub fn min_price(&self, target_price: f64) -> f64 {
        Self::apply_margin(target_price, self.max_margin)
    }

    pub fn buy_price(&self, target_price: f64) -> f64 {
        Self::apply_margin(target_price, self.avg_margin)
    }

    pub fn sell_price(&self, target_price: f64) -> f64 {
        Self::apply_margin(1.0 - target_price, -self.avg_margin)
    }

    pub fn max_price(&self, target_price: f64) -> f64 {
        Self::apply_margin(target_price, self.min_margin)
    }
}

pub struct Bands {
    bands: Vec<Band>,
}

impl Bands {
    pub fn new(bands_from_config: &[serde_json::Value]) -> Self {
        let bands: Vec<Band> = bands_from_config
            .iter()
            .map(|band| {
                Band::new(
                    band["minMargin"].as_f64().unwrap(),
                    band["avgMargin"].as_f64().unwrap(),
                    band["maxMargin"].as_f64().unwrap(),
                    band["minAmount"].as_f64().unwrap(),
                    band["avgAmount"].as_f64().unwrap(),
                    band["maxAmount"].as_f64().unwrap(),
                )
            })
            .collect();

        if Self::bands_overlap(&bands) {
            panic!("Bands in the config overlap!");
        }

        Self { bands }
    }

    fn calculate_virtual_bands(&self, target_price: f64) -> Vec<&Band> {
        if target_price <= 0.0 {
            return Vec::new();
        }

        self.bands
            .iter()
            .filter(|band| {
                if band.max_price(target_price) > 0.0 {
                    if band.buy_price(target_price) <= 0.0 {
                        // Would need to adjust avg_margin, but we can't mutate
                        // For now, just skip
                    }
                    true
                } else {
                    false
                }
            })
            .collect()
    }

    pub fn cancellable_orders(&self, orders: &[Order], target_price: f64) -> Vec<Order> {
        if target_price <= 0.0 {
            return orders.to_vec();
        }

        let virtual_bands = self.calculate_virtual_bands(target_price);
        let mut orders_to_cancel = Vec::new();

        for (idx, band) in virtual_bands.iter().enumerate() {
            let is_first = idx == 0;
            let is_last = idx == virtual_bands.len() - 1;
            orders_to_cancel.extend(band.excessive_orders(orders, target_price, is_first, is_last));
        }

        for order in orders {
            if !virtual_bands.iter().any(|band| band.includes(order, target_price)) {
                orders_to_cancel.push(order.clone());
            }
        }

        orders_to_cancel
    }

    pub fn new_orders(
        &self,
        orders: &[Order],
        collateral_balance: f64,
        token_balance: f64,
        target_price: f64,
        buy_token: Token,
    ) -> Vec<Order> {
        let sell_token = buy_token.complement();
        let mut new_orders = Vec::new();
        let mut free_collateral_balance = collateral_balance;

        for band in self.calculate_virtual_bands(target_price) {
            let band_amount: f64 = orders
                .iter()
                .filter(|order| band.includes(order, target_price))
                .map(|order| order.size)
                .sum();

            if band_amount < band.min_amount {
                let sell_price = band.sell_price(target_price);
                let sell_size = math_round_down(
                    (band.avg_amount - band_amount).min(token_balance),
                    MAX_DECIMALS,
                );

                if Self::new_order_is_valid(sell_price, sell_size) {
                    new_orders.push(Order::new(sell_size, sell_price, Side::Sell, sell_token, None));
                    free_collateral_balance -= sell_size * sell_price;
                }

                let buy_price = band.buy_price(target_price);
                let buy_size = math_round_down(
                    (band.avg_amount - band_amount).min(free_collateral_balance / buy_price),
                    MAX_DECIMALS,
                );

                if Self::new_order_is_valid(buy_price, buy_size) {
                    new_orders.push(Order::new(buy_size, buy_price, Side::Buy, buy_token, None));
                    free_collateral_balance -= buy_size * buy_price;
                }
            }
        }

        new_orders
    }

    fn new_order_is_valid(price: f64, size: f64) -> bool {
        price > 0.0 && price < 1.0 && size >= MIN_SIZE
    }

    fn bands_overlap(bands: &[Band]) -> bool {
        for (i, band1) in bands.iter().enumerate() {
            for (j, band2) in bands.iter().enumerate() {
                if i != j {
                    if band1.min_margin < band2.max_margin && band2.min_margin < band1.max_margin {
                        return true;
                    }
                }
            }
        }
        false
    }
}

