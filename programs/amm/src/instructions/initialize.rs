use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

use crate::{constants::POOL_SEED, error::AmmError, state::Pool};

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub mint_a: Box<Account<'info, Mint>>,
    pub mint_b: Box<Account<'info, Mint>>,

    /// Anchor runs every `init` account's checks before any plain
    /// `constraint = ...` on a non-init field (regardless of field order),
    /// so the mint-equality check has to live on the *first* `init` field
    /// (this one) to actually run before `vault_a`/`vault_b` below — if the
    /// mints were equal, those would resolve to the same associated-token
    /// address and the second `init` would fail with a confusing low-level
    /// error instead of this one.
    #[account(
        init,
        payer = payer,
        space = 8 + Pool::INIT_SPACE,
        seeds = [POOL_SEED, mint_a.key().as_ref(), mint_b.key().as_ref()],
        bump,
        constraint = mint_a.key() != mint_b.key() @ AmmError::IdenticalMints,
    )]
    pub pool: Box<Account<'info, Pool>>,

    /// Pool-owned vault that will hold all of the pool's token A.
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint_a,
        associated_token::authority = pool,
    )]
    pub vault_a: Box<Account<'info, TokenAccount>>,

    /// Pool-owned vault that will hold all of the pool's token B.
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint_b,
        associated_token::authority = pool,
    )]
    pub vault_b: Box<Account<'info, TokenAccount>>,

    /// Wallet designated to receive protocol fees. Just a plain wallet
    /// (system-account owned), not a signer — anyone can pay to set up a
    /// pool, but only this wallet's own vaults can ever receive its fees.
    pub treasury: SystemAccount<'info>,

    /// Treasury's token A account, created and owned by `treasury`.
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint_a,
        associated_token::authority = treasury,
    )]
    pub treasury_vault_a: Box<Account<'info, TokenAccount>>,

    /// Treasury's token B account, created and owned by `treasury`.
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint_b,
        associated_token::authority = treasury,
    )]
    pub treasury_vault_b: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn handle_initialize(ctx: Context<InitializePool>) -> Result<()> {
    let pool = &mut ctx.accounts.pool;
    pool.mint_a = ctx.accounts.mint_a.key();
    pool.mint_b = ctx.accounts.mint_b.key();
    pool.vault_a = ctx.accounts.vault_a.key();
    pool.vault_b = ctx.accounts.vault_b.key();
    pool.treasury = ctx.accounts.treasury.key();
    pool.treasury_vault_a = ctx.accounts.treasury_vault_a.key();
    pool.treasury_vault_b = ctx.accounts.treasury_vault_b.key();
    pool.reserve_a = 0;
    pool.reserve_b = 0;
    pool.total_shares = 0;
    pool.bump = ctx.bumps.pool;

    Ok(())
}
