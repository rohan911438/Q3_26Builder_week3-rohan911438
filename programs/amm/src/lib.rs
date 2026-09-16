pub mod constants;
pub mod error;
pub mod instructions;
pub mod math;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("9WLYv6SQgcgpta6GiqXUQrKg5yxUy1ir4obN5ELdyGAW");

#[program]
pub mod amm {
    use super::*;

    /// Create a new pool for the (mint_a, mint_b) pair with empty reserves.
    pub fn initialize(ctx: Context<InitializePool>) -> Result<()> {
        instructions::initialize::handle_initialize(ctx)
    }

    /// Deposit token A and token B into the pool and receive liquidity shares.
    pub fn add_liquidity(ctx: Context<AddLiquidity>, amount_a: u64, amount_b: u64) -> Result<()> {
        instructions::add_liquidity::handle_add_liquidity(ctx, amount_a, amount_b)
    }

    /// Burn liquidity shares and withdraw a proportional share of both reserves.
    pub fn remove_liquidity(ctx: Context<RemoveLiquidity>, shares: u64) -> Result<()> {
        instructions::remove_liquidity::handle_remove_liquidity(ctx, shares)
    }

    /// Swap `amount_in` of one token for the other along the constant-product curve.
    /// `a_to_b = true` swaps token A -> token B, `false` swaps token B -> token A.
    pub fn swap(ctx: Context<Swap>, amount_in: u64, a_to_b: bool) -> Result<()> {
        instructions::swap::handle_swap(ctx, amount_in, a_to_b)
    }
}
