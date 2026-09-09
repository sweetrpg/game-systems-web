---
id: ADR-0001
title: Publisher lookup via catalog-api
status: accepted
date: 2026-09-08
scope: game-systems-web
supersedes: []
superseded-by: []
tags: [frontend, cross-service]
affected_repos: [game-systems-web]
---

# ADR-0001: Publisher lookup via catalog-api

## Context

The add-new-system form (`GET/POST /game-systems/new`) had a free-text `publisher_id` field.
Reviewers asked for a name lookup instead of a raw id.

`game-systems-api` stores `publisher_id` as an opaque string and has no publishers resource.
Publisher records live in `catalog-api` (`catalog-objects` `Publisher`/`PublisherVersion`),
which already exposes `GET /publishers/search?q=` - documented in its handler as backing
"autocomplete/picker inputs". Until now `game-systems-web` talked only to `game-systems-api`,
the shared session Redis, and `shared-web`.

`docs/deployment-conventions.md` (platform) sanctions cross-namespace calls when they are
"deliberate and narrow"; every other frontend (`catalog-web`, `admin-web`, `assets-web`,
`game-room-web`) already reaches `catalog-api` at
`http://api-v1.sweetrpg-catalog.svc.cluster.local:8000` via `CATALOG_API_URL`.

## Options

- **Option A - new `/publishers` endpoint on `game-systems-api`**: `game-systems-web` stays
  single-upstream; `game-systems-api` grows a publishers route proxying `catalog-api` or its
  own store. Rejected: backend work in a separate repo for no gain over calling `catalog-api`
  directly, and it would still just forward `catalog-api` data.
- **Option B - datalist from `publisher_id`s already on systems**: derive suggestions from
  values `game-systems-api` already returns, no new dependency. Rejected: those are ids with no
  names - it can't show a name picker, which is the actual request.
- **Option C - call `catalog-api` `GET /publishers/search` read-only (chosen)**: a new
  fail-open client in `game-systems-web`. See Decision.

## Decision

`game-systems-web` calls `catalog-api` `GET /publishers/search?q=` read-only to back the
publisher name picker on the add-new form. A new `CatalogClient` (`src/catalog_client.rs`,
2s timeout) is fail-open: an unset `CATALOG_API_URL`, a timeout, a non-2xx, or an unparseable
body all yield an empty result rather than surfacing a failure. The browser hits a same-origin
JSON route on this app (`GET /game-systems/new/publishers?q=`, same session + write-role gate as
the form); that route calls `catalog-api` server-side. The picker's JS sets a hidden
`publisher_id` from the chosen suggestion. With JS disabled, the typed name is resolved to an id
server-side on submit: exactly one case-insensitive name match wins; zero matches (when the
picker is enabled) or more than one re-renders the form with an error; a disabled picker accepts
the name with a blank id.

## Consequences

`game-systems-web` now has a fourth outbound dependency and a cross-namespace path into
`sweetrpg-catalog`. It is read-only and fail-open, so a `catalog-api` outage degrades the
publisher field to plain text rather than breaking the form. A transient `catalog-api` failure
during a no-JS submit is indistinguishable from "no such publisher" and shows the not-found
error; re-submitting recovers. Reversing this means dropping the picker and the
`/new/publishers` route and going back to a free-text id field. The detail page
(`templates/detail.html`) still renders `publisher_id` raw - resolving it to a name there is a
possible follow-up that would reuse this client.
