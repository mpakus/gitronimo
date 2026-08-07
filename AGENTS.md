# Agent Rules

- Read `PLAN.md` and this file before editing code.
- Work on one unchecked `PLAN.md` checkbox group at a time.
- Record the intended files and acceptance checks in `docs/work-log.md` before coding.
- Never build Git commands with shell strings; use typed `std::process::Command` arguments.
- Never place Git or domain logic in GPUI render implementations.
- Never import GPUI in `git_domain`.
- Keep `gpui-component` usage inside `ui_kit`.
- Pin framework versions exactly and commit `Cargo.lock`.
- Add tests for every Git parser and mutation; use temporary repositories for integration tests.
- Do not log credentials, environment dumps, or unredacted command output.
- Do not add `unsafe` without an ADR.
- Do not copy Tower assets, copy, icons, or proprietary design details.
- Do not add a dependency, crate, or framework abstraction without a current checklist item that needs it.
- Before completing a task, run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check`.

