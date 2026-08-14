# GitRonimo documentation

Index of project documentation. Start with [README.md](../README.md) for install and overview. Product version **0.9.2** is `APP_VERSION` in `apps/desktop/src/views/about.rs` (bump after each release).

## Product and planning

| Document | Audience | Purpose |
|----------|----------|---------|
| [PLAN.md](../PLAN.md) | Contributors | Roadmap, checklist, crate boundaries |
| [UI-PLAN.md](UI-PLAN.md) | UI contributors | Tower parity phases and screenshot matrix |
| [UI-IMPROVE.md](UI-IMPROVE.md) | UI contributors | Tower guide patterns mapped to GitRonimo views |
| [work-log.md](work-log.md) | Contributors | Per-task intent, files, acceptance (write before coding) |
| [../CHANGELOG.md](../CHANGELOG.md) | Everyone | Release notes |

## Architecture and policy

| Document | Purpose |
|----------|---------|
| [architecture.md](architecture.md) | Crate layers, async mutations, selection state |
| [implementation-boundaries.md](implementation-boundaries.md) | What belongs in each crate |
| [dependency-policy.md](dependency-policy.md) | Pins, `cargo deny`, Dependabot |
| [adr/](adr/) | Architecture decision records |
| [packaging.md](packaging.md) | Apple Silicon / Intel `.app`, signing, notarization, CI release |

## Using GitRonimo

| Document | Purpose |
|----------|---------|
| [desktop-shell.md](desktop-shell.md) | Activity bar, message history, confirms, pins, command palette, About |
| [keyboard-shortcuts.md](keyboard-shortcuts.md) | Global shortcuts and Working Copy multi-select |
| [troubleshooting.md](troubleshooting.md) | Repo open failures, index locks, auth, toolchain, Gatekeeper |
| [third-party-notices.md](third-party-notices.md) | License attributions |

## Screenshots

| Location | Purpose |
|----------|---------|
| [screens/README.md](screens/README.md) | GitRonimo captures vs Tower reference shots |
| `screens/gitronimo-*.png` | Committed product screenshots (README, docs) |
| `screens/tower-guides/` | Tower help guide reference (local study, gitignored) |
| `screens/tower-*.png` | Tower comparison captures (local study, gitignored) |

Tower screenshots are © [Tower](https://www.git-tower.com/) / fournova and are never shipped in the app bundle.

## Agent and contributor setup

| Document | Purpose |
|----------|---------|
| [AGENTS.md](../AGENTS.md) | Agent rules, toolchain, XERJ reference coding, `APP_VERSION` |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | PR gates and conduct |
| [../skills/README.md](../skills/README.md) | GPUI agent skills vendored in-repo |

## Maintenance

When you change user-visible behavior:

1. Add or update an entry in [work-log.md](work-log.md) **before** coding.
2. Update [keyboard-shortcuts.md](keyboard-shortcuts.md) if shortcuts or selection rules change.
3. Update [desktop-shell.md](desktop-shell.md) if activity bar, palette, confirms, branch pin/archive, or About behavior changes.
4. Update [README.md](../README.md) screenshots if the UI changed materially.
5. Sync [UI-IMPROVE.md](UI-IMPROVE.md) implementation status when a Tower parity item lands.
6. Add a [CHANGELOG.md](../CHANGELOG.md) note when preparing a release.
7. Bump `APP_VERSION` in `apps/desktop/src/views/about.rs` after each product release (currently **0.9.2**).
