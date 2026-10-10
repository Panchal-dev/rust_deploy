# ============================================================
# STAGE 1: Builder — Rust toolchain + cargo cache
# ============================================================
FROM rust:1.85-slim-bookworm AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependency compilation
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
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

COPY --from=builder /app/target/release/cf-bypass-rust /usr/local/bin/cf-bypass-rust

# Non-root user
RUN useradd -m -s /bin/bash appuser
USER appuser

HEALTHCHECK --interval=30s --timeout=10s --start-period=25s --retries=3 \
    CMD curl -f http://localhost:${PORT}/health || exit 1

CMD ["cf-bypass-rust"]
