# Architecture overview

GitRonimo uses a deliberately small layered workspace:

- `apps/desktop`: GPUI application state, native menus, dialogs, background tasks, and repository watching. The shipped binary is **GitRonimo**.
- `crates/app_core`: repository-opening and preference-store use cases, plus Git engine ports (`GitRefQuery`, `GitHistoryQuery`, `GitObjectQuery`, `GitIndexMutate`, `GitNetwork`).
- `crates/git_domain`: UI-independent Git data models (`CommitRequest`, `LoadedDiff`, status/history types).
- `crates/git_gix`: gitoxide `gix` adapter (default engine for discovery, HEAD, refs, status, history, trees/diffs, stage/commit, HTTPS fetch/clone).
- `crates/git_cli`: typed, shell-free adapter for the installed Git executable (fallback and unmigrated operations).
- `crates/ui_kit`: project-owned colors and GPUI presentation primitives.
- `crates/platform_macos`: Keychain `SecretStore` for the optional GitHub personal access token.
- `crates/hosting_github`: GitHub API adapter used by Settings / Pull Requests.

Repository discovery, HEAD, refs, working-copy status, history, tree/blob reads, unified diffs, stage/unstage/commit, and HTTPS fetch/clone use `gix` unless Settings forces system Git or `gix` fails (then `git_cli`). Checkout, merge, rebase, stash mutations, discard, hunk staging, push, hooks, signed commits, and SSH/`file://` remotes still run through `git_cli`. GPUI rendering does not invoke Git or filesystem work. The desktop app runs loading, status, history, and network work off the UI thread and refreshes state after mutations.

Working Copy file selection (`selected_paths`) is UI state in `GitronimoApp`; staging mutations can preserve that selection when triggered from row checkboxes (`run_mutation` with `preserve_selection: true`). Toolbar stage/unstage actions still clear selection after success.

## Preference store

`RecentRepositoryStore` (`app_core`) owns `~/Library/Application Support/Gitronimo/recent-repositories.json`: recents, window geometry, sidebar/list widths, expanded ref groups, bookmark folders, per-repository `branch_organization` (pinned / archived branch names), and per-repository workflow config.

Every load-modify-save path takes a **path-keyed mutex** so concurrent writers (geometry vs pins, multiple `RecentRepositoryStore::new` handles) cannot drop each other's fields. Callers must keep using the typed `save_*` / `load_*` APIs rather than hand-editing the JSON document without that lock. The `use_system_git`, `auto_stash`, and `in_app_updates` flags (all default off) are stored in the same document.

## Desktop shell state

Shell chrome (activity history, `AppConfirmDialog`, command palette, pin presentation) is documented in [desktop-shell.md](desktop-shell.md). Status lines append through `set_activity`; overlays dispatch into `main.rs` handlers and must not run Git inside `Render`.

See [implementation boundaries](implementation-boundaries.md) for the governing constraints and [keyboard shortcuts](keyboard-shortcuts.md) for selection behavior.
