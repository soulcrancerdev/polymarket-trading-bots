use std::ops::{Div, Mul};

use anchor_lang::{prelude::msg, solana_program::native_token::LAMPORTS_PER_SOL};

pub fn slippage_calc(expected: u64, executed: u64) -> f32 {
    msg!("quote {} , swap {}  ", expected, executed);

    let percentage = ((executed).abs_diff(expected) as f32)
        .mul(100.0)
        .div(expected as f32);

    msg!("slippage : {} %", percentage);

    percentage
}
