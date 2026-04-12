mod auth;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{
    config::AuthConfig,
    twitch_eventsub::{self, EventSubSocket},
    user_cache::{UserCache, load_user_cache, save_user_cache},
};
use apatite_api::twitch_api::TwitchAPIError;
use async_trait::async_trait;
use reqwest::{Client, Request, RequestBuilder, Response, StatusCode};
use serde_json::{Value, json};

pub struct TwitchAPIRequest {
    pub request: Request,
    pub ratelimit_resend: bool,
}

impl TwitchAPIRequest {
    pub fn from_request(request: Request, ratelimit_resend: bool) -> Self {
        Self {
            request,
            ratelimit_resend,
        }
    }

    pub async fn send(&mut self) -> Result<Response, TwitchAPIError> {
        let res = Client::new()
            .execute(self.request.try_clone().unwrap())
            .await
            .map_err(|_| TwitchAPIError::RequestError)?;
        if res.status().is_success() {
            return Ok(res);
        }
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            if !self.ratelimit_resend {
                return Err(TwitchAPIError::RateLimited);
            }
            let ratelimit_reset = res.headers()["ratelimit-reset"]
                .to_str()
                .map_err(|_| TwitchAPIError::ResponseError)?;
            let target = UNIX_EPOCH
                + Duration::from_secs(
                    ratelimit_reset
                        .parse()
                        .map_err(|_| TwitchAPIError::ParseError)?,
                );
            let now = SystemTime::now();

            let duration = match target.duration_since(now) {
                Ok(dur) => dur,
                Err(_) => Duration::from_secs(0), // already in the past
            };

            tokio::time::sleep(duration).await;
            return Box::pin(self.send()).await;
        }
        Err(TwitchAPIError::RequestError)
    }
}

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

    fn get(&self, endpoint: &str) -> RequestBuilder {
        let client = Client::new();

        client
            .get(format!("https://api.twitch.tv/helix/{}", endpoint))
            .bearer_auth(&self.token)
            .header("Client-Id", &self.client_id)
    }

    fn post(&self, endpoint: &str) -> RequestBuilder {
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
        let body = json!(
            {
                "broadcaster_id": &self.broadcaster_id,
                "sender_id": &self.bot_user_id,
                "message": message
            }
        );

        let req = self.post("chat/messages").json(&body);
        let res = TwitchAPIRequest::from_request(req.build().unwrap(), true)
            .send()
            .await?;

        let json: Value = res.json().await.map_err(|_| TwitchAPIError::ParseError)?;

        let message_sent = json["data"][0]["is_sent"]
            .as_bool()
            .ok_or(TwitchAPIError::ParseError)?;

        if !message_sent {
            let drop_code = json["data"][0]["drop_reason"]["code"]
                .as_str()
                .unwrap_or("");
            if drop_code == "msg_rejected" {
                return Err(TwitchAPIError::PermissionError);
            }

            if drop_code != "msg_duplicate" {
                return Err(TwitchAPIError::ResponseError);
            }

            let message = if message.ends_with(IDENTICAL_MSG_CHAR) {
                message.trim_end_matches(IDENTICAL_MSG_CHAR).to_string()
            } else {
                let mut s = message.to_string();
                s.push(IDENTICAL_MSG_CHAR);
                s
            };
            println!("Recalling...");
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

        let req = self.get("users").query(&[("login", username)]);
        let res = TwitchAPIRequest::from_request(req.build().unwrap(), true)
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

        let req = self.post("eventsub/subscriptions").json(&body);
        let _ = TwitchAPIRequest::from_request(req.build().unwrap(), true)
            .send()
            .await
            .map_err(|_| TwitchAPIError::RequestError)?;
        Ok(())
    }
}
