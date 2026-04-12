use async_trait::async_trait;
use serde_json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TwitchAPIError {}

#[async_trait]
pub trait TwitchAPI: Send + Sync {
    async fn send_message(&self, message: &str) -> Result<(), TwitchAPIError>;

    async fn get_user_id_cached(&self, username: &str) -> Option<String>;

    async fn subscribe_to_event(
        &self,
        r#type: &str,
        version: &str,
        condition: serde_json::Value,
    ) -> Result<(), TwitchAPIError>;
}
