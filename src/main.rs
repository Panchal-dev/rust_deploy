use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use std::env;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower::ServiceExt; // for `.oneshot()` in tests

// ============================================================
// HANDLERS
// ============================================================

/// Health check endpoint — Railway uses this for zero-downtime deploy
async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "railway-rust-test",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Root endpoint
async fn root() -> Json<Value> {
    Json(json!({
        "message": "Rust app running on Railway",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// Build the router (shared between main and tests)
fn build_app() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
}

// ============================================================
// MAIN
// ============================================================

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    // Railway auto-injects PORT env variable
    let port: u16 = env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a valid number");

    // Must bind to 0.0.0.0 so Railway's proxy can reach it
    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    tracing::info!("🚀 Starting server on {}", addr);

    let app = build_app();
    let listener = TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

// ============================================================
// TESTS — All in one file
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    /// Helper: call router and return (status, json body)
    async fn call_route(path: &str) -> (StatusCode, Value) {
        let app = build_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let status = response.status();
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);
        (status, json)
    }

    // ---------- TEST 1: /health ----------
    #[tokio::test]
    async fn test_health_returns_200() {
        let (status, _) = call_route("/health").await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_body_status_ok() {
        let (_, json) = call_route("/health").await;
        assert_eq!(json["status"], "ok");
        assert_eq!(json["service"], "railway-rust-test");
    }

    #[tokio::test]
    async fn test_health_has_timestamp() {
        let (_, json) = call_route("/health").await;
        assert!(json["timestamp"].is_string());
        let ts = json["timestamp"].as_str().unwrap();
        assert!(!ts.is_empty());
    }

    // ---------- TEST 2: / ----------
    #[tokio::test]
    async fn test_root_returns_200() {
        let (status, _) = call_route("/").await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn test_root_contains_railway() {
        let (_, json) = call_route("/").await;
        assert!(json["message"].as_str().unwrap().contains("Railway"));
    }

    #[tokio::test]
    async fn test_root_has_version() {
        let (_, json) = call_route("/").await;
        assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    }

    // ---------- TEST 3: 404 ----------
    #[tokio::test]
    async fn test_unknown_route_returns_404() {
        let (status, _) = call_route("/does-not-exist").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    // ---------- TEST 4: PORT env parsing ----------
    #[test]
    fn test_port_env_parse() {
        std::env::set_var("PORT", "8080");
        let port: u16 = std::env::var("PORT").unwrap().parse().unwrap();
        assert_eq!(port, 8080);
        std::env::remove_var("PORT");
    }
}
