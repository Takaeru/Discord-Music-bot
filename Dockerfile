# syntax=docker/dockerfile:1

# Stage 1: Build binary
FROM golang:1.24-alpine AS builder

WORKDIR /build

# Pre-fetch Go modules
COPY go.mod go.sum ./
RUN go mod download

# Copy application source code
COPY . .

# Compile optimized static Go binary
RUN CGO_ENABLED=0 GOOS=linux go build -trimpath -ldflags="-w -s" -o /build/bot ./cmd/bot

# Stage 2: Runtime image
FROM alpine:3.21

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    ffmpeg \
    yt-dlp \
    python3 \
    tzdata

# Create dedicated non-root user and group
RUN addgroup -g 1000 botuser && \
    adduser -D -u 1000 -G botuser botuser

WORKDIR /app

# Copy compiled binary from builder stage
COPY --from=builder /build/bot /app/bot

# Set file ownership
RUN chown -R botuser:botuser /app

# Run as non-root user
USER botuser:botuser

# Support graceful shutdown via SIGTERM
STOPSIGNAL SIGTERM

ENTRYPOINT ["/app/bot"]
