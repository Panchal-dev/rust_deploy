//! Browser / solver manager.
//!
//! Wraps `chaser-cf`'s CloudflareSolver and exposes a small, stable
//! interface used by the endpoint handlers. If the underlying solver is
//! swapped later, only this file needs to change.

use std::sync::Arc;
use std::time::Instant;

use chaser_cf::{CloudflareSolver, ProxyConfig};

use crate::config::Config;
use crate::proxy::NormalizedProxy;

/// Shared solver handle. Internally reference-counted; clone is cheap.
pub struct BrowserManager {
    solver: Arc<CloudflareSolver>,
    timeout_ms: u64,
    started_at: Instant,
    context_limit: usize,
}

impl BrowserManager {
    /// Build the solver once at startup. This launches Xvfb (on Linux) and a
    /// warm browser context pool, so subsequent solves are fast.
    pub async fn new(config: &Config) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let solver = CloudflareSolver::builder()
            .with_virtual_display(cfg!(target_os = "linux"))
            .with_context_limit(config.browser_limit)
            .with_timeout_ms(config.timeout_ms)
            .add_extra_arg("--no-sandbox")
            .add_extra_arg("--disable-dev-shm-usage")
            .build()
            .await?;

        Ok(Self {
            solver: Arc::new(solver),
            timeout_ms: config.timeout_ms,
            started_at: Instant::now(),
            context_limit: config.browser_limit,
        })
    }

    /// Seconds since the solver was created (approximate uptime).
    pub fn uptime_seconds(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Reported pool size (used as a rough "contexts available" metric).
    pub fn contexts_available(&self) -> usize {
        self.context_limit
    }

    /// Solve an IUAM (managed) challenge, returning the `cf_clearance` cookie.
    pub async fn solve_iuam(
        &self,
        domain: &str,
        proxy: Option<&NormalizedProxy>,
    ) -> Result<IuamOutcome, Box<dyn std::error::Error + Send + Sync>> {
        let task = self.solver.solve_waf_session(domain);
        let task = Self::apply_proxy(task, proxy);

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(self.timeout_ms),
            task,
        )
        .await
        .map_err(|_| -> Box<dyn std::error::Error + Send + Sync> {
            "solve timed out".into()
        })??;

        Ok(IuamOutcome {
            cf_clearance: result.cf_clearance,
            user_agent: result.user_agent,
            cookies: result.cookies,
        })
    }

    /// Solve a Turnstile widget with a known site key. Returns the token.
    pub async fn solve_turnstile(
        &self,
        domain: &str,
        site_key: &str,
        proxy: Option<&NormalizedProxy>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let task = self.solver.solve_turnstile_min(domain, site_key);
        let task = Self::apply_proxy(task, proxy);

        let token = tokio::time::timeout(
            std::time::Duration::from_millis(self.timeout_ms),
            task,
        )
        .await
        .map_err(|_| -> Box<dyn std::error::Error + Send + Sync> {
            "turnstile solve timed out".into()
        })??;

        Ok(token)
    }

    /// Attach a proxy to a solve task if one was supplied.
    fn apply_proxy<T>(task: T, proxy: Option<&NormalizedProxy>) -> T
    where
        T: ProxyAttachable,
    {
        match proxy {
            Some(p) => {
                let cfg = ProxyConfig::new(&p.host, p.port)
                    .with_scheme(p.chrome_scheme())
                    .with_auth_opt(p.username.as_deref(), p.password.as_deref());
                task.with_proxy(cfg)
            }
            None => task,
        }
    }
}

/// Result of an IUAM solve.
pub struct IuamOutcome {
    pub cf_clearance: String,
    pub user_agent: String,
    pub cookies: Vec<String>,
}

/// Trait capturing the `with_proxy` method on chaser-cf solve tasks, so the
/// generic helper above compiles for both IUAM and Turnstile task types.
pub trait ProxyAttachable {
    fn with_proxy(self, proxy: ProxyConfig) -> Self;
}
