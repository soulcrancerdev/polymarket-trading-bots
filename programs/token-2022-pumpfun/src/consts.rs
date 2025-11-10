use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone)]
pub struct InitializeConfigurationParam {
    pub swap_fee : f32,                                       //  swap percentage
    pub bonding_curve_limitation : u64,                       //  bonding curve limitation
    pub initial_virtual_sol : u64,          //  sol percentage which moves to pumpfun after bonding curve 
    pub initial_virtual_token : u64,          //  sol percentage which moves to pumpfun after bonding curve 
    pub create_pool_fee_lamports : u64,          //  sol percentage which moves to pumpfun after bonding curve 
}

pub const FEE_SEED : &'static [u8] = b"pumpfun_fee";