# =============================================================================
# Battles Record - Docker Build
# =============================================================================
# Multi-stage build: Rust builder -> Debian slim runtime with FFmpeg
# Final image size: ~150MB

# -----------------------------------------------------------------------------
# Stage 1: Build the Rust binary
# -----------------------------------------------------------------------------
FROM rust:1.93-bookworm AS builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY br-daemon ./br-daemon

# Build release binary
RUN cargo build --release --package br-daemon

# -----------------------------------------------------------------------------
# Stage 2: Runtime image
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim

# Install runtime dependencies
# curl is needed for Kick API calls (bypasses Cloudflare TLS fingerprinting)
# unzip is needed for Bun installation
RUN apt-get update && apt-get install -y --no-install-recommends \
    ffmpeg \
    curl \
    ca-certificates \
    unzip \
    && rm -rf /var/lib/apt/lists/*

# Install Bun (required for yt-dlp YouTube support)
RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:${PATH}"

# Install yt-dlp
RUN apt-get update && apt-get install -y --no-install-recommends python3-pip \
    && pip3 install --no-cache-dir --break-system-packages yt-dlp \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/br-daemon /usr/local/bin/

# Copy entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/

# Setup directories and permissions
RUN chmod +x /usr/local/bin/docker-entrypoint.sh \
    && mkdir -p /config /data/recordings /data/library /data/images

# Default port
EXPOSE 8080

# Volume mount points
VOLUME ["/config", "/data/recordings", "/data/library", "/data/images"]

ENTRYPOINT ["docker-entrypoint.sh"]
