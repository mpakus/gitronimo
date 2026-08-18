# Initial workspace boundaries

The current crates are the boundaries required for the desktop app:

- `git_domain`: pure types (no GPUI);
- `app_core`: application use cases and ports (`RepositoryDiscoverer`, `GitRefQuery`, `GitHistoryQuery`, `GitObjectQuery`, `GitIndexMutate`, `GitNetwork`), plus AI commit prompt/JSON helpers (no network I/O);
- `git_gix`: gitoxide `gix` adapter (ADR 0003; no GPUI);
- `git_cli`: installed-Git adapter (typed `Command` arguments only);
- `ui_kit`: project-owned GPUI primitives and theme (no `gpui-component`; see ADR 0001);
- `platform_macos`: Keychain-backed secret store and AppKit file-URL drag-out (ADR 0002);
- `hosting_github`: GitHub HTTP/JSON adapter (no UI, no Git);
- `gitronimo-desktop`: macOS composition root (binary **GitRonimo**);
- `test_support`: deterministic Git fixtures.

Do not import GPUI in `git_domain`. Do not import `gix` outside `git_gix`. Desktop talks to Git through `apps/desktop/src/git_backend.rs` for migrated ports; unmigrated operations still use `git_cli` with typed `Command` arguments. Do not run Git or filesystem work inside GPUI `Render` implementations. Overlay fade and composer expand live in `apps/desktop/src/views/overlay_anim.rs` (no new crate); dismiss and Git stay in `main.rs`. GitHub tokens and AI API keys stay in Keychain via `platform_macos`; they are never written to preferences, activity, or crash reports. Do not add an HTTP/AI crate for commit suggestions: typed `curl` in `apps/desktop/src/ai_commit.rs` plus existing `serde_json`.

