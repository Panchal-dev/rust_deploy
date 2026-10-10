//! IUAM (managed challenge) handler.

use crate::browser::BrowserManager;
use crate::error::AppError;
use crate::models::BypassResponse;
use crate::proxy::NormalizedProxy;

/// Solve the managed challenge for `domain`. Returns a populated
/// `BypassResponse` on success.
pub async fn solve(
    browser: &BrowserManager,
    domain: &str,
    proxy: Option<&NormalizedProxy>,
) -> Result<BypassResponse, AppError> {
    if domain.trim().is_empty() {
        return Err(AppError::BadRequest("domain is required".into()));
    }

    // Reject non-http(s) to avoid file:// style surprises.
    let parsed = url::Url::parse(domain)
        .map_err(|e| AppError::BadRequest(format!("invalid domain URL: {e}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::BadRequest(
            "domain must use http or https scheme".into(),
        ));
    }

    let outcome = browser
        .solve_iuam(domain, proxy)
        .await
        .map_err(|e| AppError::SolveFailed(e.to_string()))?;

    if outcome.cf_clearance.is_empty() {
        return Err(AppError::SolveFailed(
            "cf_clearance cookie not obtained".into(),
        ));
    }

    Ok(BypassResponse::success_iuam(
        outcome.cf_clearance,
        outcome.user_agent,
        outcome.cookies,
    ))
}
