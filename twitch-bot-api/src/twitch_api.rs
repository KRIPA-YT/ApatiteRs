use async_trait::async_trait;
use serde_json;
use std::error::Error;

#[async_trait]
pub trait TwitchAPI: Send + Sync {
    async fn send_message(&self, message: &str) -> Result<(), Box<dyn Error>>;

    async fn get_user_id_cached(&self, username: &str) -> Option<String>;

    async fn subscribe_to_event(
        &self,
        r#type: &str,
        version: &str,
        condition: serde_json::Value,
    ) -> Result<(), Box<dyn Error>>;
}
