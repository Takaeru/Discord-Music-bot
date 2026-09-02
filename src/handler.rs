use serenity::all::{
    async_trait, Context, EventHandler, Interaction, Ready,
};
use std::sync::Arc;
use tracing::{error, info};

use crate::commands::{handle_command, handle_component, register_commands};
use crate::playlist::PlaylistStore;
use crate::queue::QueueManager;
use crate::source::SourceManager;

pub struct Handler {
    pub source_mgr: Arc<SourceManager>,
    pub queue_mgr: Arc<QueueManager>,
    pub playlist_store: Arc<PlaylistStore>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Bot is ready and connected as {}", ready.user.tag());

        info!("Starting idle monitor task...");
        crate::utils::voice::start_idle_monitor(ctx.clone(), self.queue_mgr.clone());

        info!("Registering global slash commands...");
        let commands = register_commands();
        match ctx.http.create_global_commands(&commands).await {
            Ok(cmds) => {
                info!("Successfully registered {} global slash commands", cmds.len());
            }
            Err(why) => {
                error!("Failed to register global slash commands: {:?}", why);
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
                info!("Handling slash command: /{}", command.data.name);
                handle_command(
                    &ctx,
                    &command,
                    &self.source_mgr,
                    &self.queue_mgr,
                    &self.playlist_store,
                )
                .await;
            }
            Interaction::Component(component) => {
                info!("Handling component interaction: {}", component.data.custom_id);
                handle_component(&ctx, &component, &self.source_mgr, &self.queue_mgr).await;
            }
            _ => {}
        }
    }
}
