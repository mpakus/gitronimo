# Agent Rules

- Read `PLAN.md` and this file before editing code.
- Before non-trivial implementation, use XERJ reference coding: search `gitronimo-*` indices on `http://127.0.0.1:9200` (see `.cursor/rules/xerj-reference-coding.mdc`); GitComet is AGPL — approach-only.
- Work on one unchecked `PLAN.md` checkbox group at a time.
- Record the intended files and acceptance checks in `docs/work-log.md` before coding.
- Never build Git commands with shell strings; use typed `std::process::Command` arguments.
- Never place Git or domain logic in GPUI render implementations.
- Never import GPUI in `git_domain`.
- Do not add `gpui-component` without a superseding ADR; use project-owned GPUI primitives in `ui_kit`.
- Pin framework versions exactly and commit `Cargo.lock`.
- Add tests for every Git parser and mutation; use temporary repositories for integration tests.
- Do not log credentials, environment dumps, or unredacted command output.
- Do not add `unsafe` without an ADR.
- Do not copy Tower or third-party icons, glyphs, or design assets into shipped product code; keep Gitronimo's branding original.
- Screenshots of third-party products (including Tower) may be saved under `docs/` for UI/UX reference and study, provided each is attributed to its source; never ship them in the app bundle or claim them as Gitronimo's own.
- Do not add a dependency, crate, or framework abstraction without a current checklist item that needs it.
- Before completing a task, run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check`.

## Toolchain

Use **Rust 1.97+** (`edition2024`). On macOS:

```bash
export PATH="$HOME/.rustup/toolchains/1.97.1-aarch64-apple-darwin/bin:$PATH"
```

System `cargo` older than 1.97 will fail on this workspace.

## Documentation map

| Doc | Purpose |
|-----|---------|
| `PLAN.md` | Product roadmap and checklist — source of truth for scope |
| `docs/README.md` | Documentation index |
| `docs/work-log.md` | Per-task intent, files, acceptance checks (write **before** coding) |
| `docs/desktop-shell.md` | Activity bar, message history, confirms, pins, command palette, About |
| `docs/UI-PLAN.md` | Tower parity phases and screenshot regression matrix |
| `docs/UI-IMPROVE.md` | Tower guide patterns mapped to GitRonimo views |
| `docs/architecture.md` | Crate layers and mutation flow |
| `docs/implementation-boundaries.md` | Layering constraints |
| `docs/troubleshooting.md` | User-facing recovery and keyboard reference |
| `docs/keyboard-shortcuts.md` | Global shortcuts and Working Copy selection rules |
| `docs/screens/README.md` | Screenshot inventory and attribution rules |
| `docs/dependency-policy.md` | `cargo deny` policy |
| `docs/packaging.md` | macOS Apple Silicon / Intel bundles, signing |
| `docs/adr/` | Architecture decision records |

Tower reference screenshots live under `docs/screens/` (attributed, internal study only).

## GPUI agent skills

Project-owned GPUI skills are vendored under `./skills/` (see `skills/README.md`). Symlink or copy them into agent skill directories when working on desktop UI:

```bash
for skill in skills/gpui-*; do
  ln -sf "../../$skill" ".cursor/skills/$(basename "$skill")"
done
```

Prefer these skills for actions, entities, styling, async, and testing patterns before improvising GPUI code.

## Working Copy selection (agent context)

When touching file-list selection or staging:

- **`SelectAllStatusFiles`** (`Cmd/Ctrl+A`) selects all paths from `visible_status_paths()` (Modified/All Files tab + search filter).
- When all visible files are selected, a plain row click clears the selection; clicking that same row again re-selects all (`file_list_select_all_toggle`). Clicking any other row selects that file alone.
- **Shift+click** and **Cmd/Ctrl+click** extend selection; checkbox clicks on a multi-selected row stage/unstage **all selected paths** and preserve list selection (`run_mutation(..., preserve_selection: true)`).
- Commit subject/body fields keep their own `SingleLineInput` `Cmd/Ctrl+A` binding when focused.

See `docs/keyboard-shortcuts.md` for the full shortcut list.

## Desktop shell (agent context)

Read `docs/desktop-shell.md` before changing activity bar, overlays, pins, or the palette.

- **Status text:** always use `set_activity(...)` (never assign `self.activity` alone) so Message history stays populated. Refresh chatter is coalesced; do not log secrets or raw Git dumps.
- **Confirms:** blocked/destructive Git outcomes that users must acknowledge belong in `AppConfirmDialog` (or the branch-delete pending modal), not only a flashing status line. Example: unmerged delete → Cancel / Delete force.
- **Pins / archives:** persist via `RecentRepositoryStore::save_branch_organization`; sidebar shows pins flat atop BRANCHES (no “PINNED” label). Preference RMW is path-locked — do not reintroduce unlocked load-modify-save on that JSON.
- **Command palette:** new user-facing commands that already have handlers should get a `PaletteCommand` + `PALETTE_COMMANDS` label + `run_palette_command` arm; keep the overlay list scrollable.
- **Overlays:** Git/domain work stays in `main.rs`; `views/workspace.rs` only renders and dispatches. About GitRonimo is `views/about.rs` (click outside to dismiss).
- **Product version:** About shows `APP_VERSION` in `apps/desktop/src/views/about.rs`. Bump that string after each release. It is independent of the Cargo workspace version.
- **Binary / menu name:** crate remains `gitronimo-desktop`; the macOS executable and bundle name is `GitRonimo` so the application menu title is GitRonimo (`GitRonimo.app`).
