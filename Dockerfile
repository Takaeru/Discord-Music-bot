# Stage 1: Build Rust binary
FROM rust:bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libopus-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/discord-bot
COPY Cargo.toml Cargo.lock* ./

# Pre-build dependensi untuk cache layer
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release

# Stage 2: Minimal runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ffmpeg \
    libopus0 \
    ca-certificates \
    curl \
    python3 \
    && curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp \
    && chmod a+rx /usr/local/bin/yt-dlp \
    && mkdir -p /etc/yt-dlp /root/.config/yt-dlp /app/.config/yt-dlp \
    && printf -- '--format-sort acodec:opus,acodec:mp3,protocol:https\n-f bestaudio[acodec=opus]/bestaudio[ext=webm]/bestaudio[ext=mp3]/bestaudio[acodec!=aac]/bestaudio\n' | tee /etc/yt-dlp.conf /etc/yt-dlp/config /root/.config/yt-dlp/config /app/.config/yt-dlp/config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/src/discord-bot/target/release/discord-music-bot /app/discord-music-bot

ENTRYPOINT ["/app/discord-music-bot"]
