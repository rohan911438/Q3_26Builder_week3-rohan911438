//! Task 1: happy-path and basic validation coverage for the four core
//! instructions (initialize, add_liquidity, remove_liquidity, swap).
//! The full negative-case matrix (Task 3) lives in `task3_full_suite.rs`.

mod common;

use amm::error::AmmError;
use common::{expect_custom_error, PoolFixture};
use solana_signer::Signer;

#[test]
fn initialize_creates_pool_with_zero_reserves() {
    let fixture = PoolFixture::new();
    let pool = fixture.pool_state();

    assert_eq!(pool.mint_a, fixture.mint_a.pubkey());
    assert_eq!(pool.mint_b, fixture.mint_b.pubkey());
    assert_eq!(pool.vault_a, fixture.vault_a);
    assert_eq!(pool.vault_b, fixture.vault_b);
    assert_eq!(pool.reserve_a, 0);
    assert_eq!(pool.reserve_b, 0);
    assert_eq!(pool.total_shares, 0);
    assert_eq!(fixture.token_balance(&fixture.vault_a), 0);
    assert_eq!(fixture.token_balance(&fixture.vault_b), 0);
}

#[test]
fn add_liquidity_first_deposit_mints_sqrt_shares() {
    let mut fixture = PoolFixture::new();
    let (user, user_a, user_b) = fixture.user_with_tokens(10_000, 10_000);

    let res = fixture.add_liquidity(&user, user_a, user_b, 1_000, 2_000);
    assert!(res.is_ok(), "add_liquidity failed: {res:?}");

    // First deposit mints shares = floor(sqrt(1000 * 2000)) = floor(sqrt(2_000_000)) = 1414.
    let pool = fixture.pool_state();
    assert_eq!(pool.reserve_a, 1_000);
    assert_eq!(pool.reserve_b, 2_000);
    assert_eq!(pool.total_shares, 1_414);

    let position = fixture.position_state(&user.pubkey());
    assert_eq!(position.shares, 1_414);

    assert_eq!(fixture.token_balance(&user_a), 10_000 - 1_000);
    assert_eq!(fixture.token_balance(&user_b), 10_000 - 2_000);
    assert_eq!(fixture.token_balance(&fixture.vault_a), 1_000);
    assert_eq!(fixture.token_balance(&fixture.vault_b), 2_000);
}

#[test]
fn add_liquidity_second_deposit_is_proportional_to_the_pool() {
    let mut fixture = PoolFixture::new();
    let (first, first_a, first_b) = fixture.user_with_tokens(10_000, 10_000);
    fixture
        .add_liquidity(&first, first_a, first_b, 1_000, 2_000)
        .unwrap();

    let (second, second_a, second_b) = fixture.user_with_tokens(10_000, 10_000);
    // Same 1:2 ratio as the pool, so both sides are the limiting factor.
    let res = fixture.add_liquidity(&second, second_a, second_b, 500, 1_000);
    assert!(res.is_ok(), "add_liquidity failed: {res:?}");

    // shares = min(500 * 1414 / 1000, 1000 * 1414 / 2000) = 707
    let pool = fixture.pool_state();
    assert_eq!(pool.reserve_a, 1_500);
    assert_eq!(pool.reserve_b, 3_000);
    assert_eq!(pool.total_shares, 1_414 + 707);

    let position = fixture.position_state(&second.pubkey());
    assert_eq!(position.shares, 707);
}

#[test]
fn remove_liquidity_returns_proportional_reserves() {
    let mut fixture = PoolFixture::new();
    let (user, user_a, user_b) = fixture.user_with_tokens(10_000, 10_000);
    fixture
        .add_liquidity(&user, user_a, user_b, 1_000, 2_000)
        .unwrap();

    // Withdraw exactly half of the 1414 shares minted on deposit.
    let res = fixture.remove_liquidity(&user, user_a, user_b, 707);
    assert!(res.is_ok(), "remove_liquidity failed: {res:?}");

    // amount = reserve * shares / total_shares
    // a: 1000 * 707 / 1414 = 500, b: 2000 * 707 / 1414 = 1000
    let pool = fixture.pool_state();
    assert_eq!(pool.reserve_a, 500);
    assert_eq!(pool.reserve_b, 1_000);
    assert_eq!(pool.total_shares, 1_414 - 707);

    let position = fixture.position_state(&user.pubkey());
    assert_eq!(position.shares, 1_414 - 707);

    assert_eq!(fixture.token_balance(&user_a), 10_000 - 1_000 + 500);
    assert_eq!(fixture.token_balance(&user_b), 10_000 - 2_000 + 1_000);
}

#[test]
fn swap_a_to_b_follows_constant_product_formula() {
    let mut fixture = PoolFixture::new();
    let (lp, lp_a, lp_b) = fixture.user_with_tokens(10_000, 10_000);
    fixture.add_liquidity(&lp, lp_a, lp_b, 1_000, 2_000).unwrap();

    let (trader, trader_a, trader_b) = fixture.user_with_tokens(1_000, 0);
    let res = fixture.swap(&trader, trader_a, trader_b, 100, true);
    assert!(res.is_ok(), "swap failed: {res:?}");

    // k = 1000 * 2000 = 2_000_000; new_x = 1100; new_y = 2_000_000 / 1100 = 1818
    // amount_out = 2000 - 1818 = 182
    let pool = fixture.pool_state();
    assert_eq!(pool.reserve_a, 1_100);
    assert_eq!(pool.reserve_b, 1_818);

    assert_eq!(fixture.token_balance(&trader_a), 1_000 - 100);
    assert_eq!(fixture.token_balance(&trader_b), 182);
}

#[test]
fn swap_b_to_a_follows_constant_product_formula() {
    let mut fixture = PoolFixture::new();
    let (lp, lp_a, lp_b) = fixture.user_with_tokens(10_000, 10_000);
    fixture.add_liquidity(&lp, lp_a, lp_b, 1_000, 2_000).unwrap();

    let (trader, trader_a, trader_b) = fixture.user_with_tokens(0, 1_000);
    let res = fixture.swap(&trader, trader_a, trader_b, 200, false);
    assert!(res.is_ok(), "swap failed: {res:?}");

    // k = 2_000_000; new_y (reserve_b) = 2200; new_x = 2_000_000 / 2200 = 909
    // amount_out = 1000 - 909 = 91
    let pool = fixture.pool_state();
    assert_eq!(pool.reserve_b, 2_200);
    assert_eq!(pool.reserve_a, 909);

    assert_eq!(fixture.token_balance(&trader_b), 1_000 - 200);
    assert_eq!(fixture.token_balance(&trader_a), 91);
}

#[test]
fn add_liquidity_rejects_zero_amount() {
    let mut fixture = PoolFixture::new();
    let (user, user_a, user_b) = fixture.user_with_tokens(1_000, 1_000);

    let res = fixture.add_liquidity(&user, user_a, user_b, 0, 500);
    expect_custom_error(res, u32::from(AmmError::InvalidAmount));
}

#[test]
fn swap_rejects_zero_amount_in() {
    let mut fixture = PoolFixture::new();
    let (lp, lp_a, lp_b) = fixture.user_with_tokens(10_000, 10_000);
    fixture.add_liquidity(&lp, lp_a, lp_b, 1_000, 2_000).unwrap();

    let (trader, trader_a, trader_b) = fixture.user_with_tokens(1_000, 0);
    let res = fixture.swap(&trader, trader_a, trader_b, 0, true);
    expect_custom_error(res, u32::from(AmmError::InvalidAmount));
}
