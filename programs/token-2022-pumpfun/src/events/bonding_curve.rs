use anchor_lang::prelude::*;

#[event]
pub struct BondingCurveCompleted {
    pub mintAddr: Pubkey,
    pub userAta: Pubkey,
    pub solPool: Pubkey,
    pub tokenPool: Pubkey,
}
