use serde::{Deserialize, Serialize};

/// One tag on a game system, mirroring `model-core.go`'s `Tag`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Tag {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
}

/// The flattened current view `game-systems-api` returns from `GET /systems` and
/// `GET /systems/:id` - the current version's substantive fields plus the record's audit block.
/// Only the fields the frontend renders are modeled; unknown fields are ignored.
#[derive(Clone, Debug, Deserialize)]
pub struct GameSystemView {
    pub id: String,
    #[serde(default)]
    pub record_id: String,
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub edition: String,
    #[serde(default)]
    pub publisher_id: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub tags: Vec<Tag>,
}

/// One historical version row from `GET /systems/:id/versions`.
#[derive(Clone, Debug, Deserialize)]
pub struct GameSystemVersionRow {
    #[serde(default)]
    pub version: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub edition: String,
    #[serde(default)]
    pub state: String,
}

/// The paginated envelope `GET /systems` returns once query-layer pagination lands
/// (`openspec/changes/game-systems-web`, task group 1). `total` is the count of live systems
/// matching the search, independent of the page size.
#[derive(Clone, Debug)]
pub struct SystemsPage {
    pub systems: Vec<GameSystemView>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

/// `GET /systems` is served in two shapes during this change's rollout: `game-systems-api`
/// currently returns a bare JSON array of every live system (pre task group 1); afterwards it
/// returns the `{systems,total,page,per_page}` envelope. Accept either so the browse page
/// works before and after the API change.
#[derive(Deserialize)]
#[serde(untagged)]
enum SystemsPayload {
    Envelope {
        #[serde(default)]
        systems: Vec<GameSystemView>,
        #[serde(default)]
        total: i64,
        #[serde(default)]
        page: u32,
        #[serde(default)]
        per_page: u32,
    },
    BareList(Vec<GameSystemView>),
}

impl From<SystemsPayload> for SystemsPage {
    fn from(payload: SystemsPayload) -> Self {
        match payload {
            SystemsPayload::Envelope {
                systems,
                total,
                page,
                per_page,
            } => SystemsPage {
                total: if total > 0 {
                    total
                } else {
                    systems.len() as i64
                },
                page: page.max(1),
                per_page: per_page.max(1),
                systems,
            },
            // Bare array: the API returned every live system with no server-side paging. Show
            // them all as one page; the pager collapses (has_next is false). Replaced by real
            // pagination once task group 1 ships.
            SystemsPayload::BareList(systems) => SystemsPage {
                total: systems.len() as i64,
                page: 1,
                per_page: systems.len().max(1) as u32,
                systems,
            },
        }
    }
}

/// Query parameters for the browse/search list, forwarded verbatim to `game-systems-api`. The
/// frontend never filters, sorts, or paginates in-process - see the `game-systems-web` and
/// `game-systems-catalog` specs.
#[derive(Clone, Debug, Default)]
pub struct ListQuery {
    pub search: Option<String>,
    pub sort: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

/// Body for `POST /systems`. `system_id` is the stable kebab-case slug; the rest are the new
/// system's initial version fields.
#[derive(Clone, Debug, Default, Serialize)]
pub struct NewSystem {
    pub system_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub edition: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub publisher_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub notes: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Tag>,
}

/// `game-systems-api`'s error envelope (`api-core.go`'s `ErrorVO`).
#[derive(Clone, Debug, Default, Deserialize)]
pub struct ApiError {
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub message: String,
}

/// What went wrong talking to `game-systems-api`, mapped by the route layer to the right shared
/// error page or a re-rendered form.
#[derive(Debug)]
pub enum ClientError {
    /// The system id resolved to nothing - render the shared 404 page.
    NotFound,
    /// The caller lacks a write role at the API - the add form's authoritative rejection.
    Forbidden(ApiError),
    /// The API rejected the submission (validation, duplicate slug) - re-render the form.
    BadRequest(ApiError),
    /// Transport failure or a 5xx from the API - render the shared 502/503 page.
    Upstream(String),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::NotFound => write!(f, "not found"),
            ClientError::Forbidden(e) => write!(f, "forbidden: {}", e.message),
            ClientError::BadRequest(e) => write!(f, "bad request: {}", e.message),
            ClientError::Upstream(m) => write!(f, "upstream error: {m}"),
        }
    }
}

impl std::error::Error for ClientError {}

/// Thin `reqwest` client over `game-systems-api`. One `reqwest::Client` is built at startup and
/// cloned per call (cheap - it is an `Arc` internally).
#[derive(Clone)]
pub struct GameSystemsApiClient {
    base_url: String,
    http: reqwest::Client,
}

impl GameSystemsApiClient {
    pub fn new(base_url: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("failed to build reqwest client");
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// `GET /systems` with search/sort/pagination pushed to the query string.
    pub async fn list(&self, query: &ListQuery) -> Result<SystemsPage, ClientError> {
        let mut params: Vec<(&str, String)> = Vec::new();
        if let Some(q) = query.search.as_ref().filter(|q| !q.is_empty()) {
            params.push(("q", q.clone()));
        }
        if let Some(sort) = query.sort.as_ref().filter(|s| !s.is_empty()) {
            params.push(("sort", sort.clone()));
        }
        if let Some(page) = query.page {
            params.push(("page", page.to_string()));
        }
        if let Some(per_page) = query.per_page {
            params.push(("per_page", per_page.to_string()));
        }
        let resp = self
            .http
            .get(self.url("/systems"))
            .query(&params)
            .send()
            .await
            .map_err(|e| ClientError::Upstream(e.to_string()))?;
        let payload: SystemsPayload = self.decode(resp).await?;
        Ok(payload.into())
    }

    /// `GET /systems/:id` - resolves `id` as a document id or a `system_id` slug server-side.
    pub async fn get(&self, id: &str) -> Result<GameSystemView, ClientError> {
        let resp = self
            .http
            .get(self.url(&format!("/systems/{id}")))
            .send()
            .await
            .map_err(|e| ClientError::Upstream(e.to_string()))?;
        self.decode(resp).await
    }

    /// `GET /systems/:id/versions` - full version history, newest-first as the API orders it.
    pub async fn versions(&self, id: &str) -> Result<Vec<GameSystemVersionRow>, ClientError> {
        let resp = self
            .http
            .get(self.url(&format!("/systems/{id}/versions")))
            .send()
            .await
            .map_err(|e| ClientError::Upstream(e.to_string()))?;
        self.decode(resp).await
    }

    /// `POST /systems` - forwards the caller's bearer token so `game-systems-api`'s `authz`
    /// middleware runs the authoritative `RequireAnyRole` check. Returns the created system's
    /// view on success.
    pub async fn create(
        &self,
        bearer_token: &str,
        body: &NewSystem,
    ) -> Result<GameSystemView, ClientError> {
        let resp = self
            .http
            .post(self.url("/systems"))
            .bearer_auth(bearer_token)
            .json(body)
            .send()
            .await
            .map_err(|e| ClientError::Upstream(e.to_string()))?;
        self.decode(resp).await
    }

    async fn decode<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T, ClientError> {
        let status = resp.status();
        if status.is_success() {
            return resp
                .json::<T>()
                .await
                .map_err(|e| ClientError::Upstream(format!("decode: {e}")));
        }
        let api_err = resp.json::<ApiError>().await.unwrap_or_default();
        match status {
            reqwest::StatusCode::NOT_FOUND => Err(ClientError::NotFound),
            reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
                Err(ClientError::Forbidden(api_err))
            }
            s if s.is_client_error() => Err(ClientError::BadRequest(api_err)),
            s => Err(ClientError::Upstream(format!("api returned {s}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn list_decodes_the_paginated_envelope() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/systems"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "systems": [{"id": "gs1", "name": "Dungeon Quest", "edition": "1e"}],
                "total": 1,
                "page": 1,
                "per_page": 24
            })))
            .mount(&server)
            .await;

        let client = GameSystemsApiClient::new(server.uri());
        let page = client.list(&ListQuery::default()).await.unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.systems.len(), 1);
        assert_eq!(page.systems[0].name, "Dungeon Quest");
    }

    #[tokio::test]
    async fn list_decodes_a_bare_array_from_the_pre_pagination_api() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/systems"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {"id": "gs1", "name": "Dungeon Quest", "edition": "1e"},
                {"id": "gs2", "name": "Star Frontier", "edition": "2e"}
            ])))
            .mount(&server)
            .await;

        let client = GameSystemsApiClient::new(server.uri());
        let page = client.list(&ListQuery::default()).await.unwrap();
        assert_eq!(page.systems.len(), 2);
        assert_eq!(page.total, 2);
        assert_eq!(page.page, 1);
        assert_eq!(page.systems[1].name, "Star Frontier");
    }

    #[tokio::test]
    async fn get_maps_404_to_not_found() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/systems/missing"))
            .respond_with(ResponseTemplate::new(404).set_body_json(serde_json::json!({})))
            .mount(&server)
            .await;

        let client = GameSystemsApiClient::new(server.uri());
        assert!(matches!(
            client.get("missing").await,
            Err(ClientError::NotFound)
        ));
    }

    #[tokio::test]
    async fn create_maps_403_to_forbidden() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/systems"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "error": "forbidden",
                "message": "requires a write role"
            })))
            .mount(&server)
            .await;

        let client = GameSystemsApiClient::new(server.uri());
        let body = NewSystem {
            system_id: "dungeon-quest".to_string(),
            name: "Dungeon Quest".to_string(),
            ..Default::default()
        };
        match client.create("tok", &body).await {
            Err(ClientError::Forbidden(e)) => assert_eq!(e.error, "forbidden"),
            other => panic!("expected Forbidden, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn create_maps_400_to_bad_request_with_message() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/systems"))
            .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
                "error": "invalid_request",
                "message": "system_id already in use"
            })))
            .mount(&server)
            .await;

        let client = GameSystemsApiClient::new(server.uri());
        let body = NewSystem {
            system_id: "taken".to_string(),
            name: "Taken".to_string(),
            ..Default::default()
        };
        match client.create("tok", &body).await {
            Err(ClientError::BadRequest(e)) => assert_eq!(e.message, "system_id already in use"),
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }
}
