use laminar_config::AppConfig;
use sqlx::{postgres::PgRow, types::JsonValue, PgPool, Row};
use thiserror::Error;

use crate::{PostgresStore, PostgresStoreError, RedisStore, RedisStoreError};

#[derive(Debug, Clone)]
pub struct LaminarStores {
    pub postgres: PostgresStore,
    pub redis: RedisStore,
}

#[derive(Debug, Error)]
pub enum StoreBootstrapError {
    #[error(transparent)]
    Postgres(#[from] PostgresStoreError),
    #[error(transparent)]
    Redis(#[from] RedisStoreError),
}

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error("invalid numeric value in column `{column}`: {value}")]
    InvalidNumeric { column: &'static str, value: String },
}

impl LaminarStores {
    pub async fn connect(config: &AppConfig) -> Result<Self, StoreBootstrapError> {
        let postgres = PostgresStore::connect(&config.database_url).await?;
        let redis = RedisStore::connect(&config.redis_url)?;

        Ok(Self { postgres, redis })
    }

    pub async fn connect_and_migrate(config: &AppConfig) -> Result<Self, StoreBootstrapError> {
        let stores = Self::connect(config).await?;
        stores.postgres.run_migrations().await?;
        stores.ping().await?;
        Ok(stores)
    }

    pub async fn ping(&self) -> Result<(), StoreBootstrapError> {
        self.postgres.ping().await?;
        self.redis.ping().await?;
        Ok(())
    }

    pub fn ingestion_checkpoints(&self) -> IngestionCheckpointRepository {
        IngestionCheckpointRepository::new(self.postgres.pool())
    }

    pub fn global_state_current(&self) -> GlobalStateCurrentRepository {
        GlobalStateCurrentRepository::new(self.postgres.pool())
    }

    pub fn stability_pool_current(&self) -> StabilityPoolCurrentRepository {
        StabilityPoolCurrentRepository::new(self.postgres.pool())
    }

    pub fn unsigned_tx_requests(&self) -> UnsignedTxRequestRepository {
        UnsignedTxRequestRepository::new(self.postgres.pool())
    }

    pub fn keeper_jobs(&self) -> KeeperJobRepository {
        KeeperJobRepository::new(self.postgres.pool())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestionCheckpointRecord {
    pub stream_name: String,
    pub processed_slot: i64,
    pub confirmed_slot: i64,
    pub finalized_slot: i64,
    pub cursor: JsonValue,
    pub metadata: JsonValue,
}

#[derive(Debug, Clone)]
pub struct IngestionCheckpointRepository {
    pool: PgPool,
}

impl IngestionCheckpointRepository {
    pub fn new(pool: &PgPool) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn get(
        &self,
        stream_name: &str,
    ) -> Result<Option<IngestionCheckpointRecord>, RepositoryError> {
        let row = sqlx::query(
            r#"
            select
                stream_name,
                processed_slot,
                confirmed_slot,
                finalized_slot,
                cursor,
                metadata
            from ingestion_checkpoints
            where stream_name = $1
            "#,
        )
        .bind(stream_name)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_ingestion_checkpoint_row).transpose()
    }

    pub async fn upsert(&self, record: &IngestionCheckpointRecord) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            insert into ingestion_checkpoints (
                stream_name,
                processed_slot,
                confirmed_slot,
                finalized_slot,
                cursor,
                metadata
            ) values ($1, $2, $3, $4, $5, $6)
            on conflict (stream_name) do update set
                processed_slot = excluded.processed_slot,
                confirmed_slot = excluded.confirmed_slot,
                finalized_slot = excluded.finalized_slot,
                cursor = excluded.cursor,
                metadata = excluded.metadata,
                updated_at = now()
            "#,
        )
        .bind(&record.stream_name)
        .bind(record.processed_slot)
        .bind(record.confirmed_slot)
        .bind(record.finalized_slot)
        .bind(record.cursor.clone())
        .bind(record.metadata.clone())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalStateCurrentRecord {
    pub global_state_pubkey: String,
    pub projection_slot: i64,
    pub tx_signature: Option<String>,
    pub authority: String,
    pub amusd_mint: String,
    pub asol_mint: String,
    pub treasury: String,
    pub supported_lst_mint: String,
    pub total_lst_amount: u64,
    pub amusd_supply: u64,
    pub asol_supply: u64,
    pub min_cr_bps: u64,
    pub target_cr_bps: u64,
    pub mint_paused: bool,
    pub redeem_paused: bool,
    pub oracle_backend: String,
    pub lst_rate_backend: String,
    pub pyth_sol_usd_price_account: Option<String>,
    pub lst_stake_pool: Option<String>,
    pub rounding_reserve_lamports: u64,
    pub uncertainty_index_bps: u64,
    pub nav_floor_lamports: u64,
    pub max_asol_mint_per_round: u64,
    pub raw_model: JsonValue,
}

#[derive(Debug, Clone)]
pub struct GlobalStateCurrentRepository {
    pool: PgPool,
}

impl GlobalStateCurrentRepository {
    pub fn new(pool: &PgPool) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn get(
        &self,
        global_state_pubkey: &str,
    ) -> Result<Option<GlobalStateCurrentRecord>, RepositoryError> {
        let row = sqlx::query(
            r#"
            select
                global_state_pubkey,
                projection_slot,
                tx_signature,
                authority,
                amusd_mint,
                asol_mint,
                treasury,
                supported_lst_mint,
                total_lst_amount::text as total_lst_amount,
                amusd_supply::text as amusd_supply,
                asol_supply::text as asol_supply,
                min_cr_bps::text as min_cr_bps,
                target_cr_bps::text as target_cr_bps,
                mint_paused,
                redeem_paused,
                oracle_backend,
                lst_rate_backend,
                pyth_sol_usd_price_account,
                lst_stake_pool,
                rounding_reserve_lamports::text as rounding_reserve_lamports,
                uncertainty_index_bps::text as uncertainty_index_bps,
                nav_floor_lamports::text as nav_floor_lamports,
                max_asol_mint_per_round::text as max_asol_mint_per_round,
                raw_model
            from global_state_current
            where global_state_pubkey = $1
            "#,
        )
        .bind(global_state_pubkey)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_global_state_current_row).transpose()
    }

    pub async fn upsert(&self, record: &GlobalStateCurrentRecord) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            insert into global_state_current (
                global_state_pubkey,
                projection_slot,
                tx_signature,
                authority,
                amusd_mint,
                asol_mint,
                treasury,
                supported_lst_mint,
                total_lst_amount,
                amusd_supply,
                asol_supply,
                min_cr_bps,
                target_cr_bps,
                mint_paused,
                redeem_paused,
                oracle_backend,
                lst_rate_backend,
                pyth_sol_usd_price_account,
                lst_stake_pool,
                rounding_reserve_lamports,
                uncertainty_index_bps,
                nav_floor_lamports,
                max_asol_mint_per_round,
                raw_model
            ) values (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9::numeric,
                $10::numeric,
                $11::numeric,
                $12::numeric,
                $13::numeric,
                $14,
                $15,
                $16,
                $17,
                $18,
                $19,
                $20::numeric,
                $21::numeric,
                $22::numeric,
                $23::numeric,
                $24
            )
            on conflict (global_state_pubkey) do update set
                projection_slot = excluded.projection_slot,
                tx_signature = excluded.tx_signature,
                authority = excluded.authority,
                amusd_mint = excluded.amusd_mint,
                asol_mint = excluded.asol_mint,
                treasury = excluded.treasury,
                supported_lst_mint = excluded.supported_lst_mint,
                total_lst_amount = excluded.total_lst_amount,
                amusd_supply = excluded.amusd_supply,
                asol_supply = excluded.asol_supply,
                min_cr_bps = excluded.min_cr_bps,
                target_cr_bps = excluded.target_cr_bps,
                mint_paused = excluded.mint_paused,
                redeem_paused = excluded.redeem_paused,
                oracle_backend = excluded.oracle_backend,
                lst_rate_backend = excluded.lst_rate_backend,
                pyth_sol_usd_price_account = excluded.pyth_sol_usd_price_account,
                lst_stake_pool = excluded.lst_stake_pool,
                rounding_reserve_lamports = excluded.rounding_reserve_lamports,
                uncertainty_index_bps = excluded.uncertainty_index_bps,
                nav_floor_lamports = excluded.nav_floor_lamports,
                max_asol_mint_per_round = excluded.max_asol_mint_per_round,
                raw_model = excluded.raw_model,
                updated_at = now()
            "#,
        )
        .bind(&record.global_state_pubkey)
        .bind(record.projection_slot)
        .bind(&record.tx_signature)
        .bind(&record.authority)
        .bind(&record.amusd_mint)
        .bind(&record.asol_mint)
        .bind(&record.treasury)
        .bind(&record.supported_lst_mint)
        .bind(record.total_lst_amount.to_string())
        .bind(record.amusd_supply.to_string())
        .bind(record.asol_supply.to_string())
        .bind(record.min_cr_bps.to_string())
        .bind(record.target_cr_bps.to_string())
        .bind(record.mint_paused)
        .bind(record.redeem_paused)
        .bind(&record.oracle_backend)
        .bind(&record.lst_rate_backend)
        .bind(&record.pyth_sol_usd_price_account)
        .bind(&record.lst_stake_pool)
        .bind(record.rounding_reserve_lamports.to_string())
        .bind(record.uncertainty_index_bps.to_string())
        .bind(record.nav_floor_lamports.to_string())
        .bind(record.max_asol_mint_per_round.to_string())
        .bind(record.raw_model.clone())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StabilityPoolCurrentRecord {
    pub stability_pool_pubkey: String,
    pub global_state_pubkey: String,
    pub projection_slot: i64,
    pub tx_signature: Option<String>,
    pub samusd_mint: String,
    pub pool_amusd_vault: String,
    pub pool_asol_vault: String,
    pub total_amusd: u64,
    pub total_asol: u64,
    pub total_samusd: u64,
    pub stability_withdrawals_paused: bool,
    pub last_harvest_lst_to_sol_rate: u64,
    pub raw_model: JsonValue,
}

#[derive(Debug, Clone)]
pub struct StabilityPoolCurrentRepository {
    pool: PgPool,
}

impl StabilityPoolCurrentRepository {
    pub fn new(pool: &PgPool) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn get(
        &self,
        stability_pool_pubkey: &str,
    ) -> Result<Option<StabilityPoolCurrentRecord>, RepositoryError> {
        let row = sqlx::query(
            r#"
            select
                stability_pool_pubkey,
                global_state_pubkey,
                projection_slot,
                tx_signature,
                samusd_mint,
                pool_amusd_vault,
                pool_asol_vault,
                total_amusd::text as total_amusd,
                total_asol::text as total_asol,
                total_samusd::text as total_samusd,
                stability_withdrawals_paused,
                last_harvest_lst_to_sol_rate::text as last_harvest_lst_to_sol_rate,
                raw_model
            from stability_pool_current
            where stability_pool_pubkey = $1
            "#,
        )
        .bind(stability_pool_pubkey)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_stability_pool_current_row).transpose()
    }

    pub async fn upsert(&self, record: &StabilityPoolCurrentRecord) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            insert into stability_pool_current (
                stability_pool_pubkey,
                global_state_pubkey,
                projection_slot,
                tx_signature,
                samusd_mint,
                pool_amusd_vault,
                pool_asol_vault,
                total_amusd,
                total_asol,
                total_samusd,
                stability_withdrawals_paused,
                last_harvest_lst_to_sol_rate,
                raw_model
            ) values (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8::numeric,
                $9::numeric,
                $10::numeric,
                $11,
                $12::numeric,
                $13
            )
            on conflict (stability_pool_pubkey) do update set
                global_state_pubkey = excluded.global_state_pubkey,
                projection_slot = excluded.projection_slot,
                tx_signature = excluded.tx_signature,
                samusd_mint = excluded.samusd_mint,
                pool_amusd_vault = excluded.pool_amusd_vault,
                pool_asol_vault = excluded.pool_asol_vault,
                total_amusd = excluded.total_amusd,
                total_asol = excluded.total_asol,
                total_samusd = excluded.total_samusd,
                stability_withdrawals_paused = excluded.stability_withdrawals_paused,
                last_harvest_lst_to_sol_rate = excluded.last_harvest_lst_to_sol_rate,
                raw_model = excluded.raw_model,
                updated_at = now()
            "#,
        )
        .bind(&record.stability_pool_pubkey)
        .bind(&record.global_state_pubkey)
        .bind(record.projection_slot)
        .bind(&record.tx_signature)
        .bind(&record.samusd_mint)
        .bind(&record.pool_amusd_vault)
        .bind(&record.pool_asol_vault)
        .bind(record.total_amusd.to_string())
        .bind(record.total_asol.to_string())
        .bind(record.total_samusd.to_string())
        .bind(record.stability_withdrawals_paused)
        .bind(record.last_harvest_lst_to_sol_rate.to_string())
        .bind(record.raw_model.clone())
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewUnsignedTxRequest {
    pub idempotency_key: Option<String>,
    pub wallet_pubkey: String,
    pub request_kind: String,
    pub request_body: JsonValue,
    pub quote_summary: JsonValue,
    pub unsigned_tx_base64: Option<String>,
    pub required_signers: JsonValue,
    pub recent_blockhash: Option<String>,
    pub last_valid_block_height: Option<u64>,
    pub simulation_summary: JsonValue,
    pub expiry_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedTxRequestRecord {
    pub request_id: String,
    pub idempotency_key: Option<String>,
    pub wallet_pubkey: String,
    pub request_kind: String,
    pub request_body: JsonValue,
    pub quote_summary: JsonValue,
    pub unsigned_tx_base64: Option<String>,
    pub required_signers: JsonValue,
    pub recent_blockhash: Option<String>,
    pub last_valid_block_height: Option<u64>,
    pub simulation_summary: JsonValue,
    pub expiry_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UnsignedTxRequestRepository {
    pool: PgPool,
}

impl UnsignedTxRequestRepository {
    pub fn new(pool: &PgPool) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn create(
        &self,
        record: &NewUnsignedTxRequest,
    ) -> Result<UnsignedTxRequestRecord, RepositoryError> {
        let row = sqlx::query(
            r#"
            insert into unsigned_tx_requests (
                idempotency_key,
                wallet_pubkey,
                request_kind,
                request_body,
                quote_summary,
                unsigned_tx_base64,
                required_signers,
                recent_blockhash,
                last_valid_block_height,
                simulation_summary,
                expiry_at
            ) values (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8,
                $9::numeric,
                $10,
                $11::timestamptz
            )
            returning
                request_id::text as request_id,
                idempotency_key,
                wallet_pubkey,
                request_kind,
                request_body,
                quote_summary,
                unsigned_tx_base64,
                required_signers,
                recent_blockhash,
                last_valid_block_height::text as last_valid_block_height,
                simulation_summary,
                expiry_at::text as expiry_at
            "#,
        )
        .bind(&record.idempotency_key)
        .bind(&record.wallet_pubkey)
        .bind(&record.request_kind)
        .bind(record.request_body.clone())
        .bind(record.quote_summary.clone())
        .bind(&record.unsigned_tx_base64)
        .bind(record.required_signers.clone())
        .bind(&record.recent_blockhash)
        .bind(record.last_valid_block_height.map(|v| v.to_string()))
        .bind(record.simulation_summary.clone())
        .bind(&record.expiry_at)
        .fetch_one(&self.pool)
        .await?;

        map_unsigned_tx_request_row(row)
    }

    pub async fn get_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<UnsignedTxRequestRecord>, RepositoryError> {
        let row = sqlx::query(
            r#"
            select
                request_id::text as request_id,
                idempotency_key,
                wallet_pubkey,
                request_kind,
                request_body,
                quote_summary,
                unsigned_tx_base64,
                required_signers,
                recent_blockhash,
                last_valid_block_height::text as last_valid_block_height,
                simulation_summary,
                expiry_at::text as expiry_at
            from unsigned_tx_requests
            where idempotency_key = $1
            "#,
        )
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_unsigned_tx_request_row).transpose()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeeperJobRecord {
    pub job_name: String,
    pub enabled: bool,
    pub schedule_kind: String,
    pub lease_key: String,
    pub config: JsonValue,
    pub last_enqueued_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewKeeperJobRun {
    pub job_name: String,
    pub run_key: Option<String>,
    pub trigger_slot: Option<i64>,
    pub status: String,
    pub reason: Option<String>,
    pub input: JsonValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeeperJobRunRecord {
    pub run_id: String,
    pub job_name: String,
    pub run_key: Option<String>,
    pub trigger_slot: Option<i64>,
    pub status: String,
    pub reason: Option<String>,
    pub input: JsonValue,
    pub output: JsonValue,
    pub error_text: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct KeeperJobRepository {
    pool: PgPool,
}

impl KeeperJobRepository {
    pub fn new(pool: &PgPool) -> Self {
        Self { pool: pool.clone() }
    }

    pub async fn get_job(
        &self,
        job_name: &str,
    ) -> Result<Option<KeeperJobRecord>, RepositoryError> {
        let row = sqlx::query(
            r#"
            select
                job_name,
                enabled,
                schedule_kind,
                lease_key,
                config,
                last_enqueued_at::text as last_enqueued_at
            from keeper_jobs
            where job_name = $1
            "#,
        )
        .bind(job_name)
        .fetch_optional(&self.pool)
        .await?;

        row.map(map_keeper_job_row).transpose()
    }

    pub async fn upsert_job(&self, record: &KeeperJobRecord) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            insert into keeper_jobs (
                job_name,
                enabled,
                schedule_kind,
                lease_key,
                config,
                last_enqueued_at
            ) values (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6::timestamptz
            )
            on conflict (job_name) do update set
                enabled = excluded.enabled,
                schedule_kind = excluded.schedule_kind,
                lease_key = excluded.lease_key,
                config = excluded.config,
                last_enqueued_at = excluded.last_enqueued_at,
                updated_at = now()
            "#,
        )
        .bind(&record.job_name)
        .bind(record.enabled)
        .bind(&record.schedule_kind)
        .bind(&record.lease_key)
        .bind(record.config.clone())
        .bind(&record.last_enqueued_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_run(
        &self,
        record: &NewKeeperJobRun,
    ) -> Result<KeeperJobRunRecord, RepositoryError> {
        let row = sqlx::query(
            r#"
            insert into keeper_job_runs (
                job_name,
                run_key,
                trigger_slot,
                status,
                reason,
                input
            ) values ($1, $2, $3, $4, $5, $6)
            returning
                run_id::text as run_id,
                job_name,
                run_key,
                trigger_slot,
                status,
                reason,
                input,
                output,
                error_text,
                started_at::text as started_at,
                finished_at::text as finished_at
            "#,
        )
        .bind(&record.job_name)
        .bind(&record.run_key)
        .bind(record.trigger_slot)
        .bind(&record.status)
        .bind(&record.reason)
        .bind(record.input.clone())
        .fetch_one(&self.pool)
        .await?;

        map_keeper_job_run_row(row)
    }

    pub async fn finish_run(
        &self,
        run_id: &str,
        status: &str,
        output: JsonValue,
        error_text: Option<&str>,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            update keeper_job_runs
            set
                status = $2,
                output = $3,
                error_text = $4,
                finished_at = now()
            where run_id = $1::uuid
            "#,
        )
        .bind(run_id)
        .bind(status)
        .bind(output)
        .bind(error_text)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

fn map_ingestion_checkpoint_row(row: PgRow) -> Result<IngestionCheckpointRecord, RepositoryError> {
    Ok(IngestionCheckpointRecord {
        stream_name: row.try_get("stream_name")?,
        processed_slot: row.try_get("processed_slot")?,
        confirmed_slot: row.try_get("confirmed_slot")?,
        finalized_slot: row.try_get("finalized_slot")?,
        cursor: row.try_get("cursor")?,
        metadata: row.try_get("metadata")?,
    })
}

fn map_global_state_current_row(row: PgRow) -> Result<GlobalStateCurrentRecord, RepositoryError> {
    Ok(GlobalStateCurrentRecord {
        global_state_pubkey: row.try_get("global_state_pubkey")?,
        projection_slot: row.try_get("projection_slot")?,
        tx_signature: row.try_get("tx_signature")?,
        authority: row.try_get("authority")?,
        amusd_mint: row.try_get("amusd_mint")?,
        asol_mint: row.try_get("asol_mint")?,
        treasury: row.try_get("treasury")?,
        supported_lst_mint: row.try_get("supported_lst_mint")?,
        total_lst_amount: parse_u64_column("total_lst_amount", row.try_get("total_lst_amount")?)?,
        amusd_supply: parse_u64_column("amusd_supply", row.try_get("amusd_supply")?)?,
        asol_supply: parse_u64_column("asol_supply", row.try_get("asol_supply")?)?,
        min_cr_bps: parse_u64_column("min_cr_bps", row.try_get("min_cr_bps")?)?,
        target_cr_bps: parse_u64_column("target_cr_bps", row.try_get("target_cr_bps")?)?,
        mint_paused: row.try_get("mint_paused")?,
        redeem_paused: row.try_get("redeem_paused")?,
        oracle_backend: row.try_get("oracle_backend")?,
        lst_rate_backend: row.try_get("lst_rate_backend")?,
        pyth_sol_usd_price_account: row.try_get("pyth_sol_usd_price_account")?,
        lst_stake_pool: row.try_get("lst_stake_pool")?,
        rounding_reserve_lamports: parse_u64_column(
            "rounding_reserve_lamports",
            row.try_get("rounding_reserve_lamports")?,
        )?,
        uncertainty_index_bps: parse_u64_column(
            "uncertainty_index_bps",
            row.try_get("uncertainty_index_bps")?,
        )?,
        nav_floor_lamports: parse_u64_column(
            "nav_floor_lamports",
            row.try_get("nav_floor_lamports")?,
        )?,
        max_asol_mint_per_round: parse_u64_column(
            "max_asol_mint_per_round",
            row.try_get("max_asol_mint_per_round")?,
        )?,
        raw_model: row.try_get("raw_model")?,
    })
}

fn map_stability_pool_current_row(
    row: PgRow,
) -> Result<StabilityPoolCurrentRecord, RepositoryError> {
    Ok(StabilityPoolCurrentRecord {
        stability_pool_pubkey: row.try_get("stability_pool_pubkey")?,
        global_state_pubkey: row.try_get("global_state_pubkey")?,
        projection_slot: row.try_get("projection_slot")?,
        tx_signature: row.try_get("tx_signature")?,
        samusd_mint: row.try_get("samusd_mint")?,
        pool_amusd_vault: row.try_get("pool_amusd_vault")?,
        pool_asol_vault: row.try_get("pool_asol_vault")?,
        total_amusd: parse_u64_column("total_amusd", row.try_get("total_amusd")?)?,
        total_asol: parse_u64_column("total_asol", row.try_get("total_asol")?)?,
        total_samusd: parse_u64_column("total_samusd", row.try_get("total_samusd")?)?,
        stability_withdrawals_paused: row.try_get("stability_withdrawals_paused")?,
        last_harvest_lst_to_sol_rate: parse_u64_column(
            "last_harvest_lst_to_sol_rate",
            row.try_get("last_harvest_lst_to_sol_rate")?,
        )?,
        raw_model: row.try_get("raw_model")?,
    })
}

fn map_unsigned_tx_request_row(row: PgRow) -> Result<UnsignedTxRequestRecord, RepositoryError> {
    Ok(UnsignedTxRequestRecord {
        request_id: row.try_get("request_id")?,
        idempotency_key: row.try_get("idempotency_key")?,
        wallet_pubkey: row.try_get("wallet_pubkey")?,
        request_kind: row.try_get("request_kind")?,
        request_body: row.try_get("request_body")?,
        quote_summary: row.try_get("quote_summary")?,
        unsigned_tx_base64: row.try_get("unsigned_tx_base64")?,
        required_signers: row.try_get("required_signers")?,
        recent_blockhash: row.try_get("recent_blockhash")?,
        last_valid_block_height: parse_optional_u64_column(
            "last_valid_block_height",
            row.try_get("last_valid_block_height")?,
        )?,
        simulation_summary: row.try_get("simulation_summary")?,
        expiry_at: row.try_get("expiry_at")?,
    })
}

fn map_keeper_job_row(row: PgRow) -> Result<KeeperJobRecord, RepositoryError> {
    Ok(KeeperJobRecord {
        job_name: row.try_get("job_name")?,
        enabled: row.try_get("enabled")?,
        schedule_kind: row.try_get("schedule_kind")?,
        lease_key: row.try_get("lease_key")?,
        config: row.try_get("config")?,
        last_enqueued_at: row.try_get("last_enqueued_at")?,
    })
}

fn map_keeper_job_run_row(row: PgRow) -> Result<KeeperJobRunRecord, RepositoryError> {
    Ok(KeeperJobRunRecord {
        run_id: row.try_get("run_id")?,
        job_name: row.try_get("job_name")?,
        run_key: row.try_get("run_key")?,
        trigger_slot: row.try_get("trigger_slot")?,
        status: row.try_get("status")?,
        reason: row.try_get("reason")?,
        input: row.try_get("input")?,
        output: row.try_get("output")?,
        error_text: row.try_get("error_text")?,
        started_at: row.try_get("started_at")?,
        finished_at: row.try_get("finished_at")?,
    })
}

fn parse_u64_column(column: &'static str, value: String) -> Result<u64, RepositoryError> {
    value
        .parse::<u64>()
        .map_err(|_| RepositoryError::InvalidNumeric { column, value })
}

fn parse_optional_u64_column(
    column: &'static str,
    value: Option<String>,
) -> Result<Option<u64>, RepositoryError> {
    match value {
        Some(value) => parse_u64_column(column, value).map(Some),
        None => Ok(None),
    }
}
