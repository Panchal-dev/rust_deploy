# ============================================================
# STAGE 1: Builder — Rust toolchain + dependency cache
# ============================================================
# Rust 1.90 chosen because chaser-cf's transitive dependencies
# (uuid, icu_*, idna_adapter, textwrap) require rustc >= 1.88/1.89.
FROM rust:1.90-slim-bookworm AS builder

WORKDIR /app

# Build-time system deps. libssl-dev is needed because chaser-cf
# pulls in native-tls which links against OpenSSL at build time.
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config \
        libssl-dev \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# --- Dependency cache layer -----------------------------------
# Copy manifests only, add a stub main.rs, and build once so that
# all third-party crates get compiled into a cached Docker layer.
# Subsequent builds only recompile our own code.
COPY Cargo.toml ./
COPY Cargo.lock* ./

RUN mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# --- Real build -----------------------------------------------
COPY src ./src
RUN cargo build --release --bin cf-bypass-rust

# Sanity check: make sure the binary exists and is executable.
RUN test -x /app/target/release/cf-bypass-rust \
    || (echo "binary missing after build" && exit 1)

# ============================================================
# STAGE 2: Runtime — slim Debian + Chromium + Xvfb
# ============================================================
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Runtime system deps.
#
# libssl3 / libgcc-s1 / zlib1g are REQUIRED because the release
# binary dynamically links against them (native-tls → OpenSSL,
# reqwest → zlib, std → libgcc). Without libssl3 you get:
#   error while loading shared libraries: libssl.so.3
# and the container exits BEFORE any log line is emitted —
# which is exactly the "Starting Container ... (silence)" symptom.
#
# chromium + chromium-driver + xvfb: chaser-cf launches a headed
# Chromium inside a virtual display (Cloudflare IUAM detects bare
# headless Chrome on Linux servers).
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        libssl3 \
        libgcc-s1 \
        zlib1g \
        chromium \
        chromium-driver \
        xvfb \
        xauth \
        fonts-liberation \
        fonts-dejavu-core \
        procps \
    && rm -rf /var/lib/apt/lists/*

# --- Environment ----------------------------------------------
# CHROME_BIN: chaser-cf/playwright/chromiumoxide look this up.
ENV CHROME_BIN=/usr/bin/chromium
ENV CHROMIUM_PATH=/usr/bin/chromium

# Non-root user home — Chromium/Xvfb write profiles here.
ENV HOME=/home/appuser
ENV XDG_CONFIG_HOME=/home/appuser/.config
ENV XDG_CACHE_HOME=/home/appuser/.cache

# Virtual display used by chaser-cf's with_virtual_display(true).
ENV DISPLAY=:99

# NOTE: do NOT set PORT here. Railway injects PORT at runtime and
# that value must win. Hardcoding ENV PORT=8742 was why Railway
# routed to 8080 while the app bound to 8742 → 502.
# Our config.rs already defaults to 8080 when PORT is unset.
EXPOSE 8080

# --- Copy compiled binary -------------------------------------
COPY --from=builder /app/target/release/cf-bypass-rust /usr/local/bin/cf-bypass-rust

# --- Non-root user + writable dirs ----------------------------
# /tmp/.X11-unix must be world-writable (sticky) so Xvfb can
# create its socket; otherwise Xvfb silently fails and Chromium
# launch hangs forever.
RUN useradd -m -s /bin/bash appuser \
    && mkdir -p /tmp/.X11-unix \
    && chmod 1777 /tmp/.X11-unix \
    && mkdir -p /home/appuser/.config /home/appuser/.cache \
    && chown -R appuser:appuser /app \
    && chown -R appuser:appuser /home/appuser \
    && chmod -R u+w /app

USER appuser

# --- Healthcheck ----------------------------------------------
# start-period is 120s because Chromium + Xvfb + chaser-cf warm-up
# on Railway can take 30–90 seconds. 25s was too aggressive and
# caused Railway to mark the deployment unhealthy → restart loop.
HEALTHCHECK --interval=30s --timeout=10s --start-period=120s --retries=5 \
    CMD curl -fsS "http://127.0.0.1:${PORT:-8080}/health" || exit 1

CMD ["cf-bypass-rust"]