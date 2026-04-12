use apatite_api::{Bot, Command, CommandError, CommandRegistrar, Context};
use async_trait::async_trait;
use std::collections::HashMap;

pub struct CommandHandler {
    commands: HashMap<String, Box<dyn Command>>,
}

impl CommandHandler {
    pub fn new() -> Self {
        let mut h = Self {
            commands: HashMap::new(),
        };
        h.register(Box::new(HelpCommand));
        h.register(Box::new(StopCommand));
        h
    }
}

impl CommandRegistrar for CommandHandler {
    fn register(&mut self, cmd: Box<dyn Command>) {
        self.commands.insert(cmd.name().to_string(), cmd);
    }
}

#[async_trait]
impl apatite_api::CommandHandler for CommandHandler {
    async fn handle(
        &mut self,
        msg: &str,
        user: String,
        bot: &mut dyn Bot,
    ) -> Result<(), CommandError> {
        let mut parts = msg[1..].split_whitespace();
        let name = parts.next().expect("NO NAME PANICCC");
        let args = parts.map(|s| s.to_string()).collect();

        let command = self.commands.get(name).expect("Unknown command");
        command
            .execute(&mut Context {
                user,
                message: msg.into(),
                args,
                commands: &self.commands,
                bot,
            })
            .await
    }
}

pub struct HelpCommand;

#[async_trait::async_trait]
impl Command for HelpCommand {
    fn name(&self) -> &'static str {
        "help"
    }
    fn help(&self) -> &'static str {
        "Lists all commands"
    }

    async fn execute(&self, ctx: &mut Context) -> Result<(), CommandError> {
        let help_text = ctx
            .commands
            .values()
            .map(|c| format!("!{} - {}", c.name(), c.help()))
            .collect::<Vec<_>>()
            .join(" | ");

        ctx.bot.twitch_api().send_message(&help_text).await?;
        Ok(())
    }
}

pub struct StopCommand;

#[async_trait]
impl Command for StopCommand {
    fn name(&self) -> &'static str {
        "stop"
    }
    fn help(&self) -> &'static str {
        "Stops the bot"
    }

    async fn execute(&self, ctx: &mut Context) -> Result<(), CommandError> {
        ctx.bot.twitch_api().send_message("Stopping...").await?;
        ctx.bot.stop().await?;
        Ok(())
    }
}
