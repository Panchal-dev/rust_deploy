# ============================================================
# STAGE 1: Builder — Rust toolchain + dependency cache
# ============================================================
# Rust 1.90 chosen because chaser-cf's transitive dependencies
# (uuid, icu_*, idna_adapter, textwrap) require rustc >= 1.88/1.89.
FROM rust:1.90-slim-bookworm AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependency compilation.
# We copy Cargo.toml + Cargo.lock, drop in a stub main.rs, and build
# once so that all third-party crates are compiled into a cached layer.
# Subsequent builds only recompile our own code.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# Build the real project
COPY src ./src
RUN cargo build --release --bin cf-bypass-rust

# ============================================================
# STAGE 2: Runtime — slim Debian + Chromium + Xvfb
# ============================================================
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Chromium + Xvfb are required: Cloudflare's managed challenge
# (IUAM) detects bare headless Chrome on Linux servers, so we run
# a headed Chromium inside a virtual display.
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        chromium \
        chromium-driver \
        xvfb \
        fonts-liberation \
    && rm -rf /var/lib/apt/lists/*

ENV CHROME_BIN=/usr/bin/chromium
ENV PORT=8742
EXPOSE 8742

# Copy the compiled binary
COPY --from=builder /app/target/release/cf-bypass-rust /usr/local/bin/cf-bypass-rust

# Non-root user (security best practice)
RUN useradd -m -s /bin/bash appuser
USER appuser

# Container-level healthcheck (Railway also uses /health for deploy gate)
HEALTHCHECK --interval=30s --timeout=10s --start-period=25s --retries=3 \
    CMD curl -f http://localhost:${PORT}/health || exit 1

CMD ["cf-bypass-rust"]