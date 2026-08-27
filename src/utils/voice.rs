use serenity::all::{ChannelId, Context, GuildId, UserId};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info};

use crate::queue::QueueManager;

pub fn check_voice_channel(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
) -> Result<ChannelId, &'static str> {
    let bot_id = ctx.cache.current_user().id;
    let (user_vc, bot_vc) = match ctx.cache.guild(guild_id) {
        Some(guild) => {
            let u_vc = guild.voice_states.get(&user_id).and_then(|vs| vs.channel_id);
            let b_vc = guild.voice_states.get(&bot_id).and_then(|vs| vs.channel_id);
            (u_vc, b_vc)
        }
        None => return Err("❌ Server cache not found."),
    };

    let user_channel = match user_vc {
        Some(c) => c,
        None => return Err("⚠️ You must be in a voice channel to use this command."),
    };

    if let Some(bot_channel) = bot_vc {
        if user_channel != bot_channel {
            return Err("⚠️ You must be in the same voice channel as the bot to use this command.");
        }
    }

    Ok(user_channel)
}

pub fn start_idle_monitor(ctx: Context, queue_mgr: Arc<QueueManager>) {
    tokio::spawn(async move {
        let mut idle_times: HashMap<GuildId, Instant> = HashMap::new();
        let idle_timeout = Duration::from_secs(5 * 60); // 5 minutes

        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;

            let manager = match songbird::get(&ctx).await {
                Some(m) => m,
                None => continue,
            };

            let guilds = ctx.cache.guilds();
            for guild_id in guilds {
                if let Some(call) = manager.get(guild_id) {
                    let mut is_idle = false;
                    let mut is_empty = false;

                    // Check if playing
                    {
                        let handler = call.lock().await;
                        if handler.queue().current().is_none() {
                            is_idle = true;
                        }
                    }

                    // Check if channel is empty (only bot)
                    if let Some(guild) = ctx.cache.guild(guild_id) {
                        let bot_id = ctx.cache.current_user().id;
                        if let Some(bot_vs) = guild.voice_states.get(&bot_id) {
                            if let Some(bot_channel) = bot_vs.channel_id {
                                let mut human_count = 0;
                                for vs in guild.voice_states.values() {
                                    if vs.channel_id == Some(bot_channel) && vs.user_id != bot_id {
                                        // Assume human if user is not cached (fallback), otherwise check bot status
                                        let is_human = ctx.cache.user(vs.user_id).map_or(true, |u| !u.bot);
                                        if is_human {
                                            human_count += 1;
                                        }
                                    }
                                }
                                if human_count == 0 {
                                    is_empty = true;
                                }
                            }
                        }
                    }

                    if is_idle || is_empty {
                        let entry = idle_times.entry(guild_id).or_insert_with(Instant::now);
                        if entry.elapsed() >= idle_timeout {
                            let reason = if is_empty { "empty voice channel" } else { "inactivity" };
                            info!("Disconnecting from guild {} due to {}.", guild_id, reason);
                            let _ = manager.remove(guild_id).await;
                            queue_mgr.clear(guild_id).await;
                            idle_times.remove(&guild_id);
                        } else {
                            debug!(
                                "Guild {} is idle/empty. Timeout in {:?}",
                                guild_id,
                                idle_timeout - entry.elapsed()
                            );
                        }
                    } else {
                        idle_times.remove(&guild_id);
                    }
                } else {
                    idle_times.remove(&guild_id);
                }
            }
        }
    });
}
