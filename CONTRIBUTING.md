# Contributing

Thanks for considering a contribution to `game-systems-web`.

## Branching

This repo follows the sweetrpg platform's git-flow convention:

* `develop` is the integration branch. All feature and fix branches merge here.
* `master` reflects the latest released state. Nothing is committed here directly.
* Branch names: `feature/<description>` for new functionality, `fix/<description>` for bug
  fixes, `hotfix/<description>` for urgent fixes to a released version.

```bash
git checkout develop
git pull
git checkout -b feature/my-change
# ... work, commit ...
git push -u origin feature/my-change
# open a PR: feature/my-change -> develop
```

## Commit messages

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

## Running checks locally

```bash
cargo fmt --check
cargo clippy --all-targets
cargo test
cargo run
```

`cargo run` serves on `:8080`. Set `SHARED_URL` to a reachable `shared-web` instance
(e.g. `https://dev.sweetrpg.com/shared`) to see the real logo/branding rendered locally instead
of broken image references.

## Pull requests

CI runs automatically on PRs targeting `develop`. Once checks pass, it can be merged (auto-merge
is enabled once required checks pass).
