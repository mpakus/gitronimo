# Agent Rules

- Read `PLAN.md` and this file before editing code.
- Before non-trivial implementation, use XERJ reference coding: search `gitronimo-*` indices on `http://127.0.0.1:9200` (see `.cursor/rules/xerj-reference-coding.mdc`); GitComet is AGPL — approach-only.
- Work on one unchecked `PLAN.md`, `docs/PLAN-v2.md`, or `docs/PLAN-v3.md` checkbox group at a time. 1.0 scope is tagged in `PLAN.md`. Post-1.0 A/E/D/G are in tree; remaining PLAN-v2 boxes are `gix` fallback migrations (checkout, merge, rebase, stash, push, hooks). Post-2.0 F/H/P (findability, GitHub OAuth/enterprise, polish) are in `docs/PLAN-v3.md` — do not mix them into `gix` fallback work.
- Record the intended files and acceptance checks in `docs/work-log.md` before coding.
- Never build Git commands with shell strings; use typed `std::process::Command` arguments.
- Never place Git or domain logic in GPUI render implementations.
- Never import GPUI in `git_domain`.
- Do not add `gpui-component` without a superseding ADR; use project-owned GPUI primitives in `ui_kit`.
- Pin framework versions exactly and commit `Cargo.lock`.
- Add tests for every Git parser and mutation; use temporary repositories for integration tests.
- Do not log credentials, environment dumps, or unredacted command output.
- Do not add `unsafe` without an ADR.
- Do not copy third-party icons, glyphs, or design assets into shipped product code; keep Gitronimo's branding original.
- Third-party product screenshots may be saved under `docs/` for internal study if attributed to their source; never ship them in the app bundle or claim them as Gitronimo's own.
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
| `PLAN.md` | Product roadmap and checklist — source of truth for 1.0 scope |
| `docs/PLAN-v2.md` | Post-1.0.0 roadmap. A (`gix` default), E (LFS/stash extras), D (updater), G (AI commits) are in tree; remaining boxes are system-Git fallbacks until `gix` gains the workflow |
| `docs/PLAN-v3.md` | Post-2.0.0 roadmap. F (findability), H (GitHub OAuth/enterprise), P (polish). GitLab is 3.1. Do not mix with PLAN-v2 `gix` fallbacks |
| `docs/README.md` | Documentation index |
| `docs/work-log.md` | Per-task intent, files, acceptance checks (write **before** coding) |
| `docs/desktop-shell.md` | Activity bar, message history, confirms, pins, command palette, About |
| `docs/UI-PLAN.md` | UI phases and screenshot regression matrix |
| `docs/UI-IMPROVE.md` | GitRonimo view patterns and remaining UI gaps |
| `docs/architecture.md` | Crate layers and mutation flow |
| `docs/implementation-boundaries.md` | Layering constraints |
| `docs/troubleshooting.md` | User-facing recovery and keyboard reference |
| `docs/keyboard-shortcuts.md` | Global shortcuts and Working Copy selection rules |
| `docs/todo-v1.md` | Leftover work toward 1.0.0 |
| `docs/dependency-policy.md` | `cargo deny` policy |
| `docs/packaging.md` | macOS Apple Silicon / Intel bundles, signing |
| `docs/adr/` | Architecture decision records |

Product screenshots live under `docs/` (README hero and working-copy capture).

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
- **Command palette:** new user-facing commands that already have handlers should get a `PaletteCommand` + `PALETTE_COMMANDS` label + `run_palette_command` arm; keep the overlay list scrollable. Current extras include **Suggest commit message**, **Check for updates**, **App settings**, **Fetch Git LFS objects**, **Pull Git LFS objects**, **Save stash snapshot…**, **Apply selected stash files**.
- **Overlays:** Git/domain work stays in `main.rs`; `views/workspace.rs` only renders and dispatches. About GitRonimo is `views/about.rs` (click outside to dismiss; **Check for updates** closes About then runs the same handler as Settings). App Settings is `views/settings.rs` `app_settings_overlay` (click outside to dismiss; **GitRonimo → Settings…** / Command-comma).
- **Product version:** About shows `APP_VERSION` in `apps/desktop/src/views/about.rs` (currently **2.0.4**). Bump that string and `[package.metadata.packager] version` together after each release. Independent of the Cargo workspace version.
- **Binary / menu name:** crate remains `gitronimo-desktop`; the macOS executable and bundle name is `GitRonimo` so the application menu title is GitRonimo (`GitRonimo.app`).

## Git engine (agent context)

Read ADR `docs/adr/0003-gix-default-git-engine.md` before changing discovery, status, history, diffs, stage/commit, or fetch/clone.

- Default engine is gitoxide **`gix`** via `apps/desktop/src/git_backend.rs`. Settings **Use system Git** forces `git_cli`. Automatic fallback when `gix` lacks the workflow or errors; log a redacted reason with `set_activity`.
- Do not import `gix` outside `crates/git_gix`. Do not import GPUI in `git_domain`. Desktop must not call `gix` or build Git command lines inside `Render`.
- Still on system Git: checkout/switch/restore/reset, merge/cherry-pick/revert, rebase, stash mutations, push, hooks, signed commits/tags, LFS smudge/fetch, mergetool, submodules, worktree add/remove, SSH/`file://` clone/fetch, hunk/line stage and discard.
- Dual-backend tests on temporary repositories for every migrated operation.

## AI commit messages (agent context)

Read Settings copy in `views/settings.rs` and `crates/app_core/src/ai_commit.rs` before changing Suggest.

- Opt-in (`ai_commit_messages`, default **off**). No network until the user turns it on and runs **Suggest** (composer) or palette **Suggest commit message**.
- Prompt is the **staged unified diff only** (`unified_diff_prompt_text`, cap `MAX_AI_COMMIT_DIFF_BYTES`, then `git_cli::redact_git_text`). Never send the GitHub PAT, the AI Keychain secret, unredacted command output, README/CLAUDE.md, or the full repo.
- Fill subject/body and expand the composer. **Never** call commit from Suggest. The user edits and clicks Commit.
- API key is Keychain `com.gitronimo.ai-commit` / account `default`, separate from `com.gitronimo.github`. HTTPS (including empty endpoint → OpenAI default) requires a key. HTTP is allowlisted only for `127.0.0.1`, `localhost`, and `[::1]`.
- No HTTP/AI crate: typed `curl` in `apps/desktop/src/ai_commit.rs`; prompt/JSON/parse in `app_core`. Failure leaves the composer unchanged and uses `set_activity` with a redacted sentence.

## In-app updates (agent context)

- Default **on** (`in_app_updates`). **No check on launch.** **GitRonimo → Check for Updates…**, About **Check for updates**, GitRonimo **Settings…** **Check now**, and palette **Check for updates** only when the toggle is on. Turn the toggle off in **GitRonimo → Settings…** (Command-comma), not repository Settings.
- Public GitHub Releases JSON (no PAT). Verify SHA-256, then `codesign --verify --deep --strict` and `spctl --assess --type execute`. Replace `GitRonimo.app` only; refuse `cargo run` / `target/` binaries.
- Confirm via `AppConfirmDialog::InstallUpdate`. No telemetry. No new crates (`curl` like `hosting_github`).
