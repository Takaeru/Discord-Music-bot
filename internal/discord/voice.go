package discord

import (
	"errors"
	"fmt"

	"github.com/bwmarrin/discordgo"
)

var (
	// ErrUserNotInVoice is returned when a user issues a voice command without being in a voice channel.
	ErrUserNotInVoice = errors.New("user is not in a voice channel")

	// ErrBotNotInVoice is returned when a leave command is executed but the bot is not in voice.
	ErrBotNotInVoice = errors.New("bot is not connected to a voice channel")

	// ErrNotInGuild is returned when a voice command is invoked outside of a guild.
	ErrNotInGuild = errors.New("command can only be used in a server")

	// ErrMissingConnectPermission is returned when the bot lacks PermissionVoiceConnect in a voice channel.
	ErrMissingConnectPermission = errors.New("bot lacks permission to Connect to this voice channel")

	// ErrMissingSpeakPermission is returned when the bot lacks PermissionVoiceSpeak in a voice channel.
	ErrMissingSpeakPermission = errors.New("bot lacks permission to Speak in this voice channel")
)

// CheckVoicePermissions verifies that the bot has Connect and Speak permissions in the target channel.
func CheckVoicePermissions(s *discordgo.Session, channelID string) error {
	if s == nil || s.State == nil || s.State.User == nil {
		return nil
	}

	perms, err := s.State.UserChannelPermissions(s.State.User.ID, channelID)
	if err != nil {
		perms, err = s.UserChannelPermissions(s.State.User.ID, channelID)
		if err != nil {
			return nil
		}
	}

	// Administrator permission bypasses channel restrictions
	if perms&discordgo.PermissionAdministrator != 0 {
		return nil
	}

	if perms&discordgo.PermissionVoiceConnect == 0 {
		return ErrMissingConnectPermission
	}
	if perms&discordgo.PermissionVoiceSpeak == 0 {
		return ErrMissingSpeakPermission
	}
	return nil
}

// FindUserVoiceChannel finds the voice channel ID of a user within a guild.
func FindUserVoiceChannel(s *discordgo.Session, guildID, userID string) (string, error) {
	if guildID == "" {
		return "", ErrNotInGuild
	}

	guild, err := s.State.Guild(guildID)
	if err != nil {
		guild, err = s.Guild(guildID)
		if err != nil {
			return "", fmt.Errorf("failed to fetch guild state: %w", err)
		}
	}

	for _, vs := range guild.VoiceStates {
		if vs.UserID == userID {
			return vs.ChannelID, nil
		}
	}

	return "", ErrUserNotInVoice
}

// JoinVoiceChannel joins or moves the bot to the specified voice channel using gateway signaling.
func (b *Bot) JoinVoiceChannel(guildID, channelID string) error {
	// Check voice permissions before attempting connection
	if err := CheckVoicePermissions(b.Session, channelID); err != nil {
		b.logger.Warn("Missing voice channel permissions",
			"guild_id", guildID,
			"channel_id", channelID,
			"error", err,
		)
		return err
	}

	_, err := b.musicMgr.GetOrCreate(guildID)
	if err != nil {
		return fmt.Errorf("failed to get/create guild player: %w", err)
	}

	b.logger.Info("Joining voice channel via Lavalink gateway signaling", "guild_id", guildID, "channel_id", channelID)
	if b.Session != nil && b.Session.DataReady {
		err = b.Session.ChannelVoiceJoinManual(guildID, channelID, false, true)
		if err != nil {
			return fmt.Errorf("failed to initiate voice gateway connection: %w", err)
		}
	}

	return nil
}

// LeaveVoiceChannel disconnects the bot from voice, clears the queue, and resets guild state.
func (b *Bot) LeaveVoiceChannel(guildID string) error {
	_, err := b.musicMgr.Get(guildID)
	if err != nil {
		return ErrBotNotInVoice
	}

	b.logger.Info("Leaving voice channel", "guild_id", guildID)
	if b.Session != nil && b.Session.DataReady {
		_ = b.Session.ChannelVoiceJoinManual(guildID, "", false, false)
	}
	return b.musicMgr.Remove(guildID)
}
