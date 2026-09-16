#![allow(dead_code)]

//! Shared LiteSVM test harness for the AMM program: spins up an in-process
//! validator, deploys the built `amm.so`, and exposes small helpers for
//! creating mints/token accounts and calling each instruction.

use anchor_lang::{
    solana_program::{instruction::Instruction, system_instruction, system_program},
    AccountDeserialize, InstructionData, ToAccountMetas,
};
use anchor_spl::associated_token::get_associated_token_address;
use litesvm::{types::{FailedTransactionMetadata, TransactionResult}, LiteSVM};
use solana_instruction::error::InstructionError;
use solana_keypair::Keypair;
use solana_message::{Message, VersionedMessage};
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;
use solana_transaction_error::TransactionError;
use solana_program_pack::Pack;
use spl_token_interface::{
    state::{Account as TokenAccountState, Mint as MintState},
    ID as TOKEN_PROGRAM_ID,
};

pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

fn program_bytes() -> &'static [u8] {
    include_bytes!(concat!(env!("CARGO_TARGET_TMPDIR"), "/../deploy/amm.so"))
}

pub fn setup() -> LiteSVM {
    let mut svm = LiteSVM::new();
    svm.add_program(amm::ID, program_bytes()).unwrap();
    svm
}

pub fn funded_keypair(svm: &mut LiteSVM, lamports: u64) -> Keypair {
    let kp = Keypair::new();
    svm.airdrop(&kp.pubkey(), lamports).unwrap();
    kp
}

pub fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    ixs: &[Instruction],
    extra_signers: &[&Keypair],
) -> TransactionResult {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let mut signers: Vec<&Keypair> = vec![payer];
    for signer in extra_signers.iter().copied() {
        if signer.pubkey() != payer.pubkey() {
            signers.push(signer);
        }
    }
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &signers).unwrap();
    svm.send_transaction(tx)
}

pub fn create_mint(svm: &mut LiteSVM, payer: &Keypair, mint_authority: &Pubkey, decimals: u8) -> Keypair {
    let mint = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(MintState::LEN);
    let create_ix = system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        rent,
        MintState::LEN as u64,
        &TOKEN_PROGRAM_ID,
    );
    let init_ix = spl_token_interface::instruction::initialize_mint2(
        &TOKEN_PROGRAM_ID,
        &mint.pubkey(),
        mint_authority,
        None,
        decimals,
    )
    .unwrap();
    send(svm, payer, &[create_ix, init_ix], &[&mint]).unwrap();
    mint
}

pub fn create_token_account(svm: &mut LiteSVM, payer: &Keypair, owner: &Pubkey, mint: &Pubkey) -> Keypair {
    let account = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(TokenAccountState::LEN);
    let create_ix = system_instruction::create_account(
        &payer.pubkey(),
        &account.pubkey(),
        rent,
        TokenAccountState::LEN as u64,
        &TOKEN_PROGRAM_ID,
    );
    let init_ix = spl_token_interface::instruction::initialize_account3(
        &TOKEN_PROGRAM_ID,
        &account.pubkey(),
        mint,
        owner,
    )
    .unwrap();
    send(svm, payer, &[create_ix, init_ix], &[&account]).unwrap();
    account
}

pub fn mint_to(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    destination: &Pubkey,
    authority: &Keypair,
    amount: u64,
) {
    let ix = spl_token_interface::instruction::mint_to(
        &TOKEN_PROGRAM_ID,
        mint,
        destination,
        &authority.pubkey(),
        &[],
        amount,
    )
    .unwrap();
    send(svm, payer, &[ix], &[authority]).unwrap();
}

/// Asserts a transaction failed with the given Anchor custom error code
/// (get this with `u32::from(amm::error::AmmError::Variant)`).
pub fn expect_custom_error(result: TransactionResult, expected_code: u32) {
    match result {
        Ok(_) => panic!(
            "expected transaction to fail with custom error {expected_code}, but it succeeded"
        ),
        Err(FailedTransactionMetadata { err, meta }) => match err {
            TransactionError::InstructionError(_, InstructionError::Custom(code)) => {
                assert_eq!(
                    code, expected_code,
                    "unexpected error code (logs: {:?})",
                    meta.logs
                );
            }
            other => panic!("expected custom error {expected_code}, got {other:?}"),
        },
    }
}

pub fn token_balance(svm: &LiteSVM, account: &Pubkey) -> u64 {
    let acc = svm.get_account(account).expect("token account not found");
    TokenAccountState::unpack(&acc.data).unwrap().amount
}

pub fn pool_pda(mint_a: &Pubkey, mint_b: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[amm::POOL_SEED, mint_a.as_ref(), mint_b.as_ref()], &amm::ID)
}

pub fn position_pda(pool: &Pubkey, owner: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[amm::POSITION_SEED, pool.as_ref(), owner.as_ref()], &amm::ID)
}

use anchor_lang::prelude::Pubkey;

/// A pool with two fresh mints, already initialized. Every field needed to
/// build further instructions is public so tests can assemble exactly the
/// scenario they want.
pub struct PoolFixture {
    pub svm: LiteSVM,
    pub payer: Keypair,
    pub mint_a: Keypair,
    pub mint_b: Keypair,
    pub pool: Pubkey,
    pub vault_a: Pubkey,
    pub vault_b: Pubkey,
    pub treasury: Pubkey,
    pub treasury_vault_a: Pubkey,
    pub treasury_vault_b: Pubkey,
}

/// Addresses derived for a pool before it exists, plus the result of trying
/// to initialize it. Used directly by tests that want to exercise
/// `initialize` failure cases (e.g. identical mints).
pub struct InitializeAttempt {
    pub pool: Pubkey,
    pub vault_a: Pubkey,
    pub vault_b: Pubkey,
    pub treasury: Pubkey,
    pub treasury_vault_a: Pubkey,
    pub treasury_vault_b: Pubkey,
    pub result: TransactionResult,
}

pub fn try_initialize(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint_a: &Pubkey,
    mint_b: &Pubkey,
) -> InitializeAttempt {
    let (pool, _bump) = pool_pda(mint_a, mint_b);
    let vault_a = get_associated_token_address(&pool, mint_a);
    let vault_b = get_associated_token_address(&pool, mint_b);
    let treasury = Keypair::new().pubkey();
    let treasury_vault_a = get_associated_token_address(&treasury, mint_a);
    let treasury_vault_b = get_associated_token_address(&treasury, mint_b);

    let ix = Instruction {
        program_id: amm::ID,
        accounts: amm::accounts::InitializePool {
            payer: payer.pubkey(),
            mint_a: *mint_a,
            mint_b: *mint_b,
            pool,
            vault_a,
            vault_b,
            treasury,
            treasury_vault_a,
            treasury_vault_b,
            token_program: TOKEN_PROGRAM_ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: amm::instruction::Initialize {}.data(),
    };
    let result = send(svm, payer, &[ix], &[]);

    InitializeAttempt {
        pool,
        vault_a,
        vault_b,
        treasury,
        treasury_vault_a,
        treasury_vault_b,
        result,
    }
}

impl PoolFixture {
    pub fn new() -> Self {
        let mut svm = setup();
        let payer = funded_keypair(&mut svm, 100 * LAMPORTS_PER_SOL);
        let mint_a = create_mint(&mut svm, &payer, &payer.pubkey(), 6);
        let mint_b = create_mint(&mut svm, &payer, &payer.pubkey(), 6);
        let attempt = try_initialize(&mut svm, &payer, &mint_a.pubkey(), &mint_b.pubkey());
        attempt.result.expect("initialize should succeed");
        let InitializeAttempt {
            pool,
            vault_a,
            vault_b,
            treasury,
            treasury_vault_a,
            treasury_vault_b,
            ..
        } = attempt;

        Self {
            svm,
            payer,
            mint_a,
            mint_b,
            pool,
            vault_a,
            vault_b,
            treasury,
            treasury_vault_a,
            treasury_vault_b,
        }
    }

    /// Airdrops SOL to a new user and gives them funded token accounts for
    /// both sides of the pool (an amount of 0 just skips minting that side).
    pub fn user_with_tokens(&mut self, amount_a: u64, amount_b: u64) -> (Keypair, Pubkey, Pubkey) {
        let user = funded_keypair(&mut self.svm, 10 * LAMPORTS_PER_SOL);
        let user_ata_a =
            create_token_account(&mut self.svm, &self.payer, &user.pubkey(), &self.mint_a.pubkey())
                .pubkey();
        let user_ata_b =
            create_token_account(&mut self.svm, &self.payer, &user.pubkey(), &self.mint_b.pubkey())
                .pubkey();
        if amount_a > 0 {
            let mint_a = self.mint_a.pubkey();
            let authority = self.payer.insecure_clone();
            mint_to(&mut self.svm, &self.payer, &mint_a, &user_ata_a, &authority, amount_a);
        }
        if amount_b > 0 {
            let mint_b = self.mint_b.pubkey();
            let authority = self.payer.insecure_clone();
            mint_to(&mut self.svm, &self.payer, &mint_b, &user_ata_b, &authority, amount_b);
        }
        (user, user_ata_a, user_ata_b)
    }

    pub fn add_liquidity(
        &mut self,
        user: &Keypair,
        user_a: Pubkey,
        user_b: Pubkey,
        amount_a: u64,
        amount_b: u64,
    ) -> TransactionResult {
        let vault_a = self.vault_a;
        let vault_b = self.vault_b;
        self.add_liquidity_with_vaults(user, user_a, user_b, amount_a, amount_b, vault_a, vault_b)
    }

    /// Same as [`Self::add_liquidity`] but lets a test pass arbitrary vault
    /// accounts, to exercise the "invalid pool account is rejected" case.
    #[allow(clippy::too_many_arguments)]
    pub fn add_liquidity_with_vaults(
        &mut self,
        user: &Keypair,
        user_a: Pubkey,
        user_b: Pubkey,
        amount_a: u64,
        amount_b: u64,
        vault_a: Pubkey,
        vault_b: Pubkey,
    ) -> TransactionResult {
        let (position, _) = position_pda(&self.pool, &user.pubkey());
        let ix = Instruction {
            program_id: amm::ID,
            accounts: amm::accounts::AddLiquidity {
                user: user.pubkey(),
                pool: self.pool,
                position,
                vault_a,
                vault_b,
                user_token_a: user_a,
                user_token_b: user_b,
                token_program: TOKEN_PROGRAM_ID,
                system_program: system_program::ID,
            }
            .to_account_metas(None),
            data: amm::instruction::AddLiquidity { amount_a, amount_b }.data(),
        };
        send(&mut self.svm, user, &[ix], &[])
    }

    pub fn remove_liquidity(
        &mut self,
        user: &Keypair,
        user_a: Pubkey,
        user_b: Pubkey,
        shares: u64,
    ) -> TransactionResult {
        let (position, _) = position_pda(&self.pool, &user.pubkey());
        self.remove_liquidity_with_position(user, user_a, user_b, shares, position)
    }

    /// Same as [`Self::remove_liquidity`] but lets a test pass an arbitrary
    /// position account, e.g. another user's, to exercise the "can't
    /// withdraw someone else's liquidity" case.
    #[allow(clippy::too_many_arguments)]
    pub fn remove_liquidity_with_position(
        &mut self,
        user: &Keypair,
        user_a: Pubkey,
        user_b: Pubkey,
        shares: u64,
        position: Pubkey,
    ) -> TransactionResult {
        let ix = Instruction {
            program_id: amm::ID,
            accounts: amm::accounts::RemoveLiquidity {
                user: user.pubkey(),
                pool: self.pool,
                position,
                vault_a: self.vault_a,
                vault_b: self.vault_b,
                user_token_a: user_a,
                user_token_b: user_b,
                token_program: TOKEN_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: amm::instruction::RemoveLiquidity { shares }.data(),
        };
        send(&mut self.svm, user, &[ix], &[])
    }

    pub fn swap(
        &mut self,
        user: &Keypair,
        user_a: Pubkey,
        user_b: Pubkey,
        amount_in: u64,
        a_to_b: bool,
    ) -> TransactionResult {
        let treasury_vault_a = self.treasury_vault_a;
        let treasury_vault_b = self.treasury_vault_b;
        self.swap_with_treasury(
            user,
            user_a,
            user_b,
            amount_in,
            a_to_b,
            treasury_vault_a,
            treasury_vault_b,
        )
    }

    /// Same as [`Self::swap`] but lets a test pass arbitrary treasury vault
    /// accounts, to exercise the "invalid treasury is rejected" case.
    #[allow(clippy::too_many_arguments)]
    pub fn swap_with_treasury(
        &mut self,
        user: &Keypair,
        user_a: Pubkey,
        user_b: Pubkey,
        amount_in: u64,
        a_to_b: bool,
        treasury_vault_a: Pubkey,
        treasury_vault_b: Pubkey,
    ) -> TransactionResult {
        let ix = Instruction {
            program_id: amm::ID,
            accounts: amm::accounts::Swap {
                user: user.pubkey(),
                pool: self.pool,
                vault_a: self.vault_a,
                vault_b: self.vault_b,
                treasury_vault_a,
                treasury_vault_b,
                user_token_a: user_a,
                user_token_b: user_b,
                token_program: TOKEN_PROGRAM_ID,
            }
            .to_account_metas(None),
            data: amm::instruction::Swap { amount_in, a_to_b }.data(),
        };
        send(&mut self.svm, user, &[ix], &[])
    }

    pub fn pool_state(&self) -> amm::state::Pool {
        let acc = self.svm.get_account(&self.pool).unwrap();
        amm::state::Pool::try_deserialize(&mut acc.data.as_slice()).unwrap()
    }

    pub fn position_state(&self, user: &Pubkey) -> amm::state::Position {
        let (position, _) = position_pda(&self.pool, user);
        let acc = self.svm.get_account(&position).unwrap();
        amm::state::Position::try_deserialize(&mut acc.data.as_slice()).unwrap()
    }

    pub fn token_balance(&self, account: &Pubkey) -> u64 {
        token_balance(&self.svm, account)
    }
}
