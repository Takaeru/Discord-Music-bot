package music

import (
	"time"

	"github.com/disgoorg/disgolink/v3/lavalink"
)

// Track represents an individual audio item in the music queue.
type Track struct {
	ID            string          `json:"id"`
	Title         string          `json:"title"`
	URL           string          `json:"url"`
	Duration      time.Duration   `json:"duration"`
	Thumbnail     string          `json:"thumbnail"`
	RequestedBy   string          `json:"requested_by"`
	Source        string          `json:"source"`
	LavalinkTrack *lavalink.Track `json:"lavalink_track,omitempty"`
}
