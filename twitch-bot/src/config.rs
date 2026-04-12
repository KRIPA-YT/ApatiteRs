use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct Config {
    pub auth: AuthConfig,
    pub bot: BotConfig,
}

#[derive(Serialize, Deserialize)]
pub struct AuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub user_id: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            client_id: "Client ID here".to_owned(),
            client_secret: "Client secret here".to_owned(),
            redirect_uri: "http://localhost:3000/callback".to_owned(),
            user_id: "Bot user ID here".to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct BotConfig {
    pub channel_id: String,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            channel_id: "Channel ID here".to_owned(),
        }
    }
}
