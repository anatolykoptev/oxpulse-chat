# Contributing

## Commits

This repo uses [Conventional Commits](https://www.conventionalcommits.org/).
`release-please` watches `main` and opens per-component Release PRs that bump
versions, update CHANGELOG.md, and tag on merge.

Commit scopes that drive turn-node releases:

- `feat(turn-node): ...` → MINOR bump (new env var, new distro supported)
- `fix(turn-node): ...` → PATCH bump (template hardening, bug fix)
- `feat(turn-node)!: ...` or a `BREAKING CHANGE:` footer → MAJOR bump (env-var rename, template-incompatible change)

Scopes outside `turn-node` do not cut a turn-node release.
