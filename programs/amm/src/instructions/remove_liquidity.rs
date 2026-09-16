use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::{
    constants::{POOL_SEED, POSITION_SEED},
    error::AmmError,
    state::{Pool, Position},
};

#[derive(Accounts)]
pub struct RemoveLiquidity<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [POOL_SEED, pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Box<Account<'info, Pool>>,

    #[account(
        mut,
        seeds = [POSITION_SEED, pool.key().as_ref(), user.key().as_ref()],
        bump = position.bump,
        has_one = pool,
        constraint = position.owner == user.key() @ AmmError::InsufficientShares,
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
}

pub fn handle_remove_liquidity(ctx: Context<RemoveLiquidity>, shares: u64) -> Result<()> {
    require!(shares > 0, AmmError::InvalidAmount);

    let position = &ctx.accounts.position;
    require!(position.shares >= shares, AmmError::InsufficientShares);

    let pool = &ctx.accounts.pool;
    require!(pool.total_shares > 0, AmmError::InsufficientLiquidity);

    // Withdraw the same fraction of both reserves as the fraction of total
    // shares being burned: amount = reserve * shares / total_shares.
    let amount_a: u64 = ((pool.reserve_a as u128) * (shares as u128) / (pool.total_shares as u128))
        .try_into()
        .map_err(|_| AmmError::MathOverflow)?;
    let amount_b: u64 = ((pool.reserve_b as u128) * (shares as u128) / (pool.total_shares as u128))
        .try_into()
        .map_err(|_| AmmError::MathOverflow)?;
    require!(amount_a > 0 && amount_b > 0, AmmError::InvalidAmount);
    require!(
        pool.reserve_a >= amount_a && pool.reserve_b >= amount_b,
        AmmError::InsufficientLiquidity
    );

    let mint_a = pool.mint_a;
    let mint_b = pool.mint_b;
    let bump = pool.bump;
    let pool_signer_seeds: &[&[u8]] = &[POOL_SEED, mint_a.as_ref(), mint_b.as_ref(), &[bump]];

    // Vaults are owned by the pool PDA, so the pool must sign for the CPI.
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.vault_a.to_account_info(),
                to: ctx.accounts.user_token_a.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            &[pool_signer_seeds],
        ),
        amount_a,
    )?;
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            Transfer {
                from: ctx.accounts.vault_b.to_account_info(),
                to: ctx.accounts.user_token_b.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            &[pool_signer_seeds],
        ),
        amount_b,
    )?;

    let pool = &mut ctx.accounts.pool;
    pool.reserve_a = pool
        .reserve_a
        .checked_sub(amount_a)
        .ok_or(AmmError::MathOverflow)?;
    pool.reserve_b = pool
        .reserve_b
        .checked_sub(amount_b)
        .ok_or(AmmError::MathOverflow)?;
    pool.total_shares = pool
        .total_shares
        .checked_sub(shares)
        .ok_or(AmmError::MathOverflow)?;

    let position = &mut ctx.accounts.position;
    position.shares = position
        .shares
        .checked_sub(shares)
        .ok_or(AmmError::MathOverflow)?;

    Ok(())
}
