mod auth;

use crate::{
    config::AuthConfig,
    twitch_eventsub::{self, EventSubSocket},
    user_cache::{UserCache, load_user_cache, save_user_cache},
};
use apatite_api::twitch_api::TwitchAPIError;
use async_trait::async_trait;
use reqwest::{Client, RequestBuilder};
use serde_json::{Value, json};

pub struct TwitchAPI {
    token: String,
    client_id: String,
    session_id: String,
    pub bot_user_id: String,
    pub broadcaster_id: String,
}

impl TwitchAPI {
    pub async fn connect(
        auth_config: &AuthConfig,
        broadcaster_id: String,
    ) -> Result<(EventSubSocket, TwitchAPI), TwitchAPIError> {
        let token = auth::authenticate(auth_config).await?;
        let (ws, session_id) = twitch_eventsub::connect_eventsub().await?;
        Ok((
            ws,
            TwitchAPI {
                token: token.access_token,
                client_id: auth_config.client_id.to_owned(),
                session_id,
                bot_user_id: auth_config.user_id.to_owned(),
                broadcaster_id,
            },
        ))
    }

    fn twitch_get_request(&self, endpoint: &str) -> RequestBuilder {
        let client = Client::new();

        client
            .get(format!("https://api.twitch.tv/helix/{}", endpoint))
            .bearer_auth(&self.token)
            .header("Client-Id", &self.client_id)
    }

    fn twitch_post_request(&self, endpoint: &str) -> RequestBuilder {
        let client = Client::new();

        client
            .post(format!("https://api.twitch.tv/helix/{}", endpoint))
            .bearer_auth(&self.token)
            .header("Client-Id", &self.client_id)
    }
}

const IDENTICAL_MSG_CHAR: char = '\u{34f}';

#[async_trait]
impl apatite_api::twitch_api::TwitchAPI for TwitchAPI {
    async fn send_message(&self, message: &str) -> Result<(), TwitchAPIError> {
        // TODO: Queue for ratelimiting
        let body = json!(
            {
                "broadcaster_id": &self.broadcaster_id,
                "sender_id": &self.bot_user_id,
                "message": message
            }
        );

        let res = self
            .twitch_post_request("chat/messages")
            .json(&body)
            .send()
            .await
            .map_err(|_| TwitchAPIError::RequestError)?;

        if !res.status().is_success() {
            println!("StatusCode not 200: {}", res.status());
            return Err(TwitchAPIError::RequestError);
        }

        let json: Value = res.json().await.map_err(|_| TwitchAPIError::ParseError)?;

        let message_sent = json["data"]["is_sent"]
            .as_bool()
            .ok_or(TwitchAPIError::ParseError)?;
        let msg_duplicate = json["data"]["drop_reason"]["code"]
            .as_str()
            .ok_or(TwitchAPIError::ParseError)?
            == "msg_deplicate";

        // This does not work because we're getting 429 rate limited...
        if !message_sent {
            if !msg_duplicate {
                return Err(TwitchAPIError::ResponseError);
            }
            let message = if message.ends_with(IDENTICAL_MSG_CHAR) {
                message.trim_end_matches(IDENTICAL_MSG_CHAR).to_string()
            } else {
                let mut s = message.to_string();
                s.push(IDENTICAL_MSG_CHAR);
                s
            };
            self.send_message(&message).await?;
        }

        Ok(())
    }

    async fn get_user_id_cached(&self, username: &str) -> Option<String> {
        // Try cache first
        if let Some(cache) = load_user_cache(username) {
            println!("Using cached user_id");
            return Some(cache.user_id);
        }

        let res = self
            .twitch_get_request("users")
            .query(&[("login", username)])
            .send()
            .await
            .ok()?;

        let json: Value = res.json().await.ok()?;

        let user_id = json["data"].get(0)?.get("id")?.as_str()?.to_string();

        // Save cache as TOML
        match save_user_cache(&UserCache {
            user_id: user_id.clone(),
            username: username.to_string(),
        }) {
            Ok(_) => (),
            Err(_) => println!("Couldn't save user cache!"),
        }

        Some(user_id)
    }

    async fn subscribe_to_event(
        &self,
        r#type: &str,
        version: &str,
        condition: serde_json::Value,
    ) -> Result<(), TwitchAPIError> {
        let body = json!({
            "type": r#type,
            "version": version,
            "condition": condition,
            "transport": {
                "method": "websocket",
                "session_id": &self.session_id
            }
        });

        let res = self
            .twitch_post_request("eventsub/subscriptions")
            .json(&body)
            .send()
            .await
            .map_err(|_| TwitchAPIError::RequestError)?;
        if !res.status().is_success() {
            return Err(TwitchAPIError::RequestError);
        }
        Ok(())
    }
}
