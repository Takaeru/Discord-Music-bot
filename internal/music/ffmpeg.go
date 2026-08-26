package music

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"os/exec"
	"sync"
)

var (
	// ErrStreamFailed is returned when initializing or running the FFmpeg process fails.
	ErrStreamFailed = errors.New("failed to initialize ffmpeg audio stream")

	// ErrEmptySourceURL is returned when an empty source URL is provided to the pipeline.
	ErrEmptySourceURL = errors.New("source URL cannot be empty")
)

// AudioPipeline defines the interface for creating an audio stream from a track or media source URL.
type AudioPipeline interface {
	NewStream(ctx context.Context, sourceURL string, volume int) (*AudioStream, error)
}

// AudioStream represents an active audio stream piped from an external transcoding process.
type AudioStream struct {
	cmd    *exec.Cmd
	stdout io.ReadCloser
	stderr *bytes.Buffer
	mu     sync.Mutex
	closed bool
}

// Read reads transcoded PCM audio data from the pipeline stream.
func (s *AudioStream) Read(p []byte) (int, error) {
	return s.stdout.Read(p)
}

// Close cleanly stops the transcoding process, closes pipes, and cleans up system resources.
func (s *AudioStream) Close() error {
	s.mu.Lock()
	defer s.mu.Unlock()

	if s.closed {
		return nil
	}
	s.closed = true

	var closeErr error
	if s.stdout != nil {
		closeErr = s.stdout.Close()
	}

	if s.cmd != nil && s.cmd.Process != nil {
		_ = s.cmd.Process.Kill()
		_ = s.cmd.Wait()
	}

	return closeErr
}

// StderrOutput returns the captured stderr output for troubleshooting and error diagnostics.
func (s *AudioStream) StderrOutput() string {
	if s.stderr == nil {
		return ""
	}
	return s.stderr.String()
}

// FFmpegPipeline implements AudioPipeline by executing FFmpeg to convert media to 48kHz 16-bit stereo PCM.
type FFmpegPipeline struct {
	binaryPath string
}

// NewFFmpegPipeline creates a new FFmpegPipeline with the specified binary path.
func NewFFmpegPipeline(binaryPath string) *FFmpegPipeline {
	if binaryPath == "" {
		binaryPath = "ffmpeg"
	}
	return &FFmpegPipeline{
		binaryPath: binaryPath,
	}
}

// BuildArgs constructs the command-line arguments for FFmpeg audio transcoding.
func (f *FFmpegPipeline) BuildArgs(sourceURL string, volume int) []string {
	volFactor := float64(volume) / 100.0
	if volFactor < 0 {
		volFactor = 0
	} else if volFactor > 1 {
		volFactor = 1
	}

	return []string{
		"-reconnect", "1",
		"-reconnect_streamed", "1",
		"-reconnect_delay_max", "5",
		"-i", sourceURL,
		"-vn",
		"-filter:a", fmt.Sprintf("volume=%.2f", volFactor),
		"-f", "s16le",
		"-ar", "48000",
		"-ac", "2",
		"pipe:1",
	}
}

// NewStream launches an FFmpeg child process streaming transcoded s16le PCM to stdout.
func (f *FFmpegPipeline) NewStream(ctx context.Context, sourceURL string, volume int) (*AudioStream, error) {
	if sourceURL == "" {
		return nil, ErrEmptySourceURL
	}

	args := f.BuildArgs(sourceURL, volume)
	cmd := exec.CommandContext(ctx, f.binaryPath, args...)

	stdout, err := cmd.StdoutPipe()
	if err != nil {
		return nil, fmt.Errorf("%w: failed to create stdout pipe: %v", ErrStreamFailed, err)
	}

	var stderr bytes.Buffer
	cmd.Stderr = &stderr

	if err := cmd.Start(); err != nil {
		_ = stdout.Close()
		return nil, fmt.Errorf("%w: failed to start ffmpeg process: %v", ErrStreamFailed, err)
	}

	stream := &AudioStream{
		cmd:    cmd,
		stdout: stdout,
		stderr: &stderr,
	}

	// Ensure process termination on context cancellation
	go func() {
		<-ctx.Done()
		_ = stream.Close()
	}()

	return stream, nil
}
