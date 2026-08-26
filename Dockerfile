# Stage 1: Build Rust binary
FROM rust:bookworm AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libopus-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/discord-bot
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

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
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /usr/src/discord-bot/target/release/discord-music-bot /app/discord-music-bot

ENTRYPOINT ["/app/discord-music-bot"]
