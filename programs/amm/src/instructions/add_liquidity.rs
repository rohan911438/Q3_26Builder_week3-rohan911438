use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::{
    constants::{POOL_SEED, POSITION_SEED},
    error::AmmError,
    math::integer_sqrt,
    state::{Pool, Position},
};

#[derive(Accounts)]
pub struct AddLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [POOL_SEED, pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Box<Account<'info, Pool>>,

    #[account(
        init_if_needed,
        payer = user,
        space = 8 + Position::INIT_SPACE,
        seeds = [POSITION_SEED, pool.key().as_ref(), user.key().as_ref()],
        bump,
    )]
    pub position: Box<Account<'info, Position>>,

    #[account(mut, address = pool.vault_a)]
    pub vault_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = pool.vault_b)]
    pub vault_b: Box<Account<'info, TokenAccount>>,

    #[account(mut, token::mint = pool.mint_a, token::authority = user)]
    pub user_token_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = pool.mint_b, token::authority = user)]
    pub user_token_b: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handle_add_liquidity(
    ctx: Context<AddLiquidity>,
    amount_a: u64,
    amount_b: u64,
) -> Result<()> {
    require!(amount_a > 0 && amount_b > 0, AmmError::InvalidAmount);

    let pool = &ctx.accounts.pool;

    // How many shares this deposit is worth.
    let shares_minted: u64 = if pool.total_shares == 0 {
        // First deposit sets the initial price: shares = sqrt(a * b).
        // This is the standard Uniswap V2 bootstrap formula, chosen so that
        // the first depositor can't mint an arbitrarily large or small share
        // count just by picking a lopsided amount_a / amount_b.
        integer_sqrt((amount_a as u128) * (amount_b as u128))
            .try_into()
            .map_err(|_| AmmError::MathOverflow)?
    } else {
        // Later deposits mint shares proportional to the pool's current
        // ratio. Using the smaller of the two proportional amounts means a
        // deposit that doesn't match the pool ratio never mints more than
        // its fair share (the leftover value simply isn't credited).
        let shares_a = (amount_a as u128) * (pool.total_shares as u128) / (pool.reserve_a as u128);
        let shares_b = (amount_b as u128) * (pool.total_shares as u128) / (pool.reserve_b as u128);
        shares_a
            .min(shares_b)
            .try_into()
            .map_err(|_| AmmError::MathOverflow)?
    };
    require!(shares_minted > 0, AmmError::InvalidAmount);

    // Move both tokens from the user into the pool's vaults. If the user
    // doesn't have enough balance, the token program rejects the CPI and
    // this instruction fails automatically.
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.user_token_a.to_account_info(),
                to: ctx.accounts.vault_a.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount_a,
    )?;
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.user_token_b.to_account_info(),
                to: ctx.accounts.vault_b.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        amount_b,
    )?;

    let pool = &mut ctx.accounts.pool;
    pool.reserve_a = pool
        .reserve_a
        .checked_add(amount_a)
        .ok_or(AmmError::MathOverflow)?;
    pool.reserve_b = pool
        .reserve_b
        .checked_add(amount_b)
        .ok_or(AmmError::MathOverflow)?;
    pool.total_shares = pool
        .total_shares
        .checked_add(shares_minted)
        .ok_or(AmmError::MathOverflow)?;

    let position = &mut ctx.accounts.position;
    position.pool = pool.key();
    position.owner = ctx.accounts.user.key();
    position.shares = position
        .shares
        .checked_add(shares_minted)
        .ok_or(AmmError::MathOverflow)?;
    position.bump = ctx.bumps.position;

    Ok(())
}
