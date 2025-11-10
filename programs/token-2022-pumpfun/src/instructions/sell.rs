use std::ops::{Div, Mul};

use anchor_lang::{
    prelude::*,
    solana_program::{native_token::LAMPORTS_PER_SOL, system_instruction},
};
use anchor_spl::{
    associated_token::AssociatedToken,
    token_interface::{Mint, TokenAccount, TokenInterface , transfer_checked, TransferChecked},
};


use crate::{
    errors::RaydiumPumpfunError,
    events::BondingCurveCompleted,
    states::{BondingCurve, InitializeConfiguration},
    utils::{calc_swap_quote, slippage_calc},
};


#[derive(Accounts)]
pub struct Sell<'info> {
    #[account(
        seeds = [ b"global_config"],
        bump
    )]
    pub global_configuration: Account<'info, InitializeConfiguration>,

    #[account(
        mut,
        seeds = [ &mint_addr.key().to_bytes() , BondingCurve::POOL_SEED_PREFIX ],
        bump
    )]
    pub bonding_curve: Account<'info, BondingCurve>,

    pub mint_addr: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        associated_token::mint = mint_addr,
        associated_token::authority = payer,
        associated_token::token_program = token_program,
    )]
    pub user_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK:
    #[account(
        mut,
        seeds = [ &mint_addr.key().to_bytes() , b"sol_pool".as_ref() ],
        bump
    )]
    pub sol_pool: AccountInfo<'info>,

    #[account(
        mut,
        associated_token::mint = mint_addr,
        associated_token::authority = sol_pool,
        associated_token::token_program = token_program,
    )]
    pub token_pool: Box<InterfaceAccount<'info, TokenAccount>>,

    /// CHECK:
    #[account(mut)]
    pub fee_account: AccountInfo<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

impl<'info> Sell<'info> {
    pub fn process(&mut self, in_token_amount: u64, expected_amt : u64 , slippage : f32 , bump: u8) -> Result<()> {

        let estimated_out_sol = (( in_token_amount as f32 ).mul( self.bonding_curve.price )) as u64;

        let calc_slippage = slippage_calc(expected_amt.clone(), estimated_out_sol);

        if slippage < calc_slippage {
            err!(RaydiumPumpfunError::SlippageExcceded).unwrap()
        }

        msg!(
            "Sell : {} Token => {} Sol ( Price : {} )",
            in_token_amount.clone(),
            estimated_out_sol,
            self.bonding_curve.price
        );

        msg!(
            "Sell : {} token => {} sol ",
            in_token_amount,
            estimated_out_sol
        );

        self.bonding_curve.sol_reserves -= estimated_out_sol;
        self.bonding_curve.token_reserves += in_token_amount;

        self.bonding_curve.update();

        Ok(())
    }
}
