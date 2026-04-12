mod config;
mod handler;
mod plugin_loader;
mod twitch_api_impl;
mod twitch_eventsub;
mod user_cache;

use apatite_api::{Bot, CommandHandler, twitch_api::TwitchAPI as _};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::{error::Error, fs};

use crate::{
    config::{BotConfig, Config},
    twitch_api_impl::TwitchAPI,
    twitch_eventsub::EventSubSocket,
};

pub struct Apatite {
    state: ApatiteState,
    config: Config,
    web_socket: EventSubSocket,
    handler: Box<dyn CommandHandler>,
}

pub struct ApatiteState {
    running: bool,
    twitch_api: Box<dyn apatite_api::twitch_api::TwitchAPI>,
}

impl Apatite {
    async fn new(config: Config) -> Self {
        let mut handler = handler::CommandHandler::new();
        plugin_loader::load_plugins(&mut handler);

        let (web_socket, twitch_api) =
            TwitchAPI::connect(&config.auth, config.bot.channel_id.to_owned())
                .await
                .expect("Couldn't connect to twitch!");

        subscribe_to_events(&twitch_api, &config.bot).await.unwrap();
        Self {
            config,
            state: ApatiteState {
                running: true,
                twitch_api: Box::new(twitch_api),
            },
            web_socket,
            handler: Box::new(handler),
        }
    }

    async fn run(&mut self) {
        while self.state.running
            && let Some(msg) = self.web_socket.next().await
        {
            let msg = msg.unwrap();

            if msg.is_text() {
                let data: Value = serde_json::from_str(msg.to_text().unwrap()).unwrap();

                match data["metadata"]["message_type"].as_str() {
                    Some("notification") => {
                        if let Some(event) = data["payload"]["event"].as_object() {
                            let msg = event["message"]["text"].as_str().unwrap_or("");
                            let user = event["chatter_user_name"].as_str().unwrap_or("unknown");
                            let user_id = match event["chatter_user_id"].as_str() {
                                Some(user_id) => user_id,
                                None => {
                                    println!("couldn't find user_id!");
                                    continue;
                                }
                            };

                            println!("[{}] {}", user, msg);

                            if user_id != &self.config.auth.user_id && msg.starts_with("!") {
                                // TODO: graceful handling
                                self.handler
                                    .handle(msg, user.to_string(), &mut self.state)
                                    .await
                                    .expect("Error while executing command!");
                            }
                        }
                    }

                    Some("session_keepalive") => {}
                    _ => {}
                }
            }
        }
    }
}

#[async_trait]
impl Bot for ApatiteState {
    async fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        println!("Stopping...");
        self.running = false;
        Ok(())
    }

    fn twitch_api(&mut self) -> &mut dyn apatite_api::twitch_api::TwitchAPI {
        self.twitch_api.as_mut()
    }
}

#[tokio::main]
async fn main() {
    let config_str = fs::read_to_string("config.toml").unwrap();
    let config: Config = toml::from_str(&config_str).unwrap();

    let mut apatite = Apatite::new(config).await;
    apatite.run().await;

    println!("Exiting");
}

async fn subscribe_to_events(
    twitch_api: &TwitchAPI,
    bot_config: &BotConfig,
) -> Result<(), Box<dyn Error>> {
    twitch_api
        .subscribe_to_event(
            "channel.chat.message",
            "1",
            json! {
                {
                "broadcaster_user_id": bot_config.channel_id,
                "user_id": &twitch_api.bot_user_id,
                }
            },
        )
        .await
}
