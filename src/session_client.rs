use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use serde::Deserialize;

/// Shared across every frontend - see design.md's "One shared cookie" decision in the
/// add-user-api-authn-authz OpenSpec change. `auth-web` is the only writer; this app only
/// reads it.
pub const SESSION_COOKIE_NAME: &str = "sweetrpg_session";

/// The identity `auth-web` writes into the shared session store - mirrors its own
/// `SessionUser` model. `roles` comes from `users-api`'s verified `/authz/check` response, not
/// an unverified local decode.
#[derive(Clone, Debug, Deserialize)]
pub struct SessionUser {
    #[allow(dead_code)]
    pub sub: String,
    pub name: String,
    pub email: Option<String>,
    pub roles: Vec<String>,
    /// Bearer access token for the signed-in principal, written into the shared session by
    /// `auth-web`. `game-systems-web` forwards this on `POST /systems` so `game-systems-api`'s
    /// `authz` middleware can run the authoritative `RequireAnyRole` check (design.md,
    /// decision 3). `None` on a session written before `auth-web` began carrying it - the
    /// add-new form's write path treats that as unable to authenticate and does not call the
    /// API. Not used by `main-web`, whose copy of this struct simply ignores the extra field.
    #[serde(default)]
    pub access_token: Option<String>,
    /// When this session becomes invalid, set by `auth-web` at write time. A session at or
    /// past this timestamp must be treated as absent, not stale data - see
    /// `sweetrpg/platform`'s `docs/frontend-conventions.md` ("Shared session schema").
    /// Enforced independently at the Redis key level by `ResilientRedisSessionDriver`'s TTL;
    /// this check is defense in depth since this app's read-only client never goes through
    /// that driver.
    pub expiry: DateTime<Utc>,
}

/// Reads (never writes) the shared session `auth-web` establishes. Fails open: any error
/// (disabled, unreachable Redis, missing/malformed session) is treated as "no session" rather
/// than surfacing a failure to the page being decorated - the same fail-open contract Vapor's
/// `ResilientRedisSessionDriver` gives every Swift frontend, applied here since main-web isn't
/// Vapor and has no session driver of its own to plug into.
///
/// `client` is built once (cheap - just URL parsing, doesn't touch the network) and kept for
/// the app's lifetime. The actual `ConnectionManager` is built lazily on first use and cached
/// in `manager` - if that first attempt fails (e.g. Redis isn't accepting connections yet
/// during a rollout), the *next* call tries again instead of staying disabled forever. Once one
/// attempt succeeds, `ConnectionManager`'s own built-in reconnect logic handles later transient
/// blips, same as before - this only fixes the "never got a first connection" case. See
/// sweetrpg/main-web#214: a boot-time race with a Redis rename left this permanently disabled
/// for a pod's entire lifetime, with no way to self-heal short of a manual restart.
pub struct SessionClient {
    client: Option<redis::Client>,
    manager: Mutex<Option<ConnectionManager>>,
}

impl SessionClient {
    /// `host` is `None` when `SHARED_SESSION_REDIS_HOST` is unset - the client is then
    /// permanently disabled (every lookup immediately returns `None`, no network calls at all).
    /// `password` is `None` when `SHARED_SESSION_REDIS_PASS` is unset, matching an
    /// unauthenticated Redis instance (e.g. some local setups) - matches `catalog-web`'s
    /// `SHARED_SESSION_REDIS_PASS` convention (`configure.swift`), which this app previously
    /// had no equivalent of at all.
    pub async fn new(host: Option<String>, port: u16, db: u16, password: Option<String>) -> Self {
        let Some(host) = host else {
            tracing::warn!("SHARED_SESSION_REDIS_HOST not set - shared session reads disabled, every visitor treated as logged-out");
            return Self {
                client: None,
                manager: Mutex::new(None),
            };
        };

        let credentials = password
            .as_deref()
            .map(|pass| format!(":{pass}@"))
            .unwrap_or_default();
        let url = format!("redis://{credentials}{host}:{port}/{db}");
        let client = match redis::Client::open(url) {
            Ok(client) => Some(client),
            Err(err) => {
                tracing::warn!(error = %err, "invalid SHARED_SESSION_REDIS_HOST, sessions will read as logged-out");
                None
            }
        };
        Self {
            client,
            manager: Mutex::new(None),
        }
    }

    /// Returns the cached connection if one exists, otherwise tries to build one. Every call
    /// with no cached connection retries - see the struct doc comment for why this matters.
    async fn connection(&self) -> Option<ConnectionManager> {
        let client = self.client.as_ref()?;
        if let Some(manager) = self.manager.lock().unwrap().as_ref() {
            return Some(manager.clone());
        }
        // Deliberately not held across this await - a concurrent caller finding no cached
        // manager either might build its own redundant connection too, which is harmless
        // (both work independently, one is simply discarded) and simpler than a lock that
        // spans an .await point.
        match ConnectionManager::new(client.clone()).await {
            Ok(manager) => {
                *self.manager.lock().unwrap() = Some(manager.clone());
                Some(manager)
            }
            Err(err) => {
                tracing::warn!(error = %err, "failed to connect to shared session Redis, sessions will read as logged-out");
                None
            }
        }
    }

    /// Looks up the session for a session ID (the shared cookie's raw value). Never errors -
    /// any failure (disabled client, unreachable Redis, missing key, malformed JSON) degrades
    /// to `None`.
    pub async fn current_user(&self, session_id: &str) -> Option<SessionUser> {
        let mut manager = self.connection().await?;

        // Key format matches Vapor's ResilientRedisSessionDriver (auth-web's session writer):
        // `vrs-<sessionID>`, JSON-encoded Vapor `SessionData` (a flat `[String: String]`).
        let key = format!("vrs-{session_id}");
        let raw: Option<String> = match manager.get(&key).await {
            Ok(raw) => raw,
            Err(err) => {
                tracing::warn!(error = %err, "shared session Redis read failed, treating as logged-out");
                return None;
            }
        };
        let raw = raw?;

        let session_data: HashMap<String, String> = match serde_json::from_str(&raw) {
            Ok(data) => data,
            Err(err) => {
                tracing::warn!(error = %err, "malformed shared session data, treating as logged-out");
                return None;
            }
        };
        let user_json = session_data.get("user")?;
        let user: SessionUser = match serde_json::from_str(user_json) {
            Ok(user) => user,
            Err(err) => {
                tracing::warn!(error = %err, "malformed shared session user, treating as logged-out");
                return None;
            }
        };
        if !user.is_active(Utc::now()) {
            return None;
        }
        Some(user)
    }
}

impl SessionUser {
    /// A session is active only strictly before its `expiry`. At or past that instant it must
    /// be treated as absent, not as stale-but-usable data (see the `expiry` field doc).
    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        self.expiry > now
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    const SAMPLE_SESSION_USER: &str = r#"{
        "sub": "auth0|abc123",
        "name": "Ada Lovelace",
        "email": "ada@example.com",
        "roles": ["submitter", "viewer"],
        "access_token": "tok-xyz",
        "expiry": "2999-01-01T00:00:00Z"
    }"#;

    #[test]
    fn decodes_a_sample_shared_session_user() {
        let user: SessionUser = serde_json::from_str(SAMPLE_SESSION_USER).unwrap();
        assert_eq!(user.name, "Ada Lovelace");
        assert_eq!(user.email.as_deref(), Some("ada@example.com"));
        assert_eq!(user.roles, vec!["submitter", "viewer"]);
        assert_eq!(user.access_token.as_deref(), Some("tok-xyz"));
        assert!(user.is_active(Utc::now()));
    }

    #[test]
    fn a_session_at_or_after_expiry_is_inactive() {
        let user: SessionUser = serde_json::from_str(SAMPLE_SESSION_USER).unwrap();
        let expiry = user.expiry;
        assert!(
            !user.is_active(expiry),
            "exactly at expiry must read as absent"
        );
        assert!(!user.is_active(expiry + Duration::seconds(1)));
        assert!(user.is_active(expiry - Duration::seconds(1)));
    }

    #[test]
    fn session_user_without_access_token_decodes_with_none() {
        let json =
            r#"{"sub":"s","name":"N","email":null,"roles":[],"expiry":"2999-01-01T00:00:00Z"}"#;
        let user: SessionUser = serde_json::from_str(json).unwrap();
        assert!(user.access_token.is_none());
    }

    #[tokio::test]
    async fn disabled_client_returns_no_user_without_making_a_request() {
        let client = SessionClient::new(None, 6379, 0, None).await;
        assert!(client.current_user("any-session-id").await.is_none());
    }

    #[tokio::test]
    async fn unreachable_redis_fails_open() {
        // Port 1 is a reserved/unassigned port that refuses connections immediately on any
        // platform this runs on - exercises the connection-failure branch without a mock
        // server or real timeout wait.
        let client = SessionClient::new(Some("127.0.0.1".to_string()), 1, 0, None).await;
        assert!(client.current_user("any-session-id").await.is_none());
    }

    #[tokio::test]
    async fn unreachable_redis_fails_open_with_a_password_configured() {
        // Exercises the credentialed URL construction path (unreachable, so no real auth
        // happens) without needing a real authenticated Redis instance in tests.
        let client = SessionClient::new(
            Some("127.0.0.1".to_string()),
            1,
            0,
            Some("s3cret".to_string()),
        )
        .await;
        assert!(client.current_user("any-session-id").await.is_none());
    }

    #[tokio::test]
    async fn repeated_calls_after_a_failed_connection_dont_panic_or_deadlock() {
        // Exercises connection()'s retry path (Mutex locked, released before the .await, then
        // re-locked to store a result) across multiple calls with no cached manager - a
        // regression check for sweetrpg/main-web#214's fix, which replaced a single boot-time
        // connection attempt with retry-on-next-call.
        let client = SessionClient::new(Some("127.0.0.1".to_string()), 1, 0, None).await;
        assert!(client.current_user("any-session-id").await.is_none());
        assert!(client.current_user("any-session-id").await.is_none());
    }
}
