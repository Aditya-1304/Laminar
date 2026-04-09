use laminar_config::AppConfig;
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
}
