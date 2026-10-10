//! Application error type and its HTTP mapping.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// Domain-level error. Every variant maps to one HTTP status and a
/// machine-readable JSON body.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unauthorized")]
    Unauthorized,

    #[error("solve failed: {0}")]
    SolveFailed(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, 400),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, 401),
            AppError::SolveFailed(_) => (StatusCode::INTERNAL_SERVER_ERROR, 500),
            AppError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, 500),
        };
        let body = Json(json!({ "code": code, "message": self.to_string() }));
        (status, body).into_response()
    }
}

/// Convert low-level errors into a generic internal error.
impl From<Box<dyn std::error::Error + Send + Sync>> for AppError {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        AppError::Internal(e.to_string())
    }
}