pub mod twitch_api;
use std::{collections::HashMap, error::Error};

use async_trait::async_trait;

use crate::twitch_api::TwitchAPI;

pub const API_VERSION: u32 = 1;

pub struct Context<'ctx> {
    pub user: String,
    pub message: String,
    pub args: Vec<String>,
    pub commands: &'ctx HashMap<String, Box<dyn Command>>,
    pub bot: &'ctx mut dyn Bot,
}

#[async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &'static str;
    fn help(&self) -> &'static str;
    async fn execute(&self, ctx: &mut Context) -> Result<(), Box<dyn Error>>;
}

// What plugins must export
pub struct PluginDeclaration {
    pub api_version: u32,
    pub register: unsafe fn(&mut dyn CommandRegistrar),
}

pub trait CommandRegistrar {
    fn register(&mut self, cmd: Box<dyn Command>);
}

#[async_trait]
pub trait CommandHandler: Send + Sync {
    async fn handle(
        &mut self,
        msg: &str,
        user: String,
        bot: &mut dyn Bot,
    ) -> Result<(), Box<dyn Error>>;
}

#[async_trait]
pub trait Bot: Send + Sync {
    async fn stop(&mut self) -> Result<(), Box<dyn Error>>;
    fn twitch_api(&mut self) -> &mut dyn TwitchAPI;
}
