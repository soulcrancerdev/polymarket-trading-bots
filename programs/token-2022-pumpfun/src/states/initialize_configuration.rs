use anchor_lang::prelude::*;

use crate::InitializeConfigurationParam;

#[account]
pub struct InitializeConfiguration {
    pub swap_fee : f32,                                         //  swap percentage
    pub bonding_curve_limitation : u64,                         //  bonding curve limitation
    pub initial_virtual_sol : u64,                              //  sol percentage which moves to pumpfun after bonding curve 
    pub initial_virtual_token : u64,                              //  sol percentage which moves to pumpfun after bonding curve 
    pub create_pool_fee_lamports : u64,                              //  sol percentage which moves to pumpfun after bonding curve 
}

impl InitializeConfiguration {
    pub const SIZE: usize = 36;  // Size of the struct
    pub const SEEDS: &'static [u8] = b"global_config";  // Size of the struct

    pub fn set_value (&mut self , param : InitializeConfigurationParam) -> Result<()> {

        self.bonding_curve_limitation =  param.bonding_curve_limitation;
        self.swap_fee =  param.swap_fee;
        self.initial_virtual_sol =  param.initial_virtual_sol;
        self.initial_virtual_token =  param.initial_virtual_token;
        self.create_pool_fee_lamports =  param.create_pool_fee_lamports;

        Ok(())
    }
}