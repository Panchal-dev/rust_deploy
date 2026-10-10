//! Turnstile handler.

use crate::browser::BrowserManager;
use crate::error::AppError;
use crate::models::BypassResponse;
use crate::proxy::NormalizedProxy;

/// Solve a Turnstile widget for `domain` using the provided `site_key`.
pub async fn solve(
    browser: &BrowserManager,
    domain: &str,
    site_key: &str,
    proxy: Option<&NormalizedProxy>,
) -> Result<BypassResponse, AppError> {
    if domain.trim().is_empty() {
        return Err(AppError::BadRequest("domain is required".into()));
    }
    if site_key.trim().is_empty() {
        return Err(AppError::BadRequest("siteKey is required".into()));
    }

    let parsed = url::Url::parse(domain)
        .map_err(|e| AppError::BadRequest(format!("invalid domain URL: {e}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::BadRequest(
            "domain must use http or https scheme".into(),
        ));
    }

    let token = browser
        .solve_turnstile(domain, site_key, proxy)
        .await
        .map_err(|e| AppError::SolveFailed(e.to_string()))?;

    if token.len() < 10 {
        return Err(AppError::SolveFailed(
            "turnstile token too short — likely invalid".into(),
        ));
    }

    Ok(BypassResponse::success_turnstile(token))
}
