
use laminar_core::{
    Address, BalanceSheetSnapshot, GlobalStateModel, LaminarProtocolSnapshot, LstRateBackend,
    LstRateSnapshot, OracleBackend, OracleSnapshot, ProjectionMetadata, StabilityPoolSnapshot,
};
use laminar_indexer::{
    global_state_current_record_from_snapshot, ingestion_checkpoint_record,
    stability_pool_current_record_from_snapshot, ProjectionCheckpoint, ProjectionWriteContext,
};

fn addr(value: &str) -> Address {
    Address::from(value)
}

fn sample_snapshot() -> LaminarProtocolSnapshot {
    LaminarProtocolSnapshot {
        global: GlobalStateModel {
            authority: addr("authority"),
            amusd_mint: addr("amusd"),
            asol_mint: addr("asol"),
            treasury: addr("treasury"),
            supported_lst_mint: addr("lst"),
            total_lst_amount: 1_000,
            amusd_supply: 500,
            asol_supply: 200,
            min_cr_bps: 13_000,
            target_cr_bps: 15_000,
            mint_paused: false,
            redeem_paused: true,
            rounding_reserve_lamports: 77,
            uncertainty_index_bps: 88,
            nav_floor_lamports: 99,
            max_asol_mint_per_round: 123,
            oracle_backend: OracleBackend::PythPush,
            lst_rate_backend: LstRateBackend::SanctumStakePool,
            pyth_sol_usd_price_account: addr("pyth"),
            lst_stake_pool: addr("stake_pool"),
            ..GlobalStateModel::default()
        },
        oracle: OracleSnapshot {
            backend: OracleBackend::PythPush,
            price_safe_usd: 100,
            price_redeem_usd: 101,
            price_ema_usd: 102,
            confidence_usd: 1,
            confidence_bps: 10,
            uncertainty_index_bps: 88,
            last_update_slot: 40,
            max_staleness_slots: 5,
            max_conf_bps: 100,
        },
        lst_rate: LstRateSnapshot {
            backend: LstRateBackend::SanctumStakePool,
            supported_lst_mint: addr("lst"),
            lst_to_sol_rate: 1_050_000_000,
            stake_pool: addr("stake_pool"),
            last_tvl_update_slot: 41,
            last_lst_update_epoch: 7,
            max_lst_stale_epochs: 2,
        },
        stability_pool: StabilityPoolSnapshot {
            global_state: addr("global_state"),
            samusd_mint: addr("samusd"),
            pool_amusd_vault: addr("pool_amusd"),
            pool_asol_vault: addr("pool_asol"),
            total_amusd: 300,
            total_asol: 400,
            total_samusd: 500,
            stability_withdrawals_paused: true,
            last_harvest_lst_to_sol_rate: 1_060_000_000,
            ..StabilityPoolSnapshot::default()
        },
        balance_sheet: BalanceSheetSnapshot::default(),
        metadata: ProjectionMetadata {
            indexed_slot: Some(55),
            simulated_slot: None,
        },
    }
}

#[test]
fn maps_global_state_snapshot_into_current_record() {
    let snapshot = sample_snapshot();
    let context = ProjectionWriteContext::new("global_state").with_tx_signature("sig-1");

    let record = global_state_current_record_from_snapshot(&context, &snapshot).unwrap();

    assert_eq!(record.global_state_pubkey, "global_state");
    assert_eq!(record.projection_slot, 55);
    assert_eq!(record.tx_signature.as_deref(), Some("sig-1"));
    assert_eq!(record.oracle_backend, "pyth_push");
    assert_eq!(record.lst_rate_backend, "sanctum_stake_pool");
    assert_eq!(record.pyth_sol_usd_price_account.as_deref(), Some("pyth"));
    assert_eq!(record.lst_stake_pool.as_deref(), Some("stake_pool"));
    assert_eq!(record.rounding_reserve_lamports, 77);
    assert_eq!(record.uncertainty_index_bps, 88);
    assert_eq!(record.nav_floor_lamports, 99);
    assert_eq!(record.max_asol_mint_per_round, 123);
}

#[test]
fn maps_stability_pool_snapshot_when_pool_pubkey_is_present() {
    let snapshot = sample_snapshot();
    let context = ProjectionWriteContext::new("global_state")
        .with_stability_pool_pubkey("stability_pool")
        .with_tx_signature("sig-2");

    let record = stability_pool_current_record_from_snapshot(&context, &snapshot)
        .unwrap()
        .unwrap();

    assert_eq!(record.stability_pool_pubkey, "stability_pool");
    assert_eq!(record.global_state_pubkey, "global_state");
    assert_eq!(record.projection_slot, 55);
    assert_eq!(record.tx_signature.as_deref(), Some("sig-2"));
    assert_eq!(record.samusd_mint, "samusd");
    assert_eq!(record.pool_amusd_vault, "pool_amusd");
    assert_eq!(record.pool_asol_vault, "pool_asol");
    assert_eq!(record.total_amusd, 300);
    assert_eq!(record.total_asol, 400);
    assert_eq!(record.total_samusd, 500);
    assert!(record.stability_withdrawals_paused);
}

#[test]
fn skips_stability_pool_write_when_pubkey_is_absent() {
    let snapshot = sample_snapshot();
    let context = ProjectionWriteContext::new("global_state");

    let record = stability_pool_current_record_from_snapshot(&context, &snapshot).unwrap();

    assert!(record.is_none());
}

#[test]
fn builds_checkpoint_record() {
    let record = ingestion_checkpoint_record(&ProjectionCheckpoint {
        stream_name: "laminar_program".to_owned(),
        processed_slot: 100,
        confirmed_slot: 99,
        finalized_slot: 98,
        cursor: serde_json::json!({ "signature": "abc" }),
        metadata: serde_json::json!({ "note": "checkpoint" }),
    })
    .unwrap();

    assert_eq!(record.stream_name, "laminar_program");
    assert_eq!(record.processed_slot, 100);
    assert_eq!(record.confirmed_slot, 99);
    assert_eq!(record.finalized_slot, 98);
    assert_eq!(record.cursor["signature"], "abc");
    assert_eq!(record.metadata["note"], "checkpoint");
}
