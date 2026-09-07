# Game systems web

[![CI](https://github.com/sweetrpg/game-systems-web/actions/workflows/ci.yaml/badge.svg)](https://github.com/sweetrpg/game-systems-web/actions/workflows/ci.yaml)
[![License](https://img.shields.io/github/license/sweetrpg/game-systems-web.svg)](https://img.shields.io/github/license/sweetrpg/game-systems-web.svg)
[![Issues](https://img.shields.io/github/issues/sweetrpg/game-systems-web.svg)](https://img.shields.io/github/issues/sweetrpg/game-systems-web.svg)
[![PRs](https://img.shields.io/github/issues-pr/sweetrpg/game-systems-web.svg)](https://img.shields.io/github/issues-pr/sweetrpg/game-systems-web.svg)

[![Rust](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)](https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white)

Server-rendered frontend for the SweetRPG game systems catalog, serving
`dev.sweetrpg.com/game-systems` (and `sweetrpg.com/game-systems` in production): a game system
detail page, a browse-and-search page over the live catalog, and a role-gated form to propose a
new system. It is the read/write face of `game-systems-api`, mirroring `catalog-web`'s role for
the volume catalog.

Built with [Axum](https://github.com/tokio-rs/axum) and
[Askama](https://github.com/askama-rs/askama). This is the platform's second Rust frontend;
`main-web` is the language reference - see `sweetrpg/platform`'s
`docs/rust-service-conventions.md` and this repo's `AGENTS.md` for conventions.

## Running locally

```bash
cargo run
```

Serves on `:8080`. See `CONTRIBUTING.md` for environment variables and the full local
development workflow.
