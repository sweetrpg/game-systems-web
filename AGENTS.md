# AGENTS.md

This file provides guidance to Claude Code, Codex, GitHub Copilot, and other coding agents
working in this repository.

## About This Project

`game-systems-web` is the server-rendered frontend for the platform's game systems catalog,
serving `dev.sweetrpg.com/game-systems` (and `sweetrpg.com/game-systems` in production):

- **Detail page** `GET /game-systems/:id` - one system's flattened current view (name, edition,
  publisher reference, notes, tags) plus a link to its version history.
- **Browse + search** `GET /game-systems` - a paginated list of live systems with
  case-insensitive name search and sorting. Search, sort, page, and page size are forwarded to
  `game-systems-api` as query parameters; this frontend never fetches the whole catalog and
  filters it in-process.
- **Add-new form** `GET`/`POST /game-systems/new` - gated on an authenticated suite session
  whose roles include a `game-systems-api` write role (Admin, Editor, or Submitter); submits
  `POST /systems`.

It is the read/write face of `game-systems-api`, mirroring `catalog-web`'s role for the volume
catalog.

This is the platform's **second Rust frontend**. `main-web` (`sweetrpg/main-web`) is the
language reference implementation - its `Cargo.toml`, `src/` layout, `SessionClient`,
`telemetry.rs`, `i18n.rs`, and Kubernetes overlays are the pattern this repo follows. Read it
alongside `sweetrpg/platform`'s `docs/rust-service-conventions.md` before making structural
changes here. Where this repo diverges from `main-web`, the divergence should be deliberate and
noted, not accidental drift.

### Framework and templating

- **Axum** for routing and middleware (Tokio-native, first-class `tower` compatibility for
  tracing and metrics).
- **Askama** for templates (`templates/*.html`) - compile-time-checked against the context
  struct.

### Upstream: `game-systems-api`

`GameSystemsApiClient` (`src/game_systems_client.rs`, `reqwest`) calls `game-systems-api` for
`GET /systems`, `GET /systems/:id`, `GET /systems/:id/versions`, and `POST /systems`. In-cluster
it targets `api-v1.sweetrpg-game-systems.svc.cluster.local` via `GAME_SYSTEMS_API_URL`; internal
calls bypass the ingress, so no `/api/0` vs `/api/1` version prefix applies. Any upstream
failure or timeout renders the shared 502/503 error page rather than an unhandled exception.

### Upstream: `catalog-api` (publisher picker only)

`CatalogClient` (`src/catalog_client.rs`) calls `catalog-api` `GET /publishers/search?q=`
read-only to back the publisher name picker on the add-new form (`catalog-api` owns publisher
data). Gated on `CATALOG_API_URL`; in-cluster it targets
`api-v1.sweetrpg-catalog.svc.cluster.local`, cross-namespace, pinned to `api-v1` per
`docs/deployment-conventions.md`. Fail-open: an unset var, timeout, non-2xx, or bad body yields
no suggestions rather than an error, and the typed name is resolved to an id server-side on
submit. See `docs/adr/0001-publisher-lookup-via-catalog-api.md`.

### Session and role gating

`SessionClient` (`src/session_client.rs`, copied from `main-web`) reads the suite session from
the shared Redis session store without writing it (`SHARED_SESSION_REDIS_HOST`/`PORT`/`DB`/
`PASS`; `REDIS_DB=1` in `sweetrpg-game-systems`). A session at or past its `expiry` timestamp is
treated as absent.

The add-new form is available iff `session.roles` intersects `{"submitter","editor","admin"}` -
the same UI-gating precedent `catalog-web` sets, with no live authz call. `game-systems-api`'s
own `RequireAnyRole` check is the authoritative gate on write; a 403 from the API is surfaced on
the re-rendered form. If the shared session's roles are ever scoped differently from how
`game-systems-api` scopes them, a user could see the form and then get a 403 on submit - the
same failure mode `catalog-web` accepts, and it fails safe.

### Localization

User-facing strings come from `locales/<code>.yml` via `rust-i18n`, never hardcoded in
templates. English is the default/fallback locale. Locale resolution per request: `locale`
cookie override, then `Accept-Language` (first tag, base subtag matched), then English. Askama
receives a `Tr` translator (`src/i18n.rs`) and calls its methods; dynamic keys go through
`tr.get(...)`. CI's `locale-lint` job runs `scripts/check-template-strings.sh`, which fails on
literal text between HTML tags that isn't a whitelisted brand string or a `{{ tr.* }}` lookup.

### Shared static assets and error pages

Suite-wide branding (logo, favicon, Broadsheet design tokens, theme JS) is served by
`shared-web` via `SHARED_URL` (`src/config.rs`), never a hardcoded host. Generic HTTP error
responses (400/401/403/404/500/502/503/504) route to `shared-web`'s branded `/errors/<code>`
pages.

### Build info / version footer

`src/build_info.rs` reads `BUILD_INFO_PATH` (default `/app/config/build-info.json`), baked into
the image by the `Dockerfile` from the `BUILD_*` build args. Falls back to placeholder values
outside a container.

## Observability

- **Logging**: `tracing` + `tracing-subscriber` JSON formatter to stdout. `LOG_LEVEL` env var
  (`EnvFilter` syntax), default `info`.
- **Tracing**: OTLP/HTTP via `opentelemetry-otlp` (`src/telemetry.rs`) to
  `OTEL_EXPORTER_OTLP_ENDPOINT`. Provider shutdown is deferred to the end of `main` (after
  `axum::serve` returns). No-op when the env var is unset.
- **Metrics**: `axum-prometheus` at `/metrics`, scraped via the platform's `PodMonitor` pattern.
- **Health checks**: `/healthz` and `/readyz` per PADR-0017.

## Committing Code

[Conventional Commits](https://www.conventionalcommits.org/): `<type>(<scope>): <description>`.
While the platform is under construction, do not mark any commit as BREAKING; versions stay
`0.x`.

## Branches and Workflow

Git-flow (see `docs/git-flow.md` in `sweetrpg/platform`): `develop` is the integration branch,
`master` reflects the latest release. Feature/fix branches off `develop`, PR back into
`develop`.

Releasing: dispatch the "Prepare Release" workflow - it computes the next version via
`git-cliff`, bumps `Cargo.toml`/`Cargo.lock`, updates `CHANGELOG.md`, and opens a
`release/<version>` PR into `master` (via `sweetrpg/github-actions`'s reusable
`rust-prepare-release.yaml`/`rust-release.yaml`/`rust-tag-release.yaml` workflows). Merging that
PR tags the release; `docker-build.yml` then builds and pushes
`ghcr.io/sweetrpg/game-systems-web` (multi-arch, amd64 + arm64) and bumps the deployed image
tag via `argocd-update-deployment.yaml`. No crates.io publish - this ships only as a container
image.

## Running Checks Locally

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test
cargo run
```

`cargo run` serves on `:8080`. Set `SHARED_URL` to a reachable `shared-web` instance and
`GAME_SYSTEMS_API_URL` to a reachable `game-systems-api` to render real data locally.

## Platform Conventions and Decisions

This repo is a submodule of `sweetrpg/platform`. Platform-wide conventions and Architecture
Decision Records live there:

- checked out inside the platform tree: `../../docs/README.md` (convention index) and
  `../../docs/adr/README.md` (platform ADRs, `PADR-*`)
- standalone or module-only checkout:
  <https://github.com/sweetrpg/platform/tree/master/docs> and
  <https://github.com/sweetrpg/platform/tree/master/docs/adr>

Accepted `PADR-*` records are binding constraints. Read the ADR index before proposing a
structural change; if a task needs to contradict an accepted record, stop and say so - propose
a superseding ADR rather than working around it. This repo's own service-local decisions are
`ADR-*` in `docs/adr/` here. `/adr "<title>"` scaffolds one.
