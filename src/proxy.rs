//! Proxy input normalisation.
//!
//! Accepts either a URL string ("socks5://user:pass@host:1080") or an object
//! `{ host, port, scheme?, username?, password? }`. Produces a canonical
//! `NormalizedProxy` that the browser layer can consume directly.

use serde::Deserialize;

use crate::error::AppError;

/// Raw proxy value as it arrives on the wire. Untagged so serde accepts both
/// shapes transparently.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ProxyInput {
    Url(String),
    Object {
        host: String,
        port: u16,
        #[serde(default)]
        scheme: Option<String>,
        #[serde(default)]
        username: Option<String>,
        #[serde(default)]
        password: Option<String>,
    },
}

/// Canonical proxy shape used by the solver.
#[derive(Debug, Clone)]
pub struct NormalizedProxy {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl NormalizedProxy {
    /// Scheme mapped to a canonical value Chrome accepts.
    pub fn chrome_scheme(&self) -> &str {
        match self.scheme.as_str() {
            "socks4" => "socks4",
            "socks5" => "socks5",
            _ => "http",
        }
    }
}

/// Normalise the incoming proxy. Returns `Ok(None)` when nothing was
/// requested; `Err` when the value was present but unparseable.
pub fn normalize(input: Option<ProxyInput>) -> Result<Option<NormalizedProxy>, AppError> {
    let Some(input) = input else { return Ok(None) };

    match input {
        ProxyInput::Url(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Err(AppError::BadRequest("empty proxy URL".into()));
            }

            // Bare "host:port" -> prefix http:// so url::Url can parse it.
            let candidate = if trimmed.contains("://") {
                trimmed.to_string()
            } else {
                format!("http://{trimmed}")
            };

            let parsed = url::Url::parse(&candidate)
                .map_err(|e| AppError::BadRequest(format!("invalid proxy URL: {e}")))?;

            let scheme = match parsed.scheme() {
                "socks4" => "socks4",
                "socks5" | "socket5" => "socks5",
                _ => "http",
            }
            .to_string();

            let host = parsed
                .host_str()
                .ok_or_else(|| AppError::BadRequest("proxy host missing".into()))?
                .to_string();

            let port = parsed.port().unwrap_or(if scheme == "http" { 80 } else { 1080 });

            Ok(Some(NormalizedProxy {
                scheme,
                host,
                port,
                username: if parsed.username().is_empty() {
                    None
                } else {
                    Some(parsed.username().to_string())
                },
                password: parsed.password().map(String::from),
            }))
        }
        ProxyInput::Object { host, port, scheme, username, password } => {
            if host.trim().is_empty() {
                return Err(AppError::BadRequest("proxy host is empty".into()));
            }
            if port == 0 {
                return Err(AppError::BadRequest("proxy port must be 1..=65535".into()));
            }

            let scheme = match scheme.as_deref().unwrap_or("http") {
                s if s.starts_with("socks4") => "socks4",
                s if s.starts_with("socks5") || s.starts_with("socket5") => "socks5",
                _ => "http",
            }
            .to_string();

            Ok(Some(NormalizedProxy { scheme, host, port, username, password }))
        }
    }
}
