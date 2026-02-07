use crate::constants::MAX_DECIMALS;

pub fn math_round_down(f: f64, sig_digits: u32) -> f64 {
    let multiplier = 10_f64.powi(sig_digits as i32);
    (f * multiplier).floor() / multiplier
}

pub fn math_round_up(f: f64, sig_digits: u32) -> f64 {
    let multiplier = 10_f64.powi(sig_digits as i32);
    (f * multiplier).ceil() / multiplier
}

pub fn add_randomness(price: f64, lower: f64, upper: f64) -> f64 {
    math_round_down(price + rand::random::<f64>() * (upper - lower) + lower, MAX_DECIMALS)
}

pub fn randomize_default_price(price: f64) -> f64 {
    add_randomness(price, -0.1, 0.1)
}

