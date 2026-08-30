# Stage 1: Build Rust binary
# Builder selalu native (BUILDPLATFORM) supaya cepat; xx handle cross-compile-nya
FROM --platform=$BUILDPLATFORM rust:bookworm AS builder

ARG TARGETPLATFORM
ARG BUILDPLATFORM

# xx = Docker official cross-compilation helper (handles PKG_CONFIG, sysroot, linker, dll)
COPY --from=tonistiigi/xx:latest / /

# Install build tools + clang (dibutuhkan xx sebagai cross-linker)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    clang \
    lld \
    && rm -rf /var/lib/apt/lists/*

# Install libs versi target platform (arm64 atau amd64) via xx-apt-get
RUN xx-apt-get install -y --no-install-recommends \
    libssl-dev \
    libopus-dev

# Tambahkan Rust target yang sesuai dengan TARGETPLATFORM
RUN xx-info env && rustup target add $(xx-cargo --print-target-triple)

WORKDIR /usr/src/discord-bot
COPY Cargo.toml Cargo.lock* ./

# Pre-build dependensi (caching layer)
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && xx-cargo build --release \
    && rm -rf src

COPY src ./src

# Build final binary lalu salin ke path yang konsisten
RUN touch src/main.rs \
    && xx-cargo build --release \
    && cp "target/$(xx-cargo --print-target-triple)/release/discord-music-bot" /app-binary

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

COPY --from=builder /app-binary /app/discord-music-bot

ENTRYPOINT ["/app/discord-music-bot"]
