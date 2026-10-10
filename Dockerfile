# ============================================================
# STAGE 1: Chef — base image with cargo-chef
# ============================================================
FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /app

# ============================================================
# STAGE 2: Planner — generate dependency recipe
# ============================================================
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ============================================================
# STAGE 3: Builder — cook deps + build binary
# ============================================================
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json

# Cache dependencies (this layer rebuilds only when Cargo.toml changes)
RUN cargo chef cook --release --recipe-path recipe.json

# Build actual app
COPY . .
RUN cargo build --release --bin railway-rust-test

# ============================================================
# STAGE 4: Runtime — minimal Debian image
# ============================================================
FROM debian:bookworm-slim AS runtime
WORKDIR /app

# Install CA certs (needed for HTTPS outbound) + curl (for healthcheck)
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
    && rm -rf /var/lib/apt/lists/*

# Copy compiled binary
COPY --from=builder /app/target/release/railway-rust-test /usr/local/bin/railway-rust-test

# Non-root user (security best practice)
RUN useradd -m -s /bin/bash appuser
USER appuser

# Railway injects PORT at runtime; default 3000 for local
ENV PORT=3000
EXPOSE 3000

# Container-level healthcheck
HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:${PORT}/health || exit 1

CMD ["railway-rust-test"]
