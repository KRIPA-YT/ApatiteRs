#![deny(clippy::expect_used, clippy::unwrap_used, clippy::todo)]

mod config;
mod handler;
mod plugin_loader;
mod twitch_api_impl;
mod twitch_eventsub;
mod user_cache;

use apatite_api::{
    Bot, CommandHandler,
    twitch_api::{TwitchAPI as _, TwitchAPIError},
};
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
    async fn new(config: Config) -> Result<Self, TwitchAPIError> {
        let mut handler = handler::CommandHandler::new();
        plugin_loader::load_plugins(&mut handler);

        let (web_socket, twitch_api) =
            TwitchAPI::connect(&config.auth, config.bot.channel_id.to_owned()).await?;

        subscribe_to_events(&twitch_api, &config.bot).await?;
        Ok(Self {
            config,
            state: ApatiteState {
                running: true,
                twitch_api: Box::new(twitch_api),
            },
            web_socket,
            handler: Box::new(handler),
        })
    }

    async fn run(&mut self) {
        while self.state.running
            && let Some(msg) = self.web_socket.next().await
        {
            let msg = match msg {
                Ok(msg) => msg,
                Err(_) => continue,
            };

            if msg.is_text() {
                let data: Value = match serde_json::from_str(match msg.to_text() {
                    Ok(text) => text,
                    Err(_) => {
                        println!("Couldn't convert websocket message to text!");
                        continue;
                    }
                }) {
                    Ok(value) => value,
                    Err(_) => {
                        println!("Couldn't parse to json!");
                        continue;
                    }
                };

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

                            if user_id != self.config.auth.user_id && msg.starts_with("!") {
                                match self
                                    .handler
                                    .handle(msg, user.to_string(), &mut self.state)
                                    .await
                                {
                                    Ok(_) => (),
                                    Err(err) => {
                                        println!("Error: {:?}", err);
                                        let _ = self
                                            .state
                                            .twitch_api
                                            .send_message(
                                                "An error occured while executing this command!",
                                            )
                                            .await;
                                    }
                                }
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
#[allow(clippy::expect_used)]
async fn main() {
    let config_str = match fs::read_to_string("config.toml") {
        Ok(config_str) => config_str,
        Err(_) => {
            fs::write(
                "config.toml",
                toml::to_string_pretty(&Config::default()).expect("Couldn't write config.toml"),
            )
            .expect("Couldn't write config.toml");
            return;
        }
    };
    let config: Config = toml::from_str(&config_str).expect("Couldn't parse config.toml");

    let mut apatite = Apatite::new(config).await.expect("Couldn't create bot!");
    apatite.run().await;

    println!("Exiting");
}

async fn subscribe_to_events(
    twitch_api: &TwitchAPI,
    bot_config: &BotConfig,
) -> Result<(), TwitchAPIError> {
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
