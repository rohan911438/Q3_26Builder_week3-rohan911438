use anchor_lang::prelude::*;

#[error_code]
pub enum AmmError {
    #[msg("Amount must be greater than zero")]
    InvalidAmount,
    #[msg("Pool must be initialized with two different mints")]
    IdenticalMints,
    #[msg("Pool does not have enough liquidity for this operation")]
    InsufficientLiquidity,
    #[msg("Position does not have enough shares for this withdrawal")]
    InsufficientShares,
    #[msg("Calculation overflowed")]
    MathOverflow,
}
