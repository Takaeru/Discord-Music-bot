use serenity::all::{ChannelId, Context, GuildId, UserId};

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
