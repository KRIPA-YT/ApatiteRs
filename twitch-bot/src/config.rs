use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub auth: AuthConfig,
    pub bot: BotConfig,
}

#[derive(Deserialize)]
pub struct AuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub user_id: String,
}
#[derive(Deserialize)]
pub struct BotConfig {
    pub channel_id: String,
}
