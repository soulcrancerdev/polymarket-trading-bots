use anchor_lang::prelude::*;

pub mod consts;
pub mod errors;
pub mod events;
pub mod instructions;
pub mod states;
pub mod utils;

use crate::consts::*;
use crate::instructions::*;

declare_id!("E2QG2JRMTgvVo1h9so92odibhzWH1N56vESymALn5s1w");

#[program]
pub mod token_2022_pumpfun {

    use super::*;

    pub fn initialize(ctx: Context<Initialize>, param: InitializeConfigurationParam) -> Result<()> {
        ctx.accounts.process(param);
        Ok(())
    }

    pub fn create_pool(ctx: Context<CreatePool>) -> Result<()> {
        ctx.accounts.process();
        Ok(())
    }

    pub fn buy(ctx: Context<Buy>, in_amount: u64, expected_amt: u64, slippage: f32) -> Result<()> {
        ctx.accounts
            .process(in_amount, expected_amt, slippage, ctx.bumps.sol_pool);
        Ok(())
    }

    pub fn sell(
        ctx: Context<Sell>,
        in_amount: u64,
        expected_amt: u64,
        slippage: f32,
    ) -> Result<()> {
        ctx.accounts
            .process(in_amount, expected_amt, slippage, ctx.bumps.sol_pool);
        Ok(())
    }
}
