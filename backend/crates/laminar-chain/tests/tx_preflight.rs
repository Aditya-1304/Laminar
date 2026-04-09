use anchor_lang::prelude::Pubkey;
use anyhow::Result;
use laminar_chain::{
    build_mint_amusd_instruction, prepare_simulated_unsigned_v0_transaction, ComputeBudgetConfig,
    LaminarBlockhashProvider, LaminarBuilderConfig, LaminarPreflightConfig,
    LaminarSimulationRequest, LaminarSimulationResult, LaminarSimulator, MintAmusdBuildInput,
    ProgramAddresses, RecentBlockhash,
};
use laminar_core::{Address, GlobalStateModel, LstRateBackend, OracleBackend};
use solana_hash::Hash;

fn addr(pubkey: Pubkey) -> Address {
    Address::from(pubkey.to_string())
}

fn sample_global_state(
    oracle_backend: OracleBackend,
    lst_rate_backend: LstRateBackend,
) -> GlobalStateModel {
    let treasury = Pubkey::new_unique();
    let amusd_mint = Pubkey::new_unique();
    let asol_mint = Pubkey::new_unique();
    let supported_lst_mint = Pubkey::new_unique();
    let pyth_feed = Pubkey::new_unique();
    let stake_pool = Pubkey::new_unique();

    GlobalStateModel {
        treasury: addr(treasury),
        amusd_mint: addr(amusd_mint),
        asol_mint: addr(asol_mint),
        supported_lst_mint: addr(supported_lst_mint),
        oracle_backend,
        lst_rate_backend,
        pyth_sol_usd_price_account: addr(pyth_feed),
        lst_stake_pool: addr(stake_pool),
        ..GlobalStateModel::default()
    }
}

#[derive(Clone)]
struct FixedBlockhashProvider {
    recent_blockhash: RecentBlockhash,
}

impl LaminarBlockhashProvider for FixedBlockhashProvider {
    fn get_latest_blockhash(&self) -> Result<RecentBlockhash> {
        Ok(self.recent_blockhash.clone())
    }
}

struct AssertingSimulator {
    expected_sig_verify: bool,
    expected_replace_recent_blockhash: bool,
    result: LaminarSimulationResult,
}

impl LaminarSimulator for AssertingSimulator {
    fn simulate_versioned_transaction(
        &self,
        request: LaminarSimulationRequest,
    ) -> Result<LaminarSimulationResult> {
        assert_eq!(request.sig_verify, self.expected_sig_verify);
        assert_eq!(
            request.replace_recent_blockhash,
            self.expected_replace_recent_blockhash
        );
        assert_eq!(request.transaction.signatures.len(), 1);
        Ok(self.result.clone())
    }
}

#[test]
fn preflight_pipeline_returns_unsigned_tx_simulation_summary_and_expiry() {
    let global = sample_global_state(OracleBackend::PythPush, LstRateBackend::SanctumStakePool);
    let user = Pubkey::new_unique();

    let envelope = build_mint_amusd_instruction(&MintAmusdBuildInput {
        global_state: global,
        user,
        user_amusd_account: Pubkey::new_unique(),
        user_lst_account: Pubkey::new_unique(),
        programs: ProgramAddresses::new(spl_token::id()),
        config: LaminarBuilderConfig {
            remaining_accounts: Default::default(),
            compute_budget: ComputeBudgetConfig {
                unit_limit: Some(250_000),
                unit_price_micro_lamports: Some(5_000),
            },
        },
        lst_amount: 1_000_000_000,
        min_amusd_out: 1,
    })
    .unwrap();

    let prepared = prepare_simulated_unsigned_v0_transaction(
        &user,
        &envelope,
        &FixedBlockhashProvider {
            recent_blockhash: RecentBlockhash::new(
                Hash::new_from_array([7u8; 32]),
                321,
                Some(9001),
            ),
        },
        &AssertingSimulator {
            expected_sig_verify: true,
            expected_replace_recent_blockhash: true,
            result: LaminarSimulationResult {
                logs: vec![
                    "Program log: mint_amusd".to_owned(),
                    "Program log: success".to_owned(),
                ],
                units_consumed: Some(88_000),
                error: None,
                replacement_blockhash: None,
                slot: Some(9002),
            },
        },
        &LaminarPreflightConfig {
            address_lookup_tables: Vec::new(),
            sig_verify: true,
            replace_recent_blockhash: true,
        },
    )
    .unwrap();

    assert_eq!(prepared.unsigned_transaction.required_signers, vec![user]);
    assert_eq!(prepared.unsigned_transaction.instruction_summaries.len(), 3);
    assert_eq!(prepared.unsigned_transaction.address_lookup_table_count, 0);

    assert_eq!(prepared.account_metas.len(), 16);
    assert_eq!(prepared.account_metas[0].pubkey, user.to_string());
    assert!(prepared.account_metas[0].is_signer);
    assert!(prepared.account_metas[0].is_writable);

    assert!(prepared.simulation_summary.ok);
    assert_eq!(prepared.simulation_summary.units_consumed, Some(88_000));
    assert_eq!(prepared.simulation_summary.log_count, 2);
    assert_eq!(prepared.simulation_summary.slot, Some(9002));

    assert_eq!(
        prepared.expiry_metadata.blockhash,
        Hash::new_from_array([7u8; 32]).to_string()
    );
    assert_eq!(prepared.expiry_metadata.last_valid_block_height, 321);
    assert_eq!(prepared.expiry_metadata.context_slot, Some(9001));
}

#[test]
fn preflight_pipeline_preserves_simulation_error_payload() {
    let global = sample_global_state(OracleBackend::Mock, LstRateBackend::Mock);
    let user = Pubkey::new_unique();

    let envelope = build_mint_amusd_instruction(&MintAmusdBuildInput {
        global_state: global,
        user,
        user_amusd_account: Pubkey::new_unique(),
        user_lst_account: Pubkey::new_unique(),
        programs: ProgramAddresses::new(spl_token::id()),
        config: LaminarBuilderConfig::default(),
        lst_amount: 500,
        min_amusd_out: 1,
    })
    .unwrap();

    let prepared = prepare_simulated_unsigned_v0_transaction(
        &user,
        &envelope,
        &FixedBlockhashProvider {
            recent_blockhash: RecentBlockhash::new(Hash::new_from_array([9u8; 32]), 999, None),
        },
        &AssertingSimulator {
            expected_sig_verify: false,
            expected_replace_recent_blockhash: false,
            result: LaminarSimulationResult {
                logs: vec!["Program log: custom failure".to_owned()],
                units_consumed: Some(12_345),
                error: Some("custom program error".to_owned()),
                replacement_blockhash: Some(Hash::new_from_array([1u8; 32]).to_string()),
                slot: Some(77),
            },
        },
        &LaminarPreflightConfig::default(),
    )
    .unwrap();

    assert!(!prepared.simulation_summary.ok);
    assert_eq!(
        prepared.simulation_summary.error.as_deref(),
        Some("custom program error")
    );
    assert_eq!(
        prepared.simulation_summary.replacement_blockhash,
        Some(Hash::new_from_array([1u8; 32]).to_string())
    );
    assert_eq!(prepared.simulation_summary.slot, Some(77));
}
