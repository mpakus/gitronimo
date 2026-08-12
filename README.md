# Gitronimo

Gitronimo is a native macOS Git client written in Rust with [GPUI](https://gpui.rs). It keeps Git as the source of truth and uses your installed Git executable for repository operations, credential helpers, SSH, hooks, signing, and filters.

![Gitronimo working copy with a staged diff](docs/screenshot.png)

## Beta scope

The current beta includes:

- **Welcome / Repositories** — open, add, create, and clone local repositories; grouped bookmarks; Services rail for GitHub token auth and hosted clone handoff
- **Working Copy** — status and diffs, Modified/All Files toggle, multi-select (`Command-A`, Shift-click), batch stage/unstage via checkboxes, partial line/hunk staging, commit/amend/sign-off
- **History** — bounded commit log with graph, scope filter, Changeset/Tree detail, double-click to Commit Detail
- **Stashes, Remotes, Pull Requests, Services, Settings** — two-pane list + detail layouts (some secondary views remain palette-only)
- **Network** — fetch, pull, publish, and push in the background with cancellation where Git allows
- **Safety** — typed Git invocation, force-with-lease confirmation, local crash reports, recovery from missing repos and stale index locks

Not yet included or still partial: signed/notarized distribution, OAuth/enterprise GitHub, deterministic byte-level progress parsing, full VoiceOver parity (GPUI limitation), merge/rebase wizards, and built-in editing.

See [PLAN.md](PLAN.md) for the full implementation contract and [CHANGELOG.md](CHANGELOG.md) for release notes.

## Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| Command-O | Open repository |
| Command-R | Refresh working copy |
| Command-A | Select all visible files (Working Copy) |
| Command-Shift-C | Focus commit subject |
| Command-Shift-P | Command palette |
| Command-/ | Shortcut reference overlay |
| Command-[ / Command-] | Back / Forward |
| Command-Q | Quit Gitronimo |

Working Copy selection and batch checkbox staging: [`docs/keyboard-shortcuts.md`](docs/keyboard-shortcuts.md).

## Install and run

Gitronimo currently ships as an unsigned development bundle. On macOS:

```bash
rustup toolchain install 1.97.1-aarch64-apple-darwin
export PATH="$HOME/.rustup/toolchains/1.97.1-aarch64-apple-darwin/bin:$PATH"

cargo install cargo-packager --version 0.11.8 --locked
cargo packager --release --formats app --manifest-path apps/desktop/Cargo.toml --out-dir target/release
open target/release/Gitronimo.app
```

An installed Git executable is required. See [macOS packaging](docs/packaging.md) for signing and notarization handoff.

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
| [UI plan](docs/UI-PLAN.md) | Tower parity phases |
| [UI improve](docs/UI-IMPROVE.md) | Tower guide → Gitronimo mapping |
| [Work log](docs/work-log.md) | Implementation notes |
| [Troubleshooting](docs/troubleshooting.md) | Recovery and toolchain |
| [Contributing](CONTRIBUTING.md) | Development rules |
| [AGENTS.md](AGENTS.md) | Agent/coding constraints |

## Architecture and support

- [Implementation boundaries](docs/implementation-boundaries.md)
- [Dependency policy](docs/dependency-policy.md)
- [Security policy](SECURITY.md)
- [Third-party notices](docs/third-party-notices.md)
- [Trademark statement](TRADEMARKS.md)

Gitronimo is not affiliated with Tower or any Git hosting provider. It does not include Tower code, assets, copy, or branding.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
