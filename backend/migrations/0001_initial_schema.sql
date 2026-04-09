CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS slots (
    slot BIGINT PRIMARY KEY,
    parent_slot BIGINT,
    blockhash TEXT,
    leader_identity TEXT,
    block_time TIMESTAMPTZ,
    status TEXT NOT NULL DEFAULT 'processed',
    transaction_count INTEGER,
    raw JSONB NOT NULL DEFAULT '{}'::jsonb,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_slots_status_slot ON slots (status, slot DESC);

CREATE TABLE IF NOT EXISTS transactions (
    signature TEXT PRIMARY KEY,
    slot BIGINT NOT NULL,
    tx_index INTEGER,
    program_ids TEXT[] NOT NULL DEFAULT '{}'::text[],
    signer_pubkeys TEXT[] NOT NULL DEFAULT '{}'::text[],
    success BOOLEAN NOT NULL,
    error_code TEXT,
    error_detail JSONB NOT NULL DEFAULT '{}'::jsonb,
    compute_units_consumed BIGINT,
    fee_lamports NUMERIC(20,0),
    block_time TIMESTAMPTZ,
    raw_message JSONB NOT NULL DEFAULT '{}'::jsonb,
    raw_meta JSONB NOT NULL DEFAULT '{}'::jsonb,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_transactions_slot ON transactions (slot DESC, tx_index ASC);
CREATE INDEX IF NOT EXISTS idx_transactions_success ON transactions (success, slot DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_program_ids ON transactions USING GIN (program_ids);
CREATE INDEX IF NOT EXISTS idx_transactions_signer_pubkeys ON transactions USING GIN (signer_pubkeys);

CREATE TABLE IF NOT EXISTS instruction_calls (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    signature TEXT NOT NULL,
    slot BIGINT NOT NULL,
    tx_index INTEGER,
    instruction_index INTEGER NOT NULL,
    inner_instruction_index INTEGER,
    program_id TEXT NOT NULL,
    instruction_name TEXT,
    idl_name TEXT,
    is_laminar BOOLEAN NOT NULL DEFAULT false,
    accounts JSONB NOT NULL DEFAULT '[]'::jsonb,
    args JSONB NOT NULL DEFAULT '{}'::jsonb,
    raw_data_base64 TEXT,
    decode_status TEXT NOT NULL DEFAULT 'decoded',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_instruction_calls_signature ON instruction_calls (signature, instruction_index, inner_instruction_index);
CREATE INDEX IF NOT EXISTS idx_instruction_calls_slot ON instruction_calls (slot DESC);
CREATE INDEX IF NOT EXISTS idx_instruction_calls_program ON instruction_calls (program_id, instruction_name);

CREATE TABLE IF NOT EXISTS anchor_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    signature TEXT NOT NULL,
    slot BIGINT NOT NULL,
    tx_index INTEGER,
    instruction_index INTEGER NOT NULL,
    inner_instruction_index INTEGER,
    event_index INTEGER NOT NULL DEFAULT 0,
    program_id TEXT NOT NULL,
    event_name TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    raw_log TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_anchor_events_signature ON anchor_events (signature, instruction_index, event_index);
CREATE INDEX IF NOT EXISTS idx_anchor_events_slot ON anchor_events (slot DESC);
CREATE INDEX IF NOT EXISTS idx_anchor_events_name ON anchor_events (event_name, slot DESC);

CREATE TABLE IF NOT EXISTS account_snapshots_raw (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slot BIGINT NOT NULL,
    signature TEXT,
    write_version BIGINT,
    pubkey TEXT NOT NULL,
    owner TEXT NOT NULL,
    lamports NUMERIC(20,0) NOT NULL,
    executable BOOLEAN NOT NULL DEFAULT false,
    rent_epoch BIGINT NOT NULL DEFAULT 0,
    data_encoding TEXT NOT NULL DEFAULT 'base64',
    data_base64 TEXT NOT NULL,
    data_len INTEGER NOT NULL,
    snapshot_kind TEXT NOT NULL DEFAULT 'post_tx',
    is_laminar_related BOOLEAN NOT NULL DEFAULT false,
    raw JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_account_snapshots_raw_pubkey_slot ON account_snapshots_raw (pubkey, slot DESC);
CREATE INDEX IF NOT EXISTS idx_account_snapshots_raw_signature ON account_snapshots_raw (signature);
CREATE INDEX IF NOT EXISTS idx_account_snapshots_raw_laminar ON account_snapshots_raw (is_laminar_related, slot DESC);

CREATE TABLE IF NOT EXISTS ingestion_checkpoints (
    stream_name TEXT PRIMARY KEY,
    processed_slot BIGINT NOT NULL DEFAULT 0,
    confirmed_slot BIGINT NOT NULL DEFAULT 0,
    finalized_slot BIGINT NOT NULL DEFAULT 0,
    cursor JSONB NOT NULL DEFAULT '{}'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS global_state_current (
    global_state_pubkey TEXT PRIMARY KEY,
    projection_slot BIGINT NOT NULL,
    tx_signature TEXT,
    authority TEXT NOT NULL,
    amusd_mint TEXT NOT NULL,
    asol_mint TEXT NOT NULL,
    treasury TEXT NOT NULL,
    supported_lst_mint TEXT NOT NULL,
    total_lst_amount NUMERIC(20,0) NOT NULL,
    amusd_supply NUMERIC(20,0) NOT NULL,
    asol_supply NUMERIC(20,0) NOT NULL,
    min_cr_bps NUMERIC(20,0) NOT NULL,
    target_cr_bps NUMERIC(20,0) NOT NULL,
    mint_paused BOOLEAN NOT NULL,
    redeem_paused BOOLEAN NOT NULL,
    oracle_backend TEXT NOT NULL,
    lst_rate_backend TEXT NOT NULL,
    pyth_sol_usd_price_account TEXT,
    lst_stake_pool TEXT,
    rounding_reserve_lamports NUMERIC(20,0) NOT NULL,
    uncertainty_index_bps NUMERIC(20,0) NOT NULL,
    nav_floor_lamports NUMERIC(20,0) NOT NULL,
    max_asol_mint_per_round NUMERIC(20,0) NOT NULL,
    raw_model JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_global_state_current_projection_slot ON global_state_current (projection_slot DESC);

CREATE TABLE IF NOT EXISTS global_state_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    global_state_pubkey TEXT NOT NULL,
    projection_slot BIGINT NOT NULL,
    tx_signature TEXT,
    snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_global_state_history_pubkey_slot ON global_state_history (global_state_pubkey, projection_slot DESC);

CREATE TABLE IF NOT EXISTS stability_pool_current (
    stability_pool_pubkey TEXT PRIMARY KEY,
    global_state_pubkey TEXT NOT NULL,
    projection_slot BIGINT NOT NULL,
    tx_signature TEXT,
    samusd_mint TEXT NOT NULL,
    pool_amusd_vault TEXT NOT NULL,
    pool_asol_vault TEXT NOT NULL,
    total_amusd NUMERIC(20,0) NOT NULL,
    total_asol NUMERIC(20,0) NOT NULL,
    total_samusd NUMERIC(20,0) NOT NULL,
    stability_withdrawals_paused BOOLEAN NOT NULL,
    last_harvest_lst_to_sol_rate NUMERIC(20,0) NOT NULL,
    raw_model JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_stability_pool_current_projection_slot ON stability_pool_current (projection_slot DESC);

CREATE TABLE IF NOT EXISTS stability_pool_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stability_pool_pubkey TEXT NOT NULL,
    global_state_pubkey TEXT NOT NULL,
    projection_slot BIGINT NOT NULL,
    tx_signature TEXT,
    snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_stability_pool_history_pubkey_slot ON stability_pool_history (stability_pool_pubkey, projection_slot DESC);

CREATE TABLE IF NOT EXISTS mint_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    projection_slot BIGINT NOT NULL,
    tx_signature TEXT,
    mint_pubkey TEXT NOT NULL,
    token_program TEXT NOT NULL,
    decimals INTEGER NOT NULL,
    supply NUMERIC(20,0) NOT NULL,
    mint_authority TEXT,
    freeze_authority TEXT,
    raw_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_mint_snapshots_mint_slot ON mint_snapshots (mint_pubkey, projection_slot DESC);

CREATE TABLE IF NOT EXISTS vault_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    projection_slot BIGINT NOT NULL,
    tx_signature TEXT,
    vault_pubkey TEXT NOT NULL,
    token_program TEXT NOT NULL,
    vault_role TEXT NOT NULL,
    mint_pubkey TEXT NOT NULL,
    owner_pubkey TEXT NOT NULL,
    amount NUMERIC(20,0) NOT NULL,
    raw_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_vault_snapshots_role_slot ON vault_snapshots (vault_role, projection_slot DESC);
CREATE INDEX IF NOT EXISTS idx_vault_snapshots_pubkey_slot ON vault_snapshots (vault_pubkey, projection_slot DESC);

CREATE TABLE IF NOT EXISTS oracle_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    projection_slot BIGINT NOT NULL,
    tx_signature TEXT,
    global_state_pubkey TEXT NOT NULL,
    oracle_backend TEXT NOT NULL,
    price_safe_usd NUMERIC(20,0) NOT NULL,
    price_redeem_usd NUMERIC(20,0) NOT NULL,
    price_ema_usd NUMERIC(20,0) NOT NULL,
    confidence_usd NUMERIC(20,0) NOT NULL,
    confidence_bps NUMERIC(20,0) NOT NULL,
    uncertainty_index_bps NUMERIC(20,0) NOT NULL,
    last_update_slot BIGINT,
    max_staleness_slots NUMERIC(20,0) NOT NULL,
    max_conf_bps NUMERIC(20,0) NOT NULL,
    raw_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_oracle_snapshots_slot ON oracle_snapshots (projection_slot DESC);
CREATE INDEX IF NOT EXISTS idx_oracle_snapshots_global_state ON oracle_snapshots (global_state_pubkey, projection_slot DESC);

CREATE TABLE IF NOT EXISTS lst_rate_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    projection_slot BIGINT NOT NULL,
    tx_signature TEXT,
    global_state_pubkey TEXT NOT NULL,
    lst_rate_backend TEXT NOT NULL,
    supported_lst_mint TEXT NOT NULL,
    stake_pool TEXT,
    lst_to_sol_rate NUMERIC(20,0) NOT NULL,
    last_tvl_update_slot BIGINT,
    last_lst_update_epoch NUMERIC(20,0),
    max_lst_stale_epochs NUMERIC(20,0),
    raw_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_lst_rate_snapshots_global_state ON lst_rate_snapshots (global_state_pubkey, projection_slot DESC);

CREATE TABLE IF NOT EXISTS protocol_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    projection_slot BIGINT NOT NULL,
    tx_signature TEXT,
    global_state_pubkey TEXT NOT NULL,
    stability_pool_pubkey TEXT,
    tvl_sol_lamports NUMERIC(20,0),
    liability_sol_lamports NUMERIC(20,0),
    claimable_equity_sol_lamports NUMERIC(20,0),
    accounting_equity_sol_lamports NUMERIC(20,0),
    cr_bps NUMERIC(20,0),
    stability_withdrawals_paused BOOLEAN,
    snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_protocol_snapshots_slot ON protocol_snapshots (projection_slot DESC);

CREATE TABLE IF NOT EXISTS wallet_positions_current (
    wallet_pubkey TEXT NOT NULL,
    position_kind TEXT NOT NULL,
    mint_pubkey TEXT NOT NULL,
    token_account_pubkey TEXT,
    balance NUMERIC(20,0) NOT NULL,
    value_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    projection_slot BIGINT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (wallet_pubkey, position_kind, mint_pubkey)
);

CREATE INDEX IF NOT EXISTS idx_wallet_positions_current_mint ON wallet_positions_current (mint_pubkey, projection_slot DESC);

CREATE TABLE IF NOT EXISTS wallet_position_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_pubkey TEXT NOT NULL,
    position_kind TEXT NOT NULL,
    mint_pubkey TEXT NOT NULL,
    projection_slot BIGINT NOT NULL,
    tx_signature TEXT,
    snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_wallet_position_history_wallet ON wallet_position_history (wallet_pubkey, projection_slot DESC);

CREATE TABLE IF NOT EXISTS wallet_operation_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_pubkey TEXT NOT NULL,
    signature TEXT,
    slot BIGINT,
    operation_type TEXT NOT NULL,
    status TEXT NOT NULL,
    request JSONB NOT NULL DEFAULT '{}'::jsonb,
    result JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_wallet_operation_history_wallet ON wallet_operation_history (wallet_pubkey, created_at DESC);

CREATE TABLE IF NOT EXISTS unsigned_tx_requests (
    request_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    idempotency_key TEXT UNIQUE,
    wallet_pubkey TEXT NOT NULL,
    request_kind TEXT NOT NULL,
    request_body JSONB NOT NULL DEFAULT '{}'::jsonb,
    quote_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    unsigned_tx_base64 TEXT,
    required_signers JSONB NOT NULL DEFAULT '[]'::jsonb,
    recent_blockhash TEXT,
    last_valid_block_height NUMERIC(20,0),
    simulation_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    expiry_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_unsigned_tx_requests_wallet ON unsigned_tx_requests (wallet_pubkey, created_at DESC);

CREATE TABLE IF NOT EXISTS tx_submission_attempts (
    attempt_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    request_id UUID,
    signature TEXT,
    submission_target TEXT NOT NULL,
    status TEXT NOT NULL,
    error_text TEXT,
    rpc_response JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_tx_submission_attempts_request ON tx_submission_attempts (request_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_tx_submission_attempts_signature ON tx_submission_attempts (signature);

CREATE TABLE IF NOT EXISTS tx_finality_status (
    signature TEXT PRIMARY KEY,
    latest_slot BIGINT,
    confirmation_status TEXT NOT NULL,
    finalized BOOLEAN NOT NULL DEFAULT false,
    error_text TEXT,
    raw_status JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_tx_finality_status_confirmation ON tx_finality_status (confirmation_status, updated_at DESC);

CREATE TABLE IF NOT EXISTS keeper_jobs (
    job_name TEXT PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT true,
    schedule_kind TEXT NOT NULL,
    lease_key TEXT NOT NULL,
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    last_enqueued_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS keeper_job_runs (
    run_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_name TEXT NOT NULL,
    run_key TEXT,
    trigger_slot BIGINT,
    status TEXT NOT NULL,
    reason TEXT,
    input JSONB NOT NULL DEFAULT '{}'::jsonb,
    output JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_text TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    finished_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_keeper_job_runs_job ON keeper_job_runs (job_name, started_at DESC);

CREATE TABLE IF NOT EXISTS alerts (
    alert_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    alert_kind TEXT NOT NULL,
    severity TEXT NOT NULL,
    source TEXT NOT NULL,
    dedupe_key TEXT,
    status TEXT NOT NULL DEFAULT 'open',
    message TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_alerts_status_severity ON alerts (status, severity, created_at DESC);

CREATE TABLE IF NOT EXISTS admin_actions (
    action_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor TEXT NOT NULL,
    action_kind TEXT NOT NULL,
    target TEXT,
    request JSONB NOT NULL DEFAULT '{}'::jsonb,
    result JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_admin_actions_actor ON admin_actions (actor, created_at DESC);

CREATE TABLE IF NOT EXISTS config_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    config_kind TEXT NOT NULL,
    config_body JSONB NOT NULL DEFAULT '{}'::jsonb,
    activated_by TEXT NOT NULL,
    activated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_config_versions_kind ON config_versions (config_kind, activated_at DESC);

CREATE TABLE IF NOT EXISTS idempotency_keys (
    idempotency_key TEXT PRIMARY KEY,
    request_scope TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    response_body JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL,
    expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_idempotency_keys_scope ON idempotency_keys (request_scope, created_at DESC);
