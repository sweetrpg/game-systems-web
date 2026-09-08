use std::sync::Arc;

use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Form;
use axum::Router;
use axum_extra::extract::CookieJar;
use serde::Deserialize;

use crate::game_systems_client::{ClientError, GameSystemView, ListQuery, NewSystem, Tag};
use crate::i18n::Tr;
use crate::session_client::{SessionUser, SESSION_COOKIE_NAME};
use crate::AppState;

/// Roles that may see and submit the add-new-system form. Mirrors `catalog-web`'s
/// `editCapableRoles` and `game-systems-api`'s `RequireAnyRole(Admin, Editor, Submitter)` on
/// `POST /systems` - UI gating only, the API is the authoritative gate.
const WRITE_ROLES: &[&str] = &["submitter", "editor", "admin"];

/// Allowed `sort` values, forwarded verbatim to `game-systems-api` (which applies its own
/// allowlist). Anything else is dropped so the query string can't carry an arbitrary value.
const ALLOWED_SORTS: &[&str] = &["name", "-name", "created", "-created"];

const DEFAULT_PER_PAGE: u32 = 24;
const MAX_PER_PAGE: u32 = 100;

/// Browser-facing base path of this app behind the shared-host Ingress. Links and redirects
/// carry it; the Ingress strips it before the request reaches the app (see `router`).
const BASE_PATH: &str = "/game-systems";

/// `auth-web` sits at `/auth` on the same shared host - login/logout are fixed paths, not
/// derived from `SHARED_URL` (which points at `shared-web`). Matches `main-web`.
fn login_url(return_to: &str) -> String {
    format!("/auth/login?return_to={}", urlencode(return_to))
}

const LOGOUT_URL: &str = "/auth/logout";

/// Routes are registered root-relative. The dev/local Ingress strips the `/game-systems` path
/// prefix (`strip-prefix-game-systems` middleware) before the request reaches the app, so it
/// sees `/`, `/new`, `/{id}` - same pattern as `game-room-web`. Browser-facing links and
/// redirects still carry the `/game-systems` prefix, since those go back through the Ingress.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(browse))
        .route("/new", get(new_form).post(submit_new))
        .route("/{id}", get(detail))
}

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

fn tr_for(jar: &CookieJar, headers: &HeaderMap) -> Tr {
    let accept_language = headers.get("accept-language").and_then(|v| v.to_str().ok());
    Tr::resolve(jar, accept_language)
}

async fn current_user(state: &AppState, jar: &CookieJar) -> Option<SessionUser> {
    let session_id = jar.get(SESSION_COOKIE_NAME)?.value().to_string();
    state.session_client.current_user(&session_id).await
}

fn has_write_role(user: &SessionUser) -> bool {
    user.roles.iter().any(|r| WRITE_ROLES.contains(&r.as_str()))
}

/// Maps an upstream client error to the HTTP status whose shared error page the platform's
/// `errors-shared-web` Traefik middleware renders.
fn upstream_status(err: &ClientError) -> StatusCode {
    match err {
        ClientError::NotFound => StatusCode::NOT_FOUND,
        ClientError::Forbidden(_) => StatusCode::FORBIDDEN,
        ClientError::BadRequest(_) => StatusCode::BAD_REQUEST,
        ClientError::Upstream(_) => StatusCode::BAD_GATEWAY,
    }
}

fn render<T: Template>(tpl: T) -> Response {
    match tpl.render() {
        Ok(body) => Html(body).into_response(),
        Err(err) => {
            tracing::error!(error = %err, "template render failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// GET /game-systems/:id  - detail page
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "detail.html")]
struct DetailTemplate {
    shared_url: String,
    version: String,
    build_hash: String,
    current_user_name: Option<String>,
    login_url: String,
    logout_url: String,
    tr: Tr,
    system: SystemView,
    version_history_url: String,
}

/// Template-facing projection of `GameSystemView` - flattens tags to display strings.
struct SystemView {
    name: String,
    edition: String,
    publisher_id: String,
    notes: String,
    tags: Vec<String>,
}

impl From<GameSystemView> for SystemView {
    fn from(v: GameSystemView) -> Self {
        Self {
            name: v.name,
            edition: v.edition,
            publisher_id: v.publisher_id,
            notes: v.notes,
            tags: v
                .tags
                .into_iter()
                .map(|t: Tag| {
                    if t.value.is_empty() {
                        t.name
                    } else {
                        format!("{}: {}", t.name, t.value)
                    }
                })
                .filter(|s| !s.is_empty())
                .collect(),
        }
    }
}

async fn detail(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let tr = tr_for(&jar, &headers);
    let user = current_user(&state, &jar).await;

    match state.api.get(&id).await {
        Ok(view) => {
            let system_id = view.id.clone();
            render(DetailTemplate {
                shared_url: state.config.shared_url.clone(),
                version: state.build_info.version.clone(),
                build_hash: state.build_info.sha.clone(),
                current_user_name: user.as_ref().map(|u| u.name.clone()),
                login_url: login_url(BASE_PATH),
                logout_url: LOGOUT_URL.to_string(),
                tr,
                version_history_url: format!("/game-systems/{system_id}/versions"),
                system: view.into(),
            })
        }
        Err(err) => {
            tracing::warn!(error = %err, system = %id, "detail lookup failed");
            upstream_status(&err).into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// GET /game-systems  - browse + search
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct BrowseParams {
    q: Option<String>,
    sort: Option<String>,
    page: Option<u32>,
    per_page: Option<u32>,
}

#[derive(Template)]
#[template(path = "browse.html")]
struct BrowseTemplate {
    shared_url: String,
    version: String,
    build_hash: String,
    current_user_name: Option<String>,
    login_url: String,
    logout_url: String,
    tr: Tr,
    search: String,
    sort: String,
    rows: Vec<BrowseRow>,
    can_add: bool,
    page: u32,
    has_prev: bool,
    has_next: bool,
    prev_url: String,
    next_url: String,
}

struct BrowseRow {
    id: String,
    name: String,
    edition: String,
}

fn page_url(search: &str, sort: &str, page: u32) -> String {
    let mut parts: Vec<String> = Vec::new();
    if !search.is_empty() {
        parts.push(format!("q={}", urlencode(search)));
    }
    if !sort.is_empty() {
        parts.push(format!("sort={}", urlencode(sort)));
    }
    parts.push(format!("page={page}"));
    format!("/game-systems?{}", parts.join("&"))
}

fn urlencode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

async fn browse(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    headers: HeaderMap,
    Query(params): Query<BrowseParams>,
) -> Response {
    let tr = tr_for(&jar, &headers);
    let user = current_user(&state, &jar).await;

    let search = params.q.unwrap_or_default().trim().to_string();
    let sort = params
        .sort
        .filter(|s| ALLOWED_SORTS.contains(&s.as_str()))
        .unwrap_or_default();
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params
        .per_page
        .unwrap_or(DEFAULT_PER_PAGE)
        .clamp(1, MAX_PER_PAGE);

    let query = ListQuery {
        search: (!search.is_empty()).then(|| search.clone()),
        sort: (!sort.is_empty()).then(|| sort.clone()),
        page: Some(page),
        per_page: Some(per_page),
    };

    match state.api.list(&query).await {
        Ok(result) => {
            let rows: Vec<BrowseRow> = result
                .systems
                .into_iter()
                .map(|s| BrowseRow {
                    id: s.id,
                    name: s.name,
                    edition: s.edition,
                })
                .collect();
            let has_prev = page > 1;
            let has_next = (page as i64) * (per_page as i64) < result.total;
            render(BrowseTemplate {
                shared_url: state.config.shared_url.clone(),
                version: state.build_info.version.clone(),
                build_hash: state.build_info.sha.clone(),
                current_user_name: user.as_ref().map(|u| u.name.clone()),
                login_url: login_url(BASE_PATH),
                logout_url: LOGOUT_URL.to_string(),
                tr,
                prev_url: page_url(&search, &sort, page.saturating_sub(1).max(1)),
                next_url: page_url(&search, &sort, page + 1),
                can_add: user.as_ref().map(has_write_role).unwrap_or(false),
                search,
                sort,
                rows,
                page,
                has_prev,
                has_next,
            })
        }
        Err(err) => {
            tracing::warn!(error = %err, "browse list failed");
            upstream_status(&err).into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// GET/POST /game-systems/new  - role-gated add form
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "new.html")]
struct NewTemplate {
    shared_url: String,
    version: String,
    build_hash: String,
    current_user_name: Option<String>,
    login_url: String,
    logout_url: String,
    tr: Tr,
    error: Option<String>,
    form: NewFormValues,
}

#[derive(Default)]
struct NewFormValues {
    system_id: String,
    name: String,
    edition: String,
    publisher_id: String,
    notes: String,
    tags: String,
}

#[derive(Debug, Default, Deserialize)]
struct NewFormSubmission {
    #[serde(default)]
    system_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    edition: String,
    #[serde(default)]
    publisher_id: String,
    #[serde(default)]
    notes: String,
    /// Comma-separated tag names.
    #[serde(default)]
    tags: String,
}

impl From<&NewFormSubmission> for NewFormValues {
    fn from(s: &NewFormSubmission) -> Self {
        Self {
            system_id: s.system_id.clone(),
            name: s.name.clone(),
            edition: s.edition.clone(),
            publisher_id: s.publisher_id.clone(),
            notes: s.notes.clone(),
            tags: s.tags.clone(),
        }
    }
}

fn parse_tags(raw: &str) -> Vec<Tag> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|name| Tag {
            name: name.to_string(),
            value: String::new(),
        })
        .collect()
}

fn new_page(
    state: &AppState,
    tr: Tr,
    user: &SessionUser,
    error: Option<String>,
    form: NewFormValues,
) -> Response {
    render(NewTemplate {
        shared_url: state.config.shared_url.clone(),
        version: state.build_info.version.clone(),
        build_hash: state.build_info.sha.clone(),
        current_user_name: Some(user.name.clone()),
        login_url: login_url(BASE_PATH),
        logout_url: LOGOUT_URL.to_string(),
        tr,
        error,
        form,
    })
}

async fn new_form(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    headers: HeaderMap,
) -> Response {
    let tr = tr_for(&jar, &headers);
    let Some(user) = current_user(&state, &jar).await else {
        return Redirect::to(&login_url(&format!("{BASE_PATH}/new"))).into_response();
    };
    if !has_write_role(&user) {
        return (StatusCode::FORBIDDEN, Html(tr.new_not_authorized())).into_response();
    }
    new_page(&state, tr, &user, None, NewFormValues::default())
}

async fn submit_new(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    headers: HeaderMap,
    Form(submission): Form<NewFormSubmission>,
) -> Response {
    let tr = tr_for(&jar, &headers);
    let Some(user) = current_user(&state, &jar).await else {
        return Redirect::to(&login_url(&format!("{BASE_PATH}/new"))).into_response();
    };
    if !has_write_role(&user) {
        return (StatusCode::FORBIDDEN, Html(tr.new_not_authorized())).into_response();
    }

    let Some(token) = user.access_token.as_deref().filter(|t| !t.is_empty()) else {
        // Session predates auth-web carrying an access token - can't authenticate the write.
        tracing::warn!("add-new submit blocked: session has no access token to forward");
        return new_page(
            &state,
            tr.clone(),
            &user,
            Some(tr.new_not_authorized()),
            NewFormValues::from(&submission),
        );
    };

    let body = NewSystem {
        system_id: submission.system_id.trim().to_string(),
        name: submission.name.trim().to_string(),
        edition: submission.edition.trim().to_string(),
        publisher_id: submission.publisher_id.trim().to_string(),
        notes: submission.notes.trim().to_string(),
        tags: parse_tags(&submission.tags),
    };

    match state.api.create(token, &body).await {
        Ok(created) => Redirect::to(&format!("/game-systems/{}", created.id)).into_response(),
        Err(ClientError::BadRequest(e)) | Err(ClientError::Forbidden(e)) => new_page(
            &state,
            tr,
            &user,
            Some(e.message),
            NewFormValues::from(&submission),
        ),
        Err(err) => {
            tracing::warn!(error = %err, "add-new submit upstream failure");
            upstream_status(&err).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_with_roles(roles: &[&str]) -> SessionUser {
        SessionUser {
            sub: "u1".into(),
            name: "Tester".into(),
            email: None,
            roles: roles.iter().map(|s| s.to_string()).collect(),
            access_token: Some("tok".into()),
            expiry: chrono::Utc::now() + chrono::Duration::hours(1),
        }
    }

    #[test]
    fn write_role_detected_for_submitter() {
        assert!(has_write_role(&user_with_roles(&["viewer", "submitter"])));
    }

    #[test]
    fn no_write_role_for_plain_viewer() {
        assert!(!has_write_role(&user_with_roles(&["viewer"])));
    }

    #[test]
    fn disallowed_sort_is_dropped() {
        let params = BrowseParams {
            sort: Some("notes".into()),
            ..Default::default()
        };
        let sort = params
            .sort
            .filter(|s| ALLOWED_SORTS.contains(&s.as_str()))
            .unwrap_or_default();
        assert!(sort.is_empty());
    }

    #[test]
    fn page_url_carries_search_and_sort() {
        let u = page_url("d&d", "-name", 3);
        assert!(u.contains("q=d%26d"));
        assert!(u.contains("sort=-name"));
        assert!(u.contains("page=3"));
    }

    #[test]
    fn parse_tags_splits_and_trims() {
        let tags = parse_tags(" fantasy, , d20 ");
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].name, "fantasy");
        assert_eq!(tags[1].name, "d20");
    }

    // The Ingress strips /game-systems, so the app must serve its pages at the root. A
    // regression here 404s every page in dev while /status/ping still answers (see
    // sweetrpg/game-systems-web v0.1.2).
    async fn route_response(uri: &str) -> axum::http::Response<axum::body::Body> {
        use tower::ServiceExt;
        let state = std::sync::Arc::new(crate::AppState {
            config: crate::config::Config::from_env(),
            build_info: crate::build_info::BuildInfo::load(),
            session_client: crate::session_client::SessionClient::new(None, 6379, 0, None).await,
            api: crate::game_systems_client::GameSystemsApiClient::new(
                "http://127.0.0.1:1".to_string(),
            ),
        });
        router()
            .with_state(state)
            .oneshot(
                axum::http::Request::builder()
                    .uri(uri)
                    .body(axum::body::Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn route_status(uri: &str) -> axum::http::StatusCode {
        route_response(uri).await.status()
    }

    #[tokio::test]
    async fn pages_are_served_at_the_root_not_under_game_systems() {
        // Browse: session disabled + API unreachable -> 502, not 404. Route matched.
        assert_eq!(route_status("/").await, axum::http::StatusCode::BAD_GATEWAY);
        // Add form with no session -> redirect to auth-web's sign-in, not 404, not shared-web.
        let new_resp = route_response("/new").await;
        assert_eq!(new_resp.status(), axum::http::StatusCode::SEE_OTHER);
        let location = new_resp
            .headers()
            .get(axum::http::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(
            location.starts_with("/auth/login?return_to="),
            "unexpected sign-in redirect target: {location}"
        );
        // Detail for some id -> upstream 502, not 404. Route matched.
        assert_eq!(
            route_status("/dungeon-quest").await,
            axum::http::StatusCode::BAD_GATEWAY
        );
        // A nested path is registered nowhere - the routes are one segment deep, at the root.
        assert_eq!(
            route_status("/game-systems/dungeon-quest").await,
            axum::http::StatusCode::NOT_FOUND
        );
    }
}
