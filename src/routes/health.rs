use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;

/// Health/readiness contract per PADR-0017 (`sweetrpg/platform`, `docs/adr/`):
///
/// - `GET /status/ping` - shallow liveness, always a bare 200, wired as *both* the readiness
///   and liveness probe. Never depends on an external datastore.
/// - `GET /status/health` - deep dependency check where a service has dependencies, a bare 200
///   otherwise. `game-systems-web` has no datastore of its own (the shared session Redis is
///   optional and read-only), so this is a bare 200.
pub fn router() -> Router {
    Router::new()
        .route("/status/ping", get(ok))
        .route("/status/health", get(ok))
}

async fn ok() -> StatusCode {
    StatusCode::OK
}
