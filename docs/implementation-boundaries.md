# Initial workspace boundaries

The current crates are the boundaries required for the Phase 0 spikes and the first vertical slice:

- `git_domain`: pure types;
- `app_core`: application use cases and ports;
- `git_cli`: installed-Git adapter;
- `ui_kit`: sole GPUI and `gpui-component` boundary;
- `gitronimo-desktop`: macOS composition root;
- `test_support`: deterministic Git fixtures.

The planned `git_diff`, `git_graph`, `repo_watch`, `persistence`, and `platform_macos` crates are intentionally deferred until their corresponding checklist items require a stable boundary. This prevents unused placeholder crates while preserving the dependency direction in `PLAN.md`.

