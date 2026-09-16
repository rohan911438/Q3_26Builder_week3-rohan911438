use anchor_lang::prelude::*;

/// A single constant-product pool for one token pair (mint_a / mint_b).
///
/// `reserve_a` / `reserve_b` are the AMM's bookkeeping of how many tokens
/// are in `vault_a` / `vault_b`. They are updated explicitly by every
/// instruction rather than re-read from the vaults, so the math in each
/// instruction is easy to follow directly from the state fields.
#[account]
#[derive(InitSpace)]
pub struct Pool {
    /// Mint of token A.
    pub mint_a: Pubkey,
    /// Mint of token B.
    pub mint_b: Pubkey,
    /// Token account holding the pool's token A, owned by this pool PDA.
    pub vault_a: Pubkey,
    /// Token account holding the pool's token B, owned by this pool PDA.
    pub vault_b: Pubkey,
    /// Wallet that owns the treasury vaults and receives protocol fees.
    pub treasury: Pubkey,
    /// Treasury's token A account — receives the token A side of swap fees.
    pub treasury_vault_a: Pubkey,
    /// Treasury's token B account — receives the token B side of swap fees.
    pub treasury_vault_b: Pubkey,
    /// Current token A reserve.
    pub reserve_a: u64,
    /// Current token B reserve.
    pub reserve_b: u64,
    /// Total liquidity shares minted across all providers.
    pub total_shares: u64,
    /// Bump for the pool PDA.
    pub bump: u8,
}

/// One liquidity provider's share of a single pool.
///
/// Shares are custom bookkeeping (not an SPL mint) — a deliberately simple
/// way to track "what fraction of the pool does this user own" without
/// adding a second token (the LP token) to manage.
#[account]
#[derive(InitSpace)]
pub struct Position {
    /// Pool this position belongs to.
    pub pool: Pubkey,
    /// Owner of this position.
    pub owner: Pubkey,
    /// Liquidity shares owned by this user.
    pub shares: u64,
    /// Bump for the position PDA.
    pub bump: u8,
}
