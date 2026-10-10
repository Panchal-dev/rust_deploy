//! cf-bypass-rust — Cloudflare IUAM + Turnstile solver.
//!
//! HTTP API:
//!   GET  /health      -> readiness probe
//!   POST /cloudflare  -> solve IUAM or Turnstile for a domain
//!
//! Configuration is read from environment variables (see `config.rs`).

mod browser;
mod cache;
mod config;
mod ends;
mod error;
mod models;
mod proxy;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use crate::browser::BrowserManager;
use crate::cache::TtlCache;
use crate::config::Config;
use crate::error::AppError;
use crate::models::{BypassRequest, BypassResponse, HealthResponse};

/// Shared application state — cheap to clone (Arc + moka cache is internally
/// shared), so Axum can hand it to every handler without contention.
#[derive(Clone)]
pub struct AppState {
    pub browser: Arc<BrowserManager>,
    pub cache: TtlCache,
    pub config: Arc<Config>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = Arc::new(Config::from_env()?);
    tracing::info!(?config, "configuration loaded");

    let browser = Arc::new(BrowserManager::new(&config).await?);
    tracing::info!("browser solver ready");

    let state = AppState {
        browser,
        cache: TtlCache::new(config.cache_max_entries),
        config: config.clone(),
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/cloudflare", post(cloudflare))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("🚀 listening on {addr}");

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Liveness / readiness probe.
async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "cf-bypass-rust",
        uptime_seconds: state.browser.uptime_seconds(),
        contexts_available: state.browser.contexts_available(),
        auth_required: state.config.auth_token.is_some(),
    })
}

/// POST /cloudflare — solve the requested challenge mode.
async fn cloudflare(
    State(state): State<AppState>,
    Json(req): Json<BypassRequest>,
) -> Result<Json<BypassResponse>, AppError> {
    let start = std::time::Instant::now();

    // ---- 1. Authentication ------------------------------------------------
    if let Some(expected) = &state.config.auth_token {
        if req.auth_token.as_deref() != Some(expected.as_str()) {
            return Err(AppError::Unauthorized);
        }
    }

    // ---- 2. Validate mode -------------------------------------------------
    let mode = req.mode.as_str();
    if mode != "iuam" && mode != "turnstile" {
        return Err(AppError::BadRequest(format!("unknown mode: {mode}")));
    }

    // ---- 3. Normalise proxy ----------------------------------------------
    let proxy = proxy::normalize(req.proxy.clone())?;

    // ---- 4. Cache lookup (IUAM + no proxy only) --------------------------
    let cache_key = if mode == "iuam" && proxy.is_none() {
        Some(req.domain.clone())
    } else {
        None
    };

    if let Some(key) = &cache_key {
        if let Some(hit) = state.cache.get(key).await {
            tracing::debug!(domain = %req.domain, "cache hit");
            return Ok(Json(BypassResponse {
                cached: true,
                elapsed: format!("{:.2}s", start.elapsed().as_secs_f64()),
                ..hit
            }));
        }
    }

    // ---- 5. Solve ---------------------------------------------------------
    let result = match mode {
        "iuam" => ends::iuam::solve(&state.browser, &req.domain, proxy.as_ref()).await,
        "turnstile" => {
            let site_key = req
                .site_key
                .as_deref()
                .ok_or_else(|| AppError::BadRequest("siteKey required for turnstile".into()))?;
            ends::turnstile::solve(&state.browser, &req.domain, site_key, proxy.as_ref()).await
        }
        _ => unreachable!(),
    }?;

    let response = BypassResponse {
        elapsed: format!("{:.2}s", start.elapsed().as_secs_f64()),
        cached: false,
        ..result
    };

    // ---- 6. Store in cache (IUAM + no proxy + success only) --------------
    if let Some(key) = cache_key {
        if response.cf_clearance.is_some() {
            state.cache.set(key, response.clone()).await;
        }
    }

    Ok(Json(response))
}
