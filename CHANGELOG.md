
## 0.3.0 - 2026-09-08

### Added
- Version-history page at /{id}/versions



## 0.2.0 - 2026-09-08

### Added
- Adopt shared-web design system (nav, avatar menu, footer, class vocab)


### Fixed
- Build detail/version links from record_id



## 0.1.3 - 2026-09-08

### Fixed
- Accept bare-array /systems response, fix sign-in redirect target



## 0.1.2 - 2026-09-08

### Fixed
- Serve pages at the root, not under /game-systems



## 0.1.1 - 2026-09-08

### Documentation
- Restore header order after git-cliff 0.1.0 mangle


### Fixed
- Point openfeature featureflagsource at sweetrpg-system namespace


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
