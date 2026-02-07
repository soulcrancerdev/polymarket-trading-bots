use crate::constants::MAX_DECIMALS;
use crate::order::{Order, Side};
use crate::token::{Token, COLLATERAL};
use crate::utils::math_round_down;

pub struct AMMConfig {
    pub p_min: f64,
    pub p_max: f64,
    pub spread: f64,
    pub delta: f64,
    pub depth: f64,
    pub max_collateral: f64,
}

pub struct AMM {
    token: Token,
    p_min: f64,
    p_max: f64,
    delta: f64,
    spread: f64,
    depth: f64,
    max_collateral: f64,
    p_i: Option<f64>,
    p_u: Option<f64>,
    p_l: Option<f64>,
    buy_prices: Vec<f64>,
    sell_prices: Vec<f64>,
}

impl AMM {
    pub fn new(token: Token, config: &AMMConfig) -> Self {
        if config.spread >= config.depth {
            panic!("Depth does not exceed spread.");
        }

        Self {
            token,
            p_min: config.p_min,
            p_max: config.p_max,
            delta: config.delta,
            spread: config.spread,
            depth: config.depth,
            max_collateral: config.max_collateral,
            p_i: None,
            p_u: None,
            p_l: None,
            buy_prices: Vec::new(),
            sell_prices: Vec::new(),
        }
    }

    pub fn set_price(&mut self, p_i: f64) {
        self.p_i = Some(p_i);
        self.p_u = Some((p_i + self.depth).min(self.p_max));
        self.p_l = Some((p_i - self.depth).max(self.p_min));

        let p_i = self.p_i.unwrap();
        let p_u = self.p_u.unwrap();
        let p_l = self.p_l.unwrap();

        self.buy_prices.clear();
        let mut price = ((p_i - self.spread) * 100.0).round() / 100.0;
        while price >= p_l {
            self.buy_prices.push(price);
            price = ((price - self.delta) * 100.0).round() / 100.0;
        }

        self.sell_prices.clear();
        let mut price = ((p_i + self.spread) * 100.0).round() / 100.0;
        while price <= p_u {
            self.sell_prices.push(price);
            price = ((price + self.delta) * 100.0).round() / 100.0;
        }
    }

    pub fn get_sell_orders(&self, x: f64) -> Vec<Order> {
        let sizes: Vec<f64> = self
            .sell_prices
            .iter()
            .map(|&p_t| self.sell_size(x, p_t))
            .collect();
        let sizes = Self::diff(&sizes);
        let sizes: Vec<f64> = sizes
            .iter()
            .map(|&size| math_round_down(size, MAX_DECIMALS))
            .collect();

        self.sell_prices
            .iter()
            .zip(sizes.iter())
            .map(|(&price, &size)| Order::new(size, price, Side::Sell, self.token, None))
            .collect()
    }

    pub fn get_buy_orders(&self, y: f64) -> Vec<Order> {
        let sizes: Vec<f64> = self
            .buy_prices
            .iter()
            .map(|&p_t| self.buy_size(y, p_t))
            .collect();
        let sizes = Self::diff(&sizes);
        let sizes: Vec<f64> = sizes
            .iter()
            .map(|&size| math_round_down(size, MAX_DECIMALS))
            .collect();

        self.buy_prices
            .iter()
            .zip(sizes.iter())
            .map(|(&price, &size)| Order::new(size, price, Side::Buy, self.token, None))
            .collect()
    }

    pub fn phi(&self) -> f64 {
        let p_i = self.p_i.unwrap();
        let p_l = self.p_l.unwrap();
        let first_buy_price = self.buy_prices[0];
        (1.0 / (p_i.sqrt() - p_l.sqrt())) * (1.0 / first_buy_price.sqrt() - 1.0 / p_i.sqrt())
    }

    pub fn sell_size(&self, x: f64, p_t: f64) -> f64 {
        Self::_sell_size(x, self.p_i.unwrap(), p_t, self.p_u.unwrap())
    }

    fn _sell_size(x: f64, p_i: f64, p_t: f64, p_u: f64) -> f64 {
        let l = x / (1.0 / p_i.sqrt() - 1.0 / p_u.sqrt());
        l / p_u.sqrt() - l / p_t.sqrt() + x
    }

    pub fn buy_size(&self, y: f64, p_t: f64) -> f64 {
        Self::_buy_size(y, self.p_i.unwrap(), p_t, self.p_l.unwrap())
    }

    fn _buy_size(y: f64, p_i: f64, p_t: f64, p_l: f64) -> f64 {
        let l = y / (p_i.sqrt() - p_l.sqrt());
        l * (1.0 / p_t.sqrt() - 1.0 / p_i.sqrt())
    }

    fn diff(arr: &[f64]) -> Vec<f64> {
        arr.iter()
            .enumerate()
            .map(|(i, &val)| if i == 0 { val } else { val - arr[i - 1] })
            .collect()
    }
}

pub struct AMMManager {
    amm_a: AMM,
    amm_b: AMM,
    max_collateral: f64,
}

impl AMMManager {
    pub fn new(config: AMMConfig) -> Self {
        Self {
            amm_a: AMM::new(Token::A, &config),
            amm_b: AMM::new(Token::B, &config),
            max_collateral: config.max_collateral,
        }
    }

    pub fn get_expected_orders(
        &mut self,
        target_prices: &std::collections::HashMap<Token, f64>,
        balances: &std::collections::HashMap<String, f64>,
    ) -> Vec<Order> {
        self.amm_a.set_price(*target_prices.get(&Token::A).unwrap());
        self.amm_b.set_price(*target_prices.get(&Token::B).unwrap());

        let sell_orders_a = self.amm_a.get_sell_orders(
            *balances.get(&Token::A.value().to_string()).unwrap_or(&0.0),
        );
        let sell_orders_b = self.amm_b.get_sell_orders(
            *balances.get(&Token::B.value().to_string()).unwrap_or(&0.0),
        );

        let best_sell_order_size_a = sell_orders_a.first().map(|o| o.size).unwrap_or(0.0);
        let best_sell_order_size_b = sell_orders_b.first().map(|o| o.size).unwrap_or(0.0);

        let total_collateral_allocation = balances
            .get(COLLATERAL)
            .copied()
            .unwrap_or(0.0)
            .min(self.max_collateral);

        let (collateral_allocation_a, collateral_allocation_b) = self.collateral_allocation(
            total_collateral_allocation,
            best_sell_order_size_a,
            best_sell_order_size_b,
        );

        let buy_orders_a = self.amm_a.get_buy_orders(collateral_allocation_a);
        let buy_orders_b = self.amm_b.get_buy_orders(collateral_allocation_b);

        let mut orders = Vec::new();
        orders.extend(sell_orders_a);
        orders.extend(sell_orders_b);
        orders.extend(buy_orders_a);
        orders.extend(buy_orders_b);

        orders
    }

    fn collateral_allocation(
        &self,
        collateral_balance: f64,
        best_sell_order_size_a: f64,
        best_sell_order_size_b: f64,
    ) -> (f64, f64) {
        let phi_a = self.amm_a.phi();
        let phi_b = self.amm_b.phi();

        let mut collateral_allocation_a = (best_sell_order_size_a - best_sell_order_size_b
            + collateral_balance * phi_b)
            / (phi_a + phi_b);

        if collateral_allocation_a < 0.0 {
            collateral_allocation_a = 0.0;
        } else if collateral_allocation_a > collateral_balance {
            collateral_allocation_a = collateral_balance;
        }

        let collateral_allocation_b = collateral_balance - collateral_allocation_a;

        (
            math_round_down(collateral_allocation_a, MAX_DECIMALS),
            math_round_down(collateral_allocation_b, MAX_DECIMALS),
        )
    }
}

