# Dependency policy

- Framework versions are exact pins and change only in dedicated pull requests.
- `Cargo.lock` is committed and updated with every dependency change.
- Dependabot opens monthly Cargo and GitHub Actions updates; review them individually.
- Run `cargo deny check` in CI and before merging a dependency update.
- Do not add a dependency until an active `PLAN.md` checklist item needs it.
- The initial GPUI dependency graph emits future-incompatibility warnings for transitive `block` and `proc-macro-error2`. Do not suppress them; reassess during the next GPUI upgrade.

