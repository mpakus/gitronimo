# Gitronimo

Gitronimo is a native macOS Git client written in Rust with GPUI. It is in Phase 0 technical validation and is not ready for daily use.

## Development

Install Rust 1.97.1 and run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
```

The build requires macOS once the GPUI window spike begins. An installed Git executable is an MVP requirement.

See [macOS packaging](docs/packaging.md) for the unsigned development bundle and release-signing handoff.

## Scope

The MVP focuses on opening a local repository, reviewing status and diffs, staging, committing, browsing history, and normal branch/remote workflows. See [PLAN.md](PLAN.md) for the implementation contract and non-goals.

Gitronimo is not affiliated with Tower. It does not include Tower code, assets, or branding.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
