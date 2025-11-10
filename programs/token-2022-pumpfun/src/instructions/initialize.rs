use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

use crate::{
    consts::InitializeConfigurationParam, errors::RaydiumPumpfunError,
    states::InitializeConfiguration, FEE_SEED,
};

#[derive(Accounts)]
#[instruction(params : InitializeConfigurationParam)]
pub struct Initialize<'info> {
    #[account(
        init,
        seeds = [ b"global_config"],
        payer = payer,
        space = 8 + 36,
        bump
    )]
    pub global_configuration: Account<'info, InitializeConfiguration>,

    /// CHECK:
    pub fee_account: AccountInfo<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

impl<'info> Initialize<'info> {
    pub fn process(&mut self, param: InitializeConfigurationParam) -> Result<()> {

        msg!("Global State : {:#?}" , param);

        self.global_configuration.set_value(param);

        Ok(())
    }
}
