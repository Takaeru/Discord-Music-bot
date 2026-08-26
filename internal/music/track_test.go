package music

import (
	"encoding/json"
	"testing"
	"time"
)

func TestTrackJSONSerialization(t *testing.T) {
	track := &Track{
		ID:          "yt-123",
		Title:       "Test Song Title",
		URL:         "https://www.youtube.com/watch?v=yt-123",
		Duration:    3*time.Minute + 30*time.Second,
		Thumbnail:   "https://img.youtube.com/vi/yt-123/default.jpg",
		RequestedBy: "DiscordUser#1234",
		Source:      "youtube",
	}

	data, err := json.Marshal(track)
	if err != nil {
		t.Fatalf("failed to marshal Track to JSON: %v", err)
	}

	var unmarshaled Track
	if err := json.Unmarshal(data, &unmarshaled); err != nil {
		t.Fatalf("failed to unmarshal Track from JSON: %v", err)
	}

	if unmarshaled.ID != track.ID {
		t.Errorf("expected ID '%s', got '%s'", track.ID, unmarshaled.ID)
	}
	if unmarshaled.Title != track.Title {
		t.Errorf("expected Title '%s', got '%s'", track.Title, unmarshaled.Title)
	}
	if unmarshaled.URL != track.URL {
		t.Errorf("expected URL '%s', got '%s'", track.URL, unmarshaled.URL)
	}
	if unmarshaled.Duration != track.Duration {
		t.Errorf("expected Duration '%v', got '%v'", track.Duration, unmarshaled.Duration)
	}
	if unmarshaled.Thumbnail != track.Thumbnail {
		t.Errorf("expected Thumbnail '%s', got '%s'", track.Thumbnail, unmarshaled.Thumbnail)
	}
	if unmarshaled.RequestedBy != track.RequestedBy {
		t.Errorf("expected RequestedBy '%s', got '%s'", track.RequestedBy, unmarshaled.RequestedBy)
	}
	if unmarshaled.Source != track.Source {
		t.Errorf("expected Source '%s', got '%s'", track.Source, unmarshaled.Source)
	}
}
