# Architecture overview

Gitronimo uses a deliberately small layered workspace:

- `apps/desktop`: GPUI application state, native menus, dialogs, background tasks, and repository watching.
- `crates/app_core`: repository-opening and preference-store use cases.
- `crates/git_domain`: UI-independent Git data models.
- `crates/git_cli`: typed, shell-free adapter for the installed Git executable.
- `crates/ui_kit`: project-owned colors and GPUI presentation primitives.

Repository mutations run through `git_cli`; GPUI rendering does not invoke Git or filesystem work. The desktop app runs loading, status, history, and network work off the UI thread and refreshes state after mutations. See [implementation boundaries](implementation-boundaries.md) for the governing constraints.
