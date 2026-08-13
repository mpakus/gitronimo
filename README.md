# GitRonimo

GitRonimo is a native macOS Git client written in Rust with [GPUI](https://gpui.rs). It keeps Git as the source of truth and uses your installed Git executable for repository operations, credential helpers, SSH, hooks, signing, and filters.

Product version **0.9** (About GitRonimo). The Cargo workspace version stays independent; bump the string users see in [`apps/desktop/src/views/about.rs`](apps/desktop/src/views/about.rs) (`APP_VERSION`) after each release.

![GitRonimo working copy with a staged diff](docs/screenshot.png)

## 0.9 scope

The current release includes:

- **Welcome / Repositories** — open, add, create, and clone local repositories; grouped bookmark folders
- **Working Copy** — status and diffs, Modified/All Files toggle, multi-select (`Command-A`, Shift-click), batch stage/unstage via checkboxes, partial line/hunk staging, commit/amend/sign-off
- **History** — bounded commit log with graph, scope filter, Changeset/Tree detail, double-click to Commit Detail, commit context menu
- **Stashes, Remotes, Pull Requests, Settings** — two-pane list + detail layouts (some secondary views remain palette-only)
- **Workflow** — GitHub Flow / GitLab Flow / git-flow templates, auto-detect, Start / Finish / Sync topic branches (welcome + in-repo)
- **Branches** — context menu (pin/archive/rename/delete…); pinned branches persist and sit at the top of BRANCHES; unmerged delete offers a force-confirm dialog
- **Network** — fetch, pull, publish, and push in the background with cancellation; progress in the activity bar; Pull/Push dialogs for options
- **Command palette / Message history** — searchable scrollable palette (`Command-Shift-P`); activity-bar history of statuses, errors, and confirmations
- **About GitRonimo** — application menu **GitRonimo → About GitRonimo** (icon, version **0.9**, “Made in Austin ✩ Texas”, [aomega.co](https://aomega.co))
- **Safety** — typed Git invocation, force-with-lease confirmation, local crash reports, recovery from missing repos and stale index locks

GitHub personal-access-token connect/sign-out lives in **Settings** (not a Services tab).

Not yet included or still partial: signed/notarized distribution, OAuth/enterprise GitHub, deterministic byte-level progress parsing, full VoiceOver parity (GPUI limitation), merge/rebase wizards, and built-in editing.

See [PLAN.md](PLAN.md) for the full implementation contract and [CHANGELOG.md](CHANGELOG.md) for release notes.

## Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| Command-O | Open repository |
| Command-R | Refresh working copy |
| Command-A | Select all visible files (Working Copy) |
| Command-Shift-C | Focus commit subject |
| Command-Shift-S | Save stash |
| Command-Shift-P | Command palette |
| Command-/ | Shortcut reference overlay |
| Command-[ / Command-] | Back / Forward |
| Command-Q | Quit GitRonimo |
| Command-H | Hide GitRonimo |
| Command-F | Focus toolbar search |

Working Copy selection and batch checkbox staging: [`docs/keyboard-shortcuts.md`](docs/keyboard-shortcuts.md).

## Install and run

Unsigned Apple Silicon and Intel `.app` bundles can be built locally (see [macOS packaging](docs/packaging.md)). On macOS:

```bash
rustup toolchain install 1.97.1
rustup target add aarch64-apple-darwin x86_64-apple-darwin
export PATH="$HOME/.rustup/toolchains/1.97.1-aarch64-apple-darwin/bin:$PATH"

cargo install cargo-packager --version 0.11.8 --locked
cargo build --release -p gitronimo-desktop
cargo packager --release --formats app --manifest-path apps/desktop/Cargo.toml --out-dir "$(pwd)/target/release-arm" --binaries-dir "$(pwd)/target/release"
open target/release-arm/GitRonimo.app
```

The menu bar title is **GitRonimo** (binary / bundle name). Gatekeeper will warn on unsigned builds; use the documented signing handoff for a distributable release. An installed Git executable is required.

## Build and verify

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
```

## Documentation

| Doc | Description |
|-----|-------------|
| [docs/README.md](docs/README.md) | Documentation index |
| [Architecture](docs/architecture.md) | Crate layers and mutation flow |
| [Keyboard shortcuts](docs/keyboard-shortcuts.md) | Global and Working Copy shortcuts |
| [Desktop shell](docs/desktop-shell.md) | Activity bar, palette, confirms, pins, About |
| [UI plan](docs/UI-PLAN.md) | Tower parity phases |
| [UI improve](docs/UI-IMPROVE.md) | Tower guide → GitRonimo mapping |
| [Work log](docs/work-log.md) | Implementation notes |
| [Troubleshooting](docs/troubleshooting.md) | Recovery and toolchain |
| [Packaging](docs/packaging.md) | Apple Silicon / Intel bundles, signing |
| [Contributing](CONTRIBUTING.md) | Development rules |
| [AGENTS.md](AGENTS.md) | Agent/coding constraints |

## Architecture and support

- [Implementation boundaries](docs/implementation-boundaries.md)
- [Dependency policy](docs/dependency-policy.md)
- [Security policy](SECURITY.md)
- [Third-party notices](docs/third-party-notices.md)
- [Trademark statement](TRADEMARKS.md)

GitRonimo is not affiliated with Tower or any Git hosting provider. It does not include Tower code, assets, copy, or branding.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
