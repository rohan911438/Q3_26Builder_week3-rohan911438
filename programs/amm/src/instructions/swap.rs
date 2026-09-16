use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

use crate::{
    constants::{FEE_BPS, FEE_DENOMINATOR, POOL_SEED},
    error::AmmError,
    state::Pool,
};

#[derive(Accounts)]
pub struct Swap<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        seeds = [POOL_SEED, pool.mint_a.as_ref(), pool.mint_b.as_ref()],
        bump = pool.bump,
    )]
    pub pool: Box<Account<'info, Pool>>,

    #[account(mut, address = pool.vault_a)]
    pub vault_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = pool.vault_b)]
    pub vault_b: Box<Account<'info, TokenAccount>>,

    /// Must be exactly the treasury vault recorded on `pool` — this is what
    /// stops a caller from redirecting fees to an arbitrary account.
    #[account(mut, address = pool.treasury_vault_a)]
    pub treasury_vault_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = pool.treasury_vault_b)]
    pub treasury_vault_b: Box<Account<'info, TokenAccount>>,

    #[account(mut, token::mint = pool.mint_a, token::authority = user)]
    pub user_token_a: Box<Account<'info, TokenAccount>>,
    #[account(mut, token::mint = pool.mint_b, token::authority = user)]
    pub user_token_b: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
}

/// Constant-product swap math with a protocol fee:
///
///   fee             = amount_in * FEE_BPS / FEE_DENOMINATOR
///   effective_input = amount_in - fee
///
///   k = reserve_in * reserve_out
///   new_reserve_in  = reserve_in + effective_input
///   new_reserve_out = k / new_reserve_in          (floor division)
///   amount_out      = reserve_out - new_reserve_out
///
/// Only `effective_input` ever enters the pool's reserves; `fee` is
/// transferred straight to the treasury and never affects `k`. Flooring
/// `new_reserve_out` means `amount_out` is rounded down, so the pool never
/// pays out more than the invariant allows.
pub fn handle_swap(ctx: Context<Swap>, amount_in: u64, a_to_b: bool) -> Result<()> {
    require!(amount_in > 0, AmmError::InvalidAmount);

    let pool = &ctx.accounts.pool;
    require!(
        pool.reserve_a > 0 && pool.reserve_b > 0,
        AmmError::InsufficientLiquidity
    );

    let fee: u64 = ((amount_in as u128) * (FEE_BPS as u128) / (FEE_DENOMINATOR as u128))
        .try_into()
        .map_err(|_| AmmError::MathOverflow)?;
    let effective_input = amount_in.checked_sub(fee).ok_or(AmmError::MathOverflow)?;
    require!(effective_input > 0, AmmError::InvalidAmount);

    let (reserve_in, reserve_out) = if a_to_b {
        (pool.reserve_a, pool.reserve_b)
    } else {
        (pool.reserve_b, pool.reserve_a)
    };

    let k = (reserve_in as u128) * (reserve_out as u128);
    let new_reserve_in = (reserve_in as u128) + (effective_input as u128);
    let new_reserve_out = k / new_reserve_in;
    let amount_out: u64 = ((reserve_out as u128) - new_reserve_out)
        .try_into()
        .map_err(|_| AmmError::MathOverflow)?;

    require!(amount_out > 0, AmmError::InvalidAmount);
    require!(amount_out < reserve_out, AmmError::InsufficientLiquidity);

    let (in_from, in_to, treasury_to, out_from, out_to) = if a_to_b {
        (
            &ctx.accounts.user_token_a,
            &ctx.accounts.vault_a,
            &ctx.accounts.treasury_vault_a,
            &ctx.accounts.vault_b,
            &ctx.accounts.user_token_b,
        )
    } else {
        (
            &ctx.accounts.user_token_b,
            &ctx.accounts.vault_b,
            &ctx.accounts.treasury_vault_b,
            &ctx.accounts.vault_a,
            &ctx.accounts.user_token_a,
        )
    };

    // User -> treasury: the fee, taken out of amount_in before anything
    // touches the pool's reserves.
    if fee > 0 {
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                Transfer {
                    from: in_from.to_account_info(),
                    to: treasury_to.to_account_info(),
                    authority: ctx.accounts.user.to_account_info(),
                },
            ),
            fee,
        )?;
    }

    // User -> pool: the remaining input, which is what the constant-product
    // calculation above is based on.
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            Transfer {
                from: in_from.to_account_info(),
                to: in_to.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        effective_input,
    )?;

    let mint_a = pool.mint_a;
    let mint_b = pool.mint_b;
    let bump = pool.bump;
    let pool_signer_seeds: &[&[u8]] = &[POOL_SEED, mint_a.as_ref(), mint_b.as_ref(), &[bump]];

    // Pool -> user: the output token, signed by the pool PDA.
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            Transfer {
                from: out_from.to_account_info(),
                to: out_to.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            &[pool_signer_seeds],
        ),
        amount_out,
    )?;

    let pool = &mut ctx.accounts.pool;
    if a_to_b {
        pool.reserve_a = new_reserve_in.try_into().map_err(|_| AmmError::MathOverflow)?;
        pool.reserve_b = new_reserve_out.try_into().map_err(|_| AmmError::MathOverflow)?;
    } else {
        pool.reserve_b = new_reserve_in.try_into().map_err(|_| AmmError::MathOverflow)?;
        pool.reserve_a = new_reserve_out.try_into().map_err(|_| AmmError::MathOverflow)?;
    }

    Ok(())
}
