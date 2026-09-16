//! Task 3: the remaining negative-case coverage not already exercised by
//! `task1_lifecycle.rs` (happy paths) and `task2_fees_treasury.rs` (fee /
//! treasury happy paths). Together the three files cover every instruction,
//! both directions of swap, and the failure conditions called out in the
//! assignment: zero amounts, insufficient balance/liquidity/shares, invalid
//! treasury, invalid pool accounts, and unauthorized withdrawals.

mod common;

use amm::error::AmmError;
use anchor_lang::error::ErrorCode as AnchorErrorCode;
use common::{expect_custom_error, try_initialize, PoolFixture};
use solana_signer::Signer;

// ---------------------------------------------------------------------
// TEST 1 — initialize
// ---------------------------------------------------------------------

#[test]
fn initialize_rejects_identical_mints() {
    let mut svm = common::setup();
    let payer = common::funded_keypair(&mut svm, 10 * common::LAMPORTS_PER_SOL);
    let mint = common::create_mint(&mut svm, &payer, &payer.pubkey(), 6);

    let attempt = try_initialize(&mut svm, &payer, &mint.pubkey(), &mint.pubkey());
    expect_custom_error(attempt.result, u32::from(AmmError::IdenticalMints));
}

// ---------------------------------------------------------------------
// TEST 2 — add_liquidity
// ---------------------------------------------------------------------

#[test]
fn add_liquidity_rejects_insufficient_balance() {
    let mut fixture = PoolFixture::new();
    let (user, user_a, user_b) = fixture.user_with_tokens(100, 100);

    // Ask to deposit more token A than the user actually holds.
    let res = fixture.add_liquidity(&user, user_a, user_b, 1_000, 100);

    // This is rejected by the SPL Token program itself (not our program),
    // with its own `InsufficientFunds` error (code 1) — we don't duplicate
    // a balance check the token program already performs for free.
    expect_custom_error(res, 1);
}

#[test]
fn add_liquidity_rejects_a_vault_that_does_not_belong_to_the_pool() {
    let mut fixture = PoolFixture::new();
    let (lp, lp_a, lp_b) = fixture.user_with_tokens(1_000, 1_000);

    // treasury_vault_a is a real token-A account, just not the pool's vault.
    let fake_vault_a = fixture.treasury_vault_a;
    let vault_b = fixture.vault_b;
    let res = fixture.add_liquidity_with_vaults(&lp, lp_a, lp_b, 100, 200, fake_vault_a, vault_b);

    expect_custom_error(res, u32::from(AnchorErrorCode::ConstraintAddress));
}

// ---------------------------------------------------------------------
// TEST 3 — remove_liquidity
// ---------------------------------------------------------------------

#[test]
fn remove_liquidity_rejects_zero_amount() {
    let mut fixture = PoolFixture::new();
    let (user, user_a, user_b) = fixture.user_with_tokens(1_000, 2_000);
    fixture.add_liquidity(&user, user_a, user_b, 1_000, 2_000).unwrap();

    let res = fixture.remove_liquidity(&user, user_a, user_b, 0);
    expect_custom_error(res, u32::from(AmmError::InvalidAmount));
}

#[test]
fn remove_liquidity_rejects_more_shares_than_owned() {
    let mut fixture = PoolFixture::new();
    let (user, user_a, user_b) = fixture.user_with_tokens(1_000, 2_000);
    fixture.add_liquidity(&user, user_a, user_b, 1_000, 2_000).unwrap();

    // The deposit above mints 1_414 shares; ask for far more than that.
    let res = fixture.remove_liquidity(&user, user_a, user_b, 100_000);
    expect_custom_error(res, u32::from(AmmError::InsufficientShares));
}

#[test]
fn remove_liquidity_rejects_someone_elses_position() {
    let mut fixture = PoolFixture::new();

    let (owner, owner_a, owner_b) = fixture.user_with_tokens(1_000, 2_000);
    fixture
        .add_liquidity(&owner, owner_a, owner_b, 1_000, 2_000)
        .unwrap();
    let owner_position = fixture.position_state(&owner.pubkey());
    assert!(owner_position.shares > 0);

    // A second user, with no liquidity of their own, tries to withdraw by
    // pointing at the first user's position account.
    let (attacker, attacker_a, attacker_b) = fixture.user_with_tokens(0, 0);
    let (owner_position_pda, _) = common::position_pda(&fixture.pool, &owner.pubkey());
    let res = fixture.remove_liquidity_with_position(
        &attacker,
        attacker_a,
        attacker_b,
        1,
        owner_position_pda,
    );

    // The position PDA is seeded from the signer's own pubkey, so a mismatch
    // between signer and position account fails Anchor's seeds constraint.
    expect_custom_error(res, u32::from(AnchorErrorCode::ConstraintSeeds));
}

// ---------------------------------------------------------------------
// TEST 6/7 — fee + treasury accounting across multiple swaps
// ---------------------------------------------------------------------

#[test]
fn treasury_balance_accumulates_across_multiple_swaps() {
    let mut fixture = PoolFixture::new();
    let (lp, lp_a, lp_b) = fixture.user_with_tokens(1_000_000, 1_000_000);
    fixture
        .add_liquidity(&lp, lp_a, lp_b, 100_000, 200_000)
        .unwrap();

    let (trader, trader_a, trader_b) = fixture.user_with_tokens(100_000, 0);

    let fee = |amount_in: u64| amount_in * amm::FEE_BPS / amm::FEE_DENOMINATOR;

    let treasury_before = fixture.token_balance(&fixture.treasury_vault_a);
    fixture.swap(&trader, trader_a, trader_b, 10_000, true).unwrap();
    let treasury_after_first = fixture.token_balance(&fixture.treasury_vault_a);
    assert_eq!(treasury_after_first, treasury_before + fee(10_000));

    fixture.swap(&trader, trader_a, trader_b, 20_000, true).unwrap();
    let treasury_after_second = fixture.token_balance(&fixture.treasury_vault_a);
    assert_eq!(treasury_after_second, treasury_after_first + fee(20_000));
}

// ---------------------------------------------------------------------
// TEST 8 — remaining failure cases
// ---------------------------------------------------------------------

#[test]
fn swap_rejects_when_pool_has_no_liquidity() {
    let mut fixture = PoolFixture::new();
    let (trader, trader_a, trader_b) = fixture.user_with_tokens(1_000, 0);

    let res = fixture.swap(&trader, trader_a, trader_b, 100, true);
    expect_custom_error(res, u32::from(AmmError::InsufficientLiquidity));
}

#[test]
fn add_liquidity_rejects_negative_ratio_deposit_yielding_zero_shares() {
    let mut fixture = PoolFixture::new();
    let (first, first_a, first_b) = fixture.user_with_tokens(10_000, 10_000);
    fixture
        .add_liquidity(&first, first_a, first_b, 1_000, 2_000)
        .unwrap();

    // A deposit so small, relative to the pool, that the proportional share
    // calculation floors to zero shares minted — must be rejected rather
    // than silently accepting tokens for nothing.
    let (second, second_a, second_b) = fixture.user_with_tokens(10, 10);
    let res = fixture.add_liquidity(&second, second_a, second_b, 1, 1);
    expect_custom_error(res, u32::from(AmmError::InvalidAmount));
}
