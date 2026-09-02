mod commands;
mod handler;
mod lang;
pub mod playlist;
mod queue;
pub mod source;
pub mod ai;
mod utils;

use handler::Handler;
use queue::QueueManager;
use serenity::prelude::*;
use songbird::SerenityInit;
use source::SourceManager;
use std::env;
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    // Load .env file if present
    let _ = dotenvy::dotenv();

    // Initialize structured logging
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,songbird=info,serenity=warn")),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Discord Music Bot (Rust + Songbird)...");

    let token = env::var("DISCORD_BOT_TOKEN")
        .expect("Expected DISCORD_BOT_TOKEN in environment variables");

    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::non_privileged();

    let source_mgr = Arc::new(SourceManager::new());
    let playlist_store = Arc::new(playlist::PlaylistStore::init().await);
    let queue_mgr = Arc::new(QueueManager::new(Some(playlist_store.clone())));

    let handler = Handler {
        source_mgr,
        queue_mgr,
        playlist_store,
    };

    let songbird_config = songbird::Config::default().preallocated_tracks(1);
    let songbird_voice = songbird::Songbird::serenity_from_config(songbird_config);

    let mut client = Client::builder(&token, intents)
        .event_handler(handler)
        .register_songbird_with(songbird_voice)
        .await
        .expect("Error creating Discord client");

    info!("Connecting to Discord Gateway...");
    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}
