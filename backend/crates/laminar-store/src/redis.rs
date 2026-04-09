use redis::Client;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct RedisStore {
    client: Client,
}

#[derive(Debug, Error)]
pub enum RedisStoreError {
    #[error(transparent)]
    Redis(#[from] redis::RedisError),
    #[error("unexpected redis ping response: {0}")]
    UnexpectedPing(String),
}

impl RedisStore {
    pub fn connect(redis_url: &str) -> Result<Self, RedisStoreError> {
        Ok(Self {
            client: Client::open(redis_url)?,
        })
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn ping(&self) -> Result<(), RedisStoreError> {
        let mut connection = self.client.get_multiplexed_async_connection().await?;
        let response: String = redis::cmd("PING").query_async(&mut connection).await?;

        if response == "PONG" {
            Ok(())
        } else {
            Err(RedisStoreError::UnexpectedPing(response))
        }
    }
}
