# Stage 1: Build Rust binary
# Gunakan --platform=$BUILDPLATFORM agar builder selalu jalan native (cepat)
FROM --platform=$BUILDPLATFORM rust:bookworm AS builder

# ARG dari Docker Buildx untuk cross-compilation
ARG TARGETPLATFORM
ARG BUILDPLATFORM

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libopus-dev \
    # Cross-compilation tools untuk ARM64
    gcc-aarch64-linux-gnu \
    libc6-dev-arm64-cross \
    && rm -rf /var/lib/apt/lists/*

# Tentukan Rust target berdasarkan TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
      "linux/arm64") \
        echo "aarch64-unknown-linux-gnu" > /rust_target && \
        rustup target add aarch64-unknown-linux-gnu ;; \
      *) \
        echo "x86_64-unknown-linux-gnu" > /rust_target && \
        rustup target add x86_64-unknown-linux-gnu ;; \
    esac

WORKDIR /usr/src/discord-bot
COPY Cargo.toml Cargo.lock* ./

# Pre-build deps cache dengan dummy main
RUN TARGET=$(cat /rust_target) && \
    mkdir src && echo "fn main() {}" > src/main.rs && \
    case "$TARGET" in \
      "aarch64-unknown-linux-gnu") \
        CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
        PKG_CONFIG_SYSROOT_DIR=/ \
        cargo build --release --target "$TARGET" ;; \
      *) cargo build --release --target "$TARGET" ;; \
    esac && \
    rm -rf src

COPY src ./src

RUN TARGET=$(cat /rust_target) && \
    touch src/main.rs && \
    case "$TARGET" in \
      "aarch64-unknown-linux-gnu") \
        CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
        PKG_CONFIG_SYSROOT_DIR=/ \
        cargo build --release --target "$TARGET" ;; \
      *) cargo build --release --target "$TARGET" ;; \
    esac && \
    # Salin binary ke lokasi yang konsisten
    cp "target/$TARGET/release/discord-music-bot" /app-binary

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
