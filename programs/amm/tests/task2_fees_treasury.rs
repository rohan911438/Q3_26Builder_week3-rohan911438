//! Task 2: fee calculation and treasury handling on top of the swap
//! instruction. Full negative-case coverage lives in `task3_full_suite.rs`.

mod common;

use anchor_lang::error::ErrorCode as AnchorErrorCode;
use common::{expect_custom_error, PoolFixture};

/// amount_in large enough that `amount_in * FEE_BPS / FEE_DENOMINATOR` is
/// non-zero (FEE_BPS = 30 means anything under ~334 rounds the fee to 0).
const SWAP_AMOUNT: u64 = 10_000;

fn expected_fee(amount_in: u64) -> u64 {
    amount_in * amm::FEE_BPS / amm::FEE_DENOMINATOR
}

#[test]
fn fee_constants_match_the_documented_30_bps() {
    // fee = amount_in * 30 / 10_000, i.e. 0.30%.
    assert_eq!(amm::FEE_BPS, 30);
    assert_eq!(amm::FEE_DENOMINATOR, 10_000);
    assert_eq!(expected_fee(SWAP_AMOUNT), 30);
    assert_eq!(expected_fee(10), 0); // rounds down to zero for tiny swaps
}

#[test]
fn swap_a_to_b_sends_the_fee_to_the_treasury() {
    let mut fixture = PoolFixture::new();
    let (lp, lp_a, lp_b) = fixture.user_with_tokens(1_000_000, 1_000_000);
    fixture
        .add_liquidity(&lp, lp_a, lp_b, 100_000, 200_000)
        .unwrap();

    assert_eq!(fixture.token_balance(&fixture.treasury_vault_a), 0);

    let (trader, trader_a, trader_b) = fixture.user_with_tokens(SWAP_AMOUNT, 0);
    let res = fixture.swap(&trader, trader_a, trader_b, SWAP_AMOUNT, true);
    assert!(res.is_ok(), "swap failed: {res:?}");

    let fee = expected_fee(SWAP_AMOUNT);
    let effective_input = SWAP_AMOUNT - fee;

    // treasury_before (0) + fee == treasury_after
    assert_eq!(fixture.token_balance(&fixture.treasury_vault_a), fee);
    // The user paid amount_in in total: fee + effective_input == SWAP_AMOUNT,
    // so their whole starting balance is gone.
    assert_eq!(fee + effective_input, SWAP_AMOUNT);
    assert_eq!(fixture.token_balance(&trader_a), 0);

    // Only effective_input ever reaches the pool's reserve.
    let pool = fixture.pool_state();
    assert_eq!(pool.reserve_a, 100_000 + effective_input);
}

#[test]
fn swap_b_to_a_sends_the_fee_to_the_treasury() {
    let mut fixture = PoolFixture::new();
    let (lp, lp_a, lp_b) = fixture.user_with_tokens(1_000_000, 1_000_000);
    fixture
        .add_liquidity(&lp, lp_a, lp_b, 100_000, 200_000)
        .unwrap();

    assert_eq!(fixture.token_balance(&fixture.treasury_vault_b), 0);

    let (trader, trader_a, trader_b) = fixture.user_with_tokens(0, SWAP_AMOUNT);
    let res = fixture.swap(&trader, trader_a, trader_b, SWAP_AMOUNT, false);
    assert!(res.is_ok(), "swap failed: {res:?}");

    let fee = expected_fee(SWAP_AMOUNT);
    let effective_input = SWAP_AMOUNT - fee;

    assert_eq!(fixture.token_balance(&fixture.treasury_vault_b), fee);
    let pool = fixture.pool_state();
    assert_eq!(pool.reserve_b, 200_000 + effective_input);
}

#[test]
fn swap_rejects_a_treasury_vault_that_does_not_belong_to_the_pool() {
    let mut fixture = PoolFixture::new();
    let (lp, lp_a, lp_b) = fixture.user_with_tokens(1_000_000, 1_000_000);
    fixture
        .add_liquidity(&lp, lp_a, lp_b, 100_000, 200_000)
        .unwrap();

    let (trader, trader_a, trader_b) = fixture.user_with_tokens(SWAP_AMOUNT, 0);

    // A real, valid token A account — just not the pool's treasury vault.
    // Must be a *different* account from any other account in this
    // instruction (an account that doesn't exist would fail at
    // deserialization instead of the `address` constraint under test, and
    // reusing `trader_a` would trip Anchor's duplicate-account check first).
    let fake_treasury_vault_a = lp_a;
    let treasury_vault_b = fixture.treasury_vault_b;
    let res = fixture.swap_with_treasury(
        &trader,
        trader_a,
        trader_b,
        SWAP_AMOUNT,
        true,
        fake_treasury_vault_a,
        treasury_vault_b,
    );

    expect_custom_error(res, u32::from(AnchorErrorCode::ConstraintAddress));
}
