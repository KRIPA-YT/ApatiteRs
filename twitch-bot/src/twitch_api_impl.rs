mod auth;

use std::error::Error;

use crate::{
    config::AuthConfig,
    twitch_eventsub::{self, EventSubSocket},
    user_cache::{UserCache, load_user_cache, save_user_cache},
};
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
    ) -> Result<(EventSubSocket, TwitchAPI), ()> {
        // TODO Error types
        let token = auth::authenticate(&auth_config).await;
        let (ws, session_id) = twitch_eventsub::connect_eventsub()
            .await
            .expect("Couldn't connect to eventsub!");
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

#[async_trait]
impl apatite_api::twitch_api::TwitchAPI for TwitchAPI {
    async fn send_message(&self, message: &str) -> Result<(), Box<dyn Error>> {
        // TODO: Error types
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
            .await?;

        if !res.status().is_success() {
            todo!("Implement handler for non-success status code");
        }
        println!("Sent message: {}", message);
        Ok(())
    }

    async fn get_user_id_cached(&self, username: &str) -> Option<String> {
        // Try cache first
        if let Some(cache) = load_user_cache(username) {
            println!("Using cached user_id");
            return Some(cache.user_id);
        }

        // Fetch from Twitch API

        let res = self
            .twitch_get_request("users")
            .query(&[("login", username)])
            .send()
            .await
            .ok()?;

        let json: Value = res.json().await.ok()?;

        let user_id = json["data"].get(0)?.get("id")?.as_str()?.to_string();

        // Save cache as TOML
        save_user_cache(&UserCache {
            user_id: user_id.clone(),
            username: username.to_string(),
        });

        Some(user_id)
    }

    async fn subscribe_to_event(
        &self,
        r#type: &str,
        version: &str,
        condition: serde_json::Value,
    ) -> Result<(), Box<dyn Error>> {
        let body = json!({
            "type": r#type,
            "version": version,
            "condition": condition,
            "transport": {
                "method": "websocket",
                "session_id": &self.session_id
            }
        });

        self.twitch_post_request("eventsub/subscriptions")
            .json(&body)
            .send()
            .await
            .map(|_| ())
            .map_err(|e| e.into())
    }
}
