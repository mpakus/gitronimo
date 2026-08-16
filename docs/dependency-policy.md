# Dependency policy

- Framework versions are exact pins and change only in dedicated pull requests.
- `Cargo.lock` is committed and updated with every dependency change.
- Dependabot opens monthly Cargo and GitHub Actions updates; review them individually.
- Run `cargo deny check` in CI and before merging a dependency update.
- The reviewed GPUI graph also uses `Apache-2.0 WITH LLVM-exception`, CC0-1.0, MPL-2.0, and NCSA; each is explicitly allowed in `deny.toml` rather than accepted implicitly.
- Internal path dependencies carry their workspace version so the wildcard ban remains meaningful for third-party dependencies.
- Do not add a dependency until an active `PLAN.md` or `docs/PLAN-v2.md` checklist item needs it. Optional AI commit messages use typed `curl` and existing `serde_json`; do not add an HTTP/AI crate for that path.
- The initial GPUI dependency graph emits future-incompatibility warnings for transitive `block` and `proc-macro-error2`. Do not suppress them; reassess during the next GPUI upgrade.
