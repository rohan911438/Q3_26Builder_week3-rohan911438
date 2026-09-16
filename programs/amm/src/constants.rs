/// Seed for the pool PDA: `[POOL_SEED, mint_a, mint_b]`.
pub const POOL_SEED: &[u8] = b"pool";

/// Seed for a liquidity provider's position PDA: `[POSITION_SEED, pool, owner]`.
pub const POSITION_SEED: &[u8] = b"position";

/// Swap fee, expressed in basis points (1 bps = 0.01%).
///
/// `FEE_BPS = 30` means a 0.30% fee, i.e. `fee = amount_in * 30 / 10_000`.
/// This is a fixed, protocol-wide fee (not configurable per pool) — simple
/// to reason about and to test.
pub const FEE_BPS: u64 = 30;

/// Denominator that `FEE_BPS` is measured against (10_000 bps = 100%).
pub const FEE_DENOMINATOR: u64 = 10_000;
