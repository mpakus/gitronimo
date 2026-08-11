# Architecture overview

Gitronimo uses a deliberately small layered workspace:

- `apps/desktop`: GPUI application state, native menus, dialogs, background tasks, and repository watching.
- `crates/app_core`: repository-opening and preference-store use cases.
- `crates/git_domain`: UI-independent Git data models.
- `crates/git_cli`: typed, shell-free adapter for the installed Git executable.
- `crates/ui_kit`: project-owned colors and GPUI presentation primitives.

Repository mutations run through `git_cli`; GPUI rendering does not invoke Git or filesystem work. The desktop app runs loading, status, history, and network work off the UI thread and refreshes state after mutations.

Working Copy file selection (`selected_paths`) is UI state in `GitronimoApp`; staging mutations can preserve that selection when triggered from row checkboxes (`run_mutation` with `preserve_selection: true`). Toolbar stage/unstage actions still clear selection after success.

See [implementation boundaries](implementation-boundaries.md) for the governing constraints and [keyboard shortcuts](keyboard-shortcuts.md) for selection behavior.
