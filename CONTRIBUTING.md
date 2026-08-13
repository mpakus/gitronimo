# Contributing to Gitronimo

Read [PLAN.md](PLAN.md) and [AGENTS.md](AGENTS.md) before coding. Keep each change tied to one checklist group and record its intended files and acceptance checks in [docs/work-log.md](docs/work-log.md).

Documentation lives under [docs/](docs/README.md). Update [docs/keyboard-shortcuts.md](docs/keyboard-shortcuts.md) when shortcuts or Working Copy selection behavior changes, [docs/desktop-shell.md](docs/desktop-shell.md) when activity bar / palette / confirms / pins / About change, bump `APP_VERSION` in `apps/desktop/src/views/about.rs` when cutting a product release, and refresh [README.md](README.md) screenshots when the UI changes materially.

## Development rules

- Use typed `std::process::Command` arguments for Git; never build shell command strings.
- Keep Git and domain logic outside GPUI rendering.
- Add temporary-repository tests for every Git parser or mutation.
- Do not add dependencies, unsafe code, credentials, real repositories, or copied third-party product assets without the required project justification.

## Verification

Install Rust **1.97+** (`edition2024`), then run the full gate before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
```

Describe the user-visible behavior, verification run, and any remaining manual macOS checks. Never attach repository contents, remotes containing credentials, or unredacted Git output to an issue or pull request.

## Conduct and security

Participation follows the [Code of Conduct](CODE_OF_CONDUCT.md). Report vulnerabilities through the process in [SECURITY.md](SECURITY.md), not a public issue.
