use std::env;

/// Runtime configuration read from the environment once at startup, matching the `SHARED_URL`
/// convention documented in `docs/frontend-conventions.md` (sweetrpg/platform).
pub struct Config {
    pub port: u16,
    pub shared_url: String,
    /// Base URL for `game-systems-api`. In-cluster this is
    /// `http://api-v1.sweetrpg-game-systems.svc.cluster.local:8000`; unset falls back to a
    /// local instance. Internal calls bypass the ingress, so no `/api/N` version prefix.
    pub game_systems_api_url: String,
    pub otlp_endpoint: Option<String>,
    pub log_level: String,
    /// Host of the shared session Redis instance `auth-web` owns - this app only ever reads it,
    /// never writes. Unset by default: the `SessionClient` stays disabled (every visitor reads
    /// as logged-out) until this is set. Named `SHARED_SESSION_REDIS_*` to leave `REDIS_*` free
    /// for this app's own cache Redis (`REDIS_DB=1` in `sweetrpg-game-systems`) if one is ever
    /// added.
    pub shared_session_redis_host: Option<String>,
    pub shared_session_redis_port: u16,
    /// Logical DB index on the shared Redis instance. Must match `auth-web`'s own `REDIS_DB`
    /// value; reading the wrong index silently degrades to "every visitor logged-out".
    pub shared_session_redis_db: u16,
    pub shared_session_redis_pass: Option<String>,
    /// Sentry error-reporting DSN. Unset keeps reporting a no-op rather than a startup failure.
    pub sentry_dsn: Option<String>,
    pub env: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8080),
            shared_url: env::var("SHARED_URL")
                .unwrap_or_else(|_| "http://localhost:8081".to_string()),
            game_systems_api_url: env::var("GAME_SYSTEMS_API_URL")
                .unwrap_or_else(|_| "http://localhost:8000".to_string()),
            otlp_endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(),
            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            shared_session_redis_host: env::var("SHARED_SESSION_REDIS_HOST")
                .ok()
                .filter(|v| !v.is_empty()),
            shared_session_redis_port: env::var("SHARED_SESSION_REDIS_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(6379),
            shared_session_redis_db: env::var("SHARED_SESSION_REDIS_DB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            shared_session_redis_pass: env::var("SHARED_SESSION_REDIS_PASS")
                .ok()
                .filter(|v| !v.is_empty()),
            sentry_dsn: env::var("SENTRY_DSN").ok().filter(|v| !v.is_empty()),
            env: env::var("ENV").unwrap_or_else(|_| "dev".to_string()),
        }
    }
}
