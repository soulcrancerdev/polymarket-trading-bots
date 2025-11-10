use anchor_lang::prelude::*;

#[error_code]
pub enum RaydiumPumpfunError {
    #[msg("Mismatching Values")]
    MissMatchingValue,
    #[msg("Slippage Error")]
    SlippageExcceded,
}
