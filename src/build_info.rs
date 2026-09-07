use std::env;
use std::fs;

/// Read from the JSON file the Dockerfile bakes in at build time (same `BUILD_INFO_PATH`
/// convention as `catalog-web`'s `BuildInfo.swift` / the Go services'
/// `docs/service-conventions.md`), not generated at runtime: falls back to placeholder values
/// when the file doesn't exist (local `cargo run`, outside a container).
#[derive(Clone, serde::Deserialize)]
pub struct BuildInfo {
    #[serde(default = "unset")]
    pub number: String,
    #[serde(default = "unset")]
    pub job: String,
    #[serde(default = "unset_sha")]
    pub sha: String,
    #[serde(default = "unset")]
    pub date: String,
    #[serde(default = "dev")]
    pub version: String,
}

fn unset() -> String {
    "unset".to_string()
}

/// 8 hex chars, so it needs no truncation guard where a real short commit SHA would - and
/// reads unmistakably as a placeholder rather than a real hash.
fn unset_sha() -> String {
    "deadbeef".to_string()
}

fn dev() -> String {
    "dev".to_string()
}

impl BuildInfo {
    pub fn load() -> Self {
        let path =
            env::var("BUILD_INFO_PATH").unwrap_or_else(|_| "/app/config/build-info.json".into());
        let mut info: BuildInfo = fs::read_to_string(&path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or(BuildInfo {
                number: unset(),
                job: unset(),
                sha: unset_sha(),
                date: unset(),
                version: dev(),
            });
        // BUILD_VERSION comes from a git tag (e.g. "v1.2.3") but the footer template already
        // prefixes a literal "v", so strip one here to avoid rendering "vv1.2.3".
        info.version = strip_v_prefix(info.version);
        info
    }
}

fn strip_v_prefix(version: String) -> String {
    version
        .strip_prefix('v')
        .map(str::to_string)
        .unwrap_or(version)
}

#[cfg(test)]
mod tests {
    use super::strip_v_prefix;

    #[test]
    fn strips_leading_v_from_tag_version() {
        assert_eq!(strip_v_prefix("v1.2.3".to_string()), "1.2.3");
    }

    #[test]
    fn leaves_bare_version_unchanged() {
        assert_eq!(strip_v_prefix("1.2.3".to_string()), "1.2.3");
    }

    #[test]
    fn leaves_dev_placeholder_unchanged() {
        assert_eq!(strip_v_prefix("dev".to_string()), "dev");
    }
}
