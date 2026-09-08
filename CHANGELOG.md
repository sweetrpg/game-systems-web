# Changelog

## 0.1.0 - 2026-09-08

### Added
- Crate skeleton and three pages
- Kubernetes base + dev/local overlays

### Fixed
- Health endpoints per PADR-0017, session decode tests
- Use redis.sweetrpg-auth host for shared session
- Shared session host is cache.sweetrpg-auth, not redis.
- Read auth-web's shared-session token under its real key
