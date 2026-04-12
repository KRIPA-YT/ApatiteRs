use async_trait::async_trait;
use serde_json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TwitchAPIError {
    #[error("Error while authenticating")]
    AuthError(#[from] AuthError),
    #[error("Couldn't send http request")]
    RequestError,
    #[error("Unexpected response")]
    ResponseError,
    #[error("Error while parsing response")]
    ParseError,
}

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Couldn't save token")]
    SaveError,
    #[error("Couldn't send http request")]
    RequestError,
    #[error("Couldn't parse http response")]
    ParseError,
}

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
