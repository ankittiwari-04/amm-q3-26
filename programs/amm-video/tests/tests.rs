use {
    anchor_spl::associated_token,
    litesvm::LiteSVM,
    litesvm_token::{
        get_spl_account,
        spl_token::state::Account as TokenAccount,
        CreateMint,
    },
    solana_keypair::Keypair,
    solana_message::{Instruction, Message, VersionedMessage},
    solana_pubkey::Pubkey,
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

mod ix_handlers;
use ix_handlers::*;

fn send(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    signers: &[&Keypair],
) -> litesvm::types::TransactionResult {
    svm.expire_blockhash();
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();
    svm.send_transaction(tx)
}

fn token_balance(svm: &LiteSVM, ata: &Pubkey) -> u64 {
    get_spl_account::<TokenAccount>(svm, ata)
        .expect("token account should exist")
        .amount
}

/// Shared pool fixtures for LiteSVM tests.
fn setup() -> (
    LiteSVM,
    Keypair,
    Pubkey, // mint_x
    Pubkey, // mint_y
    Pubkey, // config
    Pubkey, // mint_lp
    Pubkey, // vault_x
    Pubkey, // vault_y
    Pubkey, // treasury (owner) — distinct from payer so ATAs don't collide
    Pubkey, // treasury_x
    Pubkey, // treasury_y
) {
    let program_id = amm_video::id();
    let payer = Keypair::new();
    let treasury_kp = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/amm_video.so");
    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let mint_x = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let mint_y = CreateMint::new(&mut svm, &payer)
        .decimals(6)
        .authority(&payer.pubkey())
        .send()
        .unwrap();

    let config =
        Pubkey::find_program_address(&[b"config", &123u64.to_le_bytes()], &amm_video::id()).0;
    let mint_lp = Pubkey::find_program_address(&[b"lp", config.as_ref()], &amm_video::id()).0;

    let vault_x = associated_token::get_associated_token_address(&config, &mint_x);
    let vault_y = associated_token::get_associated_token_address(&config, &mint_y);

    // Separate wallet so treasury ATAs are not the same as the user's ATAs.
    let treasury = treasury_kp.pubkey();
    let treasury_x = associated_token::get_associated_token_address(&treasury, &mint_x);
    let treasury_y = associated_token::get_associated_token_address(&treasury, &mint_y);

    (
        svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury, treasury_x,
        treasury_y,
    )
}

#[test]
fn test_initialize() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury, treasury_x, treasury_y) =
        setup();

    let instruction = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury,
        treasury_x,
        treasury_y,
    );
    let res = send(&mut svm, &[instruction], &payer, &[&payer]);
    assert!(res.is_ok(), "initialize failed: {res:?}");

    // Treasury ATAs exist and start empty.
    assert_eq!(token_balance(&svm, &treasury_x), 0);
    assert_eq!(token_balance(&svm, &treasury_y), 0);
}

#[test]
pub fn test_deposit() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury,
        treasury_x,
        treasury_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "deposit failed: {res:?}");

    // Initial deposit: max_x/max_y with amount LP minted 1:1 when pool is empty.
    assert_eq!(token_balance(&svm, &vault_x), 200_000_000);
    assert_eq!(token_balance(&svm, &vault_y), 200_000_000);
}

#[test]
pub fn test_withdraw() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury,
        treasury_x,
        treasury_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let withdraw_ix = create_withdraw_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );
    let res = send(
        &mut svm,
        &[init_ix, deposit_ix, withdraw_ix],
        &payer,
        &[&payer],
    );
    assert!(res.is_ok(), "withdraw failed: {res:?}");
}

#[test]
pub fn test_swap_with_slippage() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury,
        treasury_x,
        treasury_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    // Reasonable min_out — swap should succeed.
    let swap_ok = create_swap_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        10_000_000,
        5_000_000,
    );

    let res = send(&mut svm, &[init_ix, deposit_ix, swap_ok], &payer, &[&payer]);
    assert!(res.is_ok(), "swap with slippage floor failed: {res:?}");
}

#[test]
pub fn test_swap_slippage_exceeded() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury,
        treasury_x,
        treasury_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let res_setup = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res_setup.is_ok());

    // Impossible min_out — must fail slippage check.
    let swap_bad = create_swap_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        10_000_000,
        9_999_999_999,
    );

    let res = send(&mut svm, &[swap_bad], &payer, &[&payer]);
    assert!(res.is_err(), "expected slippage failure");
}

#[test]
pub fn test_swap_routes_protocol_fee_to_treasury() {
    let (mut svm, payer, mint_x, mint_y, config, mint_lp, vault_x, vault_y, treasury, treasury_x, treasury_y) =
        setup();
    let init_ix = create_initialise_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        config,
        mint_lp,
        vault_x,
        vault_y,
        treasury,
        treasury_x,
        treasury_y,
    );

    let deposit_ix = create_deposit_ix(
        &mut svm, &payer, mint_x, mint_y, mint_lp, config, vault_x, vault_y,
    );

    let res_setup = send(&mut svm, &[init_ix, deposit_ix], &payer, &[&payer]);
    assert!(res_setup.is_ok());

    let amount_in: u64 = 10_000_000;
    // fee = 30 bps → fee_amount = 10_000_000 * 30 / 10_000 = 30_000
    // protocol_fee = 2000 bps of fee → 30_000 * 2000 / 10_000 = 6_000
    let expected_protocol_fee: u64 = 6_000;

    assert_eq!(token_balance(&svm, &treasury_x), 0);

    let swap_ix = create_swap_ix(
        &mut svm,
        &payer,
        mint_x,
        mint_y,
        mint_lp,
        config,
        vault_x,
        vault_y,
        treasury_x,
        treasury_y,
        amount_in,
        1, // accept any non-zero out
    );

    let res = send(&mut svm, &[swap_ix], &payer, &[&payer]);
    assert!(res.is_ok(), "swap failed: {res:?}");

    let treasury_x_bal = token_balance(&svm, &treasury_x);
    assert_eq!(
        treasury_x_bal, expected_protocol_fee,
        "treasury_x should receive the protocol share of the swap fee"
    );
    // Output mint treasury untouched for an X→Y swap.
    assert_eq!(token_balance(&svm, &treasury_y), 0);

    // Vault_x should hold: initial deposit + amount_in - protocol_fee
    // (LP fee portion of the 30k stays in the vault).
    let expected_vault_x = 200_000_000 + amount_in - expected_protocol_fee;
    assert_eq!(token_balance(&svm, &vault_x), expected_vault_x);
}
