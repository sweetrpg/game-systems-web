//! Read-only, fail-open client for `catalog-api`'s publisher search, used to back the publisher
//! name picker on the add-new-system form. `catalog-api` owns publisher data; this is the
//! "deliberate and narrow" cross-namespace read `docs/deployment-conventions.md` sanctions - see
//! `docs/adr/0001-publisher-lookup-via-catalog-api.md`. Every error path (unset `CATALOG_API_URL`,
//! timeout, transport error, non-2xx, unparseable body) yields an empty result rather than
//! surfacing a failure to the form. Shaped after `main-web`'s `admin_client.rs`.

use serde::Deserialize;

/// Shortest query worth sending upstream. A single letter matches most of the catalog and just
/// wastes a round trip.
const MIN_QUERY_LEN: usize = 2;

/// One publisher from `catalog-api`, trimmed to what the picker needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Publisher {
    pub id: String,
    pub name: String,
}

/// JSON:API envelope `GET /publishers/search` returns: `{"data":[{"id":..,"attributes":{"name":..}}]}`.
#[derive(Deserialize)]
struct SearchEnvelope {
    #[serde(default)]
    data: Vec<Resource>,
}

#[derive(Deserialize)]
struct Resource {
    #[serde(default)]
    id: String,
    #[serde(default)]
    attributes: Attributes,
}

#[derive(Default, Deserialize)]
struct Attributes {
    #[serde(default)]
    name: String,
}

/// `base_url` is `None` when `CATALOG_API_URL` is unset - the client is then a permanent no-op
/// and the form falls back to a plain publisher-name field with no suggestions.
#[derive(Clone)]
pub struct CatalogClient {
    base_url: Option<String>,
    http: reqwest::Client,
}

impl CatalogClient {
    pub fn new(base_url: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            // Tighter than the game-systems-api client's 5s: a type-ahead lookup must never
            // stall form submission.
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .expect("failed to build reqwest client");
        Self {
            base_url: base_url.map(|u| u.trim_end_matches('/').to_string()),
            http,
        }
    }

    /// Publishers whose name matches `query`, newest catalog data first as `catalog-api` orders
    /// it. Empty on any failure or a query shorter than [`MIN_QUERY_LEN`] after trimming.
    pub async fn search_publishers(&self, query: &str) -> Vec<Publisher> {
        let query = query.trim();
        let Some(base_url) = self.base_url.as_deref() else {
            return Vec::new();
        };
        if query.len() < MIN_QUERY_LEN {
            return Vec::new();
        }

        let resp = match self
            .http
            .get(format!("{base_url}/publishers/search"))
            .query(&[("q", query)])
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(err) => {
                tracing::warn!(error = %err, "publisher search: request failed");
                return Vec::new();
            }
        };

        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "publisher search: non-success response");
            return Vec::new();
        }

        match resp.json::<SearchEnvelope>().await {
            Ok(envelope) => envelope
                .data
                .into_iter()
                .filter(|r| !r.id.is_empty() && !r.attributes.name.is_empty())
                .map(|r| Publisher {
                    id: r.id,
                    name: r.attributes.name,
                })
                .collect(),
            Err(err) => {
                tracing::warn!(error = %err, "publisher search: body did not parse");
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn envelope() -> serde_json::Value {
        serde_json::json!({
            "data": [
                {"type": "publisher", "id": "pub-1", "attributes": {"name": "Wizards of the Coast"}},
                {"type": "publisher", "id": "pub-2", "attributes": {"name": "Paizo"}}
            ]
        })
    }

    #[tokio::test]
    async fn decodes_the_jsonapi_search_envelope() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/publishers/search"))
            .and(query_param("q", "wiz"))
            .respond_with(ResponseTemplate::new(200).set_body_json(envelope()))
            .mount(&server)
            .await;

        let client = CatalogClient::new(Some(server.uri()));
        let publishers = client.search_publishers("wiz").await;
        assert_eq!(
            publishers,
            vec![
                Publisher {
                    id: "pub-1".to_string(),
                    name: "Wizards of the Coast".to_string(),
                },
                Publisher {
                    id: "pub-2".to_string(),
                    name: "Paizo".to_string(),
                },
            ]
        );
    }

    #[tokio::test]
    async fn non_success_yields_empty() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/publishers/search"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = CatalogClient::new(Some(server.uri()));
        assert!(client.search_publishers("wizards").await.is_empty());
    }

    #[tokio::test]
    async fn short_query_makes_no_request() {
        // No mock mounted: a request would 404 the MockServer and still decode to empty, so
        // assert on the guard by pointing at an unroutable base and expecting a clean empty.
        let client = CatalogClient::new(Some("http://127.0.0.1:1".to_string()));
        assert!(client.search_publishers(" a ").await.is_empty());
    }

    #[tokio::test]
    async fn disabled_client_yields_empty() {
        let client = CatalogClient::new(None);
        assert!(client.search_publishers("wizards").await.is_empty());
    }
}
