use axum::http::StatusCode;
use axum::routing::get;
use axum::Router;

/// Liveness/readiness convention shared across the platform's services (see
/// `docs/service-conventions.md`): both endpoints simply confirm the process is up, since this
/// service has no database/cache dependency whose health would need to be reflected separately.
pub fn router() -> Router {
    Router::new()
        .route("/healthz", get(ok))
        .route("/readyz", get(ok))
}

async fn ok() -> StatusCode {
    StatusCode::OK
}
