use std::env;

use crate::env::load_dotenv;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub app_env: String,
    pub app_name: String,
    pub rust_log: String,
    pub api_bind_addr: String,
    pub indexer_bind_addr: String,
    pub keeper_bind_addr: String,
    pub executor_bind_addr: String,
    pub database_url: String,
    pub redis_url: String,
    pub solana_cluster: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing environment variable: {0}")]
    MissingVar(&'static str),
}

impl AppConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        load_dotenv();

        Ok(Self {
            app_env: read_var("APP_ENV")?,
            app_name: read_var("APP_NAME")?,
            rust_log: read_var("RUST_LOG")?,
            api_bind_addr: read_var("API_BIND_ADDR")?,
            indexer_bind_addr: read_var("INDEXER_BIND_ADDR")?,
            keeper_bind_addr: read_var("KEEPER_BIND_ADDR")?,
            executor_bind_addr: read_var("EXECUTOR_BIND_ADDR")?,
            database_url: read_var("DATABASE_URL")?,
            redis_url: read_var("REDIS_URL")?,
            solana_cluster: read_var("SOLANA_CLUSTER")?,
        })
    }
}

fn read_var(key: &'static str) -> Result<String, ConfigError> {
    env::var(key).map_err(|_| ConfigError::MissingVar(key))
}
