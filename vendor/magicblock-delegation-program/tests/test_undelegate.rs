use dlp::pda::{
    commit_record_pda_from_delegated_account, commit_state_pda_from_delegated_account,
    delegation_metadata_pda_from_delegated_account, delegation_record_pda_from_delegated_account,
    fees_vault_pda, validator_fees_vault_pda_from_validator,
};
use solana_program::rent::Rent;
use solana_program::{hash::Hash, native_token::LAMPORTS_PER_SOL, system_program};
use solana_program_test::{processor, read_file, BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

use crate::fixtures::{
    get_commit_record_account_data, get_delegation_metadata_data, get_delegation_record_data,
    COMMIT_NEW_STATE_ACCOUNT_DATA, DELEGATED_PDA_ID, DELEGATED_PDA_OWNER_ID, TEST_AUTHORITY,
};

mod fixtures;

#[tokio::test]
async fn test_finalize_and_undelegate() {
    // Setup
    let (banks, _, authority, blockhash) = setup_program_test_env().await;

    // Retrieve the accounts
    let delegation_record_pda = delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let commit_state_pda = commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID);

    // Save the new state data before undelegating
    let new_state_before_finalize = banks.get_account(commit_state_pda).await.unwrap().unwrap();
    let new_state_data_before_finalize = new_state_before_finalize.data.clone();

    // Create the finalize tx
    let ix_finalize = dlp::instruction_builder::finalize(authority.pubkey(), DELEGATED_PDA_ID);

    // Create the undelegate tx
    let ix_undelegate = dlp::instruction_builder::undelegate(
        authority.pubkey(),
        DELEGATED_PDA_ID,
        DELEGATED_PDA_OWNER_ID,
        authority.pubkey(),
    );

    // Submit the transaction
    let tx = Transaction::new_signed_with_payer(
        &[ix_finalize, ix_undelegate],
        Some(&authority.pubkey()),
        &[&authority],
        blockhash,
    );
    let res = banks.process_transaction(tx).await;
    println!("{:?}", res);
    assert!(res.is_ok());

    // Assert the state_diff was closed
    let commit_state_account = banks.get_account(commit_state_pda).await.unwrap();
    assert!(commit_state_account.is_none());

    // Assert the delegation_record_pda was closed
    let delegation_record_account = banks.get_account(delegation_record_pda).await.unwrap();
    assert!(delegation_record_account.is_none());

    // Assert the delegated account seeds pda was closed
    let delegation_metadata_pda = delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID);
    let delegation_metadata_account = banks.get_account(delegation_metadata_pda).await.unwrap();
    assert!(delegation_metadata_account.is_none());

    // Assert that the account owner is now set to the owner program
    let pda_account = banks.get_account(DELEGATED_PDA_ID).await.unwrap().unwrap();
    assert!(pda_account.owner.eq(&DELEGATED_PDA_OWNER_ID));

    // Assert the delegated account contains the data from the new state
    assert_eq!(new_state_data_before_finalize, pda_account.data);
}

async fn setup_program_test_env() -> (BanksClient, Keypair, Keypair, Hash) {
    let mut program_test = ProgramTest::new("dlp", dlp::ID, processor!(dlp::process_instruction));
    program_test.prefer_bpf(true);
    let authority = Keypair::from_bytes(&TEST_AUTHORITY).unwrap();

    program_test.add_account(
        authority.pubkey(),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup a delegated PDA
    program_test.add_account(
        DELEGATED_PDA_ID,
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated record PDA
    let delegation_record_data = get_delegation_record_data(authority.pubkey(), None);
    program_test.add_account(
        delegation_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(delegation_record_data.len()),
            data: delegation_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the delegated metadata PDA
    let delegation_metadata_data = get_delegation_metadata_data(authority.pubkey(), Some(true));
    program_test.add_account(
        delegation_metadata_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(delegation_metadata_data.len()),
            data: delegation_metadata_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the committed state PDA
    program_test.add_account(
        commit_state_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: COMMIT_NEW_STATE_ACCOUNT_DATA.into(),
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the commit state record PDA
    let commit_record_data = get_commit_record_account_data(authority.pubkey());
    program_test.add_account(
        commit_record_pda_from_delegated_account(&DELEGATED_PDA_ID),
        Account {
            lamports: Rent::default().minimum_balance(commit_record_data.len()),
            data: commit_record_data,
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup program to test undelegation
    let data = read_file("tests/buffers/test_delegation.so");
    program_test.add_account(
        DELEGATED_PDA_OWNER_ID,
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );

    // Setup the protocol fees vault
    program_test.add_account(
        fees_vault_pda(),
        Account {
            lamports: Rent::default().minimum_balance(0),
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Setup the validator fees vault
    program_test.add_account(
        validator_fees_vault_pda_from_validator(&authority.pubkey()),
        Account {
            lamports: LAMPORTS_PER_SOL,
            data: vec![],
            owner: dlp::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (banks, payer, blockhash) = program_test.start().await;
    (banks, payer, authority, blockhash)
}
