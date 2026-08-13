# Initial workspace boundaries

The current crates are the boundaries required for the desktop app:

- `git_domain`: pure types (no GPUI);
- `app_core`: application use cases and ports;
- `git_cli`: installed-Git adapter (typed `Command` arguments only);
- `ui_kit`: project-owned GPUI primitives and theme (no `gpui-component`; see ADR 0001);
- `platform_macos`: Keychain-backed secret store;
- `hosting_github`: GitHub HTTP/JSON adapter (no UI, no Git);
- `gitronimo-desktop`: macOS composition root (binary **GitRonimo**);
- `test_support`: deterministic Git fixtures.

Do not import GPUI in `git_domain`. Do not run Git or filesystem work inside GPUI `Render` implementations. GitHub tokens stay in Keychain via `platform_macos`; they are never written to preferences, activity, or crash reports.

