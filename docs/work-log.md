# Implementation work log

## 2026-08-18 — Cut product version 2.0.5

**Intent:** Bump product version to **2.0.5** (`APP_VERSION` + packager) for Working Copy full-width file list and History branch/tag pills. Cargo workspace version stays `0.1.0`.

**Files:** `apps/desktop/src/views/about.rs`, `apps/desktop/Cargo.toml`, `apps/desktop/src/tests.rs`, CHANGELOG, README, AGENTS.md, PLAN.md, docs.

**Acceptance:** About and packager show `2.0.5`. Test asserts `APP_VERSION`. Changelog has a 2.0.5 section. Gates green.

## 2026-08-18 — Working Copy file list full width; History branch/tag pills

**Intent:** Working Copy file rows shrink instead of filling the list pane, and the empty diff placeholder eats the rest of the window when nothing is selected. History paints refs as typographic labels; they should read as distinct branch vs tag chips (solid pills, original GitRonimo colors — not a third-party clone). Do not bump `APP_VERSION`.

**Files:** `docs/work-log.md`, `crates/git_domain/src/lib.rs`, `crates/git_cli/src/lib.rs`, `apps/desktop/src/views/working_copy.rs`, `apps/desktop/src/views/history.rs`, `docs/UI-IMPROVE.md`, `README.md`.

**Acceptance:** With no file selected, the Working Copy file list spans the content pane. With one file selected, the split + diff return. History puts filled pills on the author line: accent for HEAD/local branches, warning for tags, muted for remotes. Parser tests classify `refs/heads/*`, `refs/tags/*`, `refs/remotes/*`. Gates green.

**References:** rgitui `crates/rgitui_ui/src/badge.rs` and graph `Badge` per `RefLabel` (MIT) — adapted pill chrome. Tower is a study screenshot only; GitComet is AGPL — approach-only.

## 2026-08-17 — Align docs with 2.0.4 shell (overlays, composer, App Settings)

**Intent:** README, AGENTS.md, and `docs/` still describe 2.0 chrome without overlay fade, composer expand motion, `overlay_anim.rs` / `OverlaySlot`, or App Settings as a first-class overlay. Match the source. Do not bump `APP_VERSION`.

**Files:** `docs/work-log.md`, `AGENTS.md`, `README.md`, `CONTRIBUTING.md`, `docs/README.md`, `docs/desktop-shell.md`, `docs/architecture.md`, `docs/UI-IMPROVE.md`, `docs/implementation-boundaries.md`, `docs/keyboard-shortcuts.md`, `docs/troubleshooting.md`, `docs/PLAN-v2.md`, `docs/PLAN-v3.md`, `apps/desktop/src/views/mod.rs`, `apps/desktop/src/views/history.rs`.

**Acceptance:** Agent and user docs name fade-in/out overlays, instant context menus, composer height+fade, and App Settings (Command-comma). History still documented as typographic ref labels (not pills). Cargo workspace version stays `0.1.0`.

## 2026-08-17 — Cut product version 2.0.4

**Intent:** Bump product version to **2.0.4** (`APP_VERSION` + packager) for overlay fade, composer expand motion, About overflow, and REST publish retry. Cargo workspace version stays `0.1.0`.

**Files:** `apps/desktop/src/views/about.rs`, `apps/desktop/Cargo.toml`, `apps/desktop/src/tests.rs`, CHANGELOG, README, AGENTS.md, PLAN.md, docs.

**Acceptance:** About and packager show `2.0.4`. Test asserts `APP_VERSION`. Changelog has a 2.0.4 section. Gates green.

## 2026-08-17 — Overlay fade and commit composer expand motion

**Intent:** Popups appear and dismiss with a short opacity fade. Clicking Commit Subject should open description / Amend / Sign-off with a height+fade motion instead of a jump. Do not bump `APP_VERSION`.

**Files:** `apps/desktop/src/views/overlay_anim.rs`, `workspace.rs`, `commit_composer.rs`, `toolbar.rs`, `app_state.rs`, `main.rs`, `tests.rs`, CHANGELOG, `docs/desktop-shell.md`.

**Acceptance:** About / Settings / palette / prompts / Pull-Push / confirms fade in and out (~150ms). Composer details ease in height and opacity (~200ms); collapse eases out. Context menus stay instant. Gates green; app runs for a visual check.

**References:** rgitui `crates/rgitui_ui/src/modal.rs` (MIT) — adapted overlay entrance `Animation` + `ease_out_quint`. GitComet is AGPL — approach-only.

## 2026-08-17 — About overlay keeps site link and Check for updates inside the card

**Intent:** About GitRonimo paints the site URL and **Check for updates** past the 520px card. The details column is `flex_1` without `min_w(0)`, so GPUI sizes it to the long in-app-updates sentence and the right column overflows. Constrain that column and wrap the copy. Do not bump `APP_VERSION`.

**Files:** `apps/desktop/src/views/about.rs`, `apps/desktop/src/tests.rs`.

**Acceptance:** `about-site-link` and `about-check-updates` right edges stay inside `about-gitronimo`. Gates green.

**References:** in-tree `min_w(0)` + `overflow_hidden` on flex children (toolbar, sidebar, Working Copy). GitComet is AGPL — approach-only.

## 2026-08-17 — Retry GitHub Releases publish on 503

**Intent:** The v2.0.3 release job notarized and uploaded the signed zip artifact, then `gh release view`/`create` failed with GitHub API 503 (“No server is currently available”). Treat that as transient: retry view/create/upload/edit like `notarytool`. Do not bump `APP_VERSION`. Do not re-notarize.

**Files:** `.github/workflows/release.yml`, `docs/packaging.md`, CHANGELOG.

**Acceptance:** Publish step retries on non-404 `gh` failures. Missing release still goes to `gh release create`. Existing tag still uses upload `--clobber`. Docs mention the retry.

**References:** in-tree notarytool retry loop. GitHub 503 body from run 32052696616.

## 2026-08-17 — Cut product version 2.0.3

**Intent:** Bump product version to **2.0.3** (`APP_VERSION` + packager) for the SHA256SUMS basename lookup and release-workflow hash path. Cargo workspace version stays `0.1.0`.

**Files:** `apps/desktop/src/views/about.rs`, `apps/desktop/Cargo.toml`, `apps/desktop/src/tests.rs`, CHANGELOG, README, AGENTS.md, PLAN.md, docs.

**Acceptance:** About and packager show `2.0.3`. Test asserts `APP_VERSION`. Changelog has a 2.0.3 section. Gates green.

**References:** in-tree version bump sites from the 2.0.2 cut.

## 2026-08-17 — SHA256SUMS zip hash lookup uses basename

**Intent:** In-app install failed with “SHA256SUMS.txt is missing the zip hash.” The release workflow ran `shasum` on `target/release-universal/GitRonimo-${tag}.zip`, so the published sums file stored that path. The parser required an exact basename and rejected `/`. Match the final path component (still reject `..`). Hash from the zip’s directory in CI. Replace the v2.0.2 GitHub `SHA256SUMS.txt` so already-shipped apps can install. Do not bump `APP_VERSION`.

**Files:** `crates/hosting_github/src/releases.rs`, `.github/workflows/release.yml`, CHANGELOG, docs.

**Acceptance:** A sums line with `target/release-universal/GitRonimo-v2.0.2.zip` returns the hash for `GitRonimo-v2.0.2.zip`. `../` entries stay rejected. Workflow writes a basename-only sums file. Live v2.0.2 asset lists `GitRonimo-v2.0.2.zip`. Gates green.

**References:** in-tree `sha256_for_filename`. Published v2.0.2 `SHA256SUMS.txt` (112 bytes, path prefix). GitComet is AGPL — approach-only.

## 2026-08-17 — Cut product version 2.0.2

**Intent:** Bump product version to **2.0.2** (`APP_VERSION` + packager) for App Settings and in-app updates default-on. Cargo workspace version stays `0.1.0`.

**Files:** `apps/desktop/src/views/about.rs`, `apps/desktop/Cargo.toml`, `apps/desktop/src/tests.rs`, CHANGELOG, README, AGENTS.md, PLAN.md, docs.

**Acceptance:** About and packager show `2.0.2`. Test asserts `APP_VERSION`. Changelog has a 2.0.2 section. Gates green.

**References:** in-tree version bump sites from the 2.0.1 cut.

## 2026-08-17 — App Settings overlay; in-app updates on by default

**Intent:** Repository Settings Off/On for Updates did not toggle (duplicate GPUI `action-button:Off` / `On` ids with auto-stash and AI). In-app updates belong on the app, not a repo. Move UPDATES into a Welcome-and-repo overlay (**GitRonimo → Settings…**, Command-comma). Default the preference **on**. Unique button ids. Do not bump `APP_VERSION`. No check on launch.

**Files:** `apps/desktop` (actions, menus, keymap, workspace, settings, about, components, main, tests, app_state), `crates/app_core`, CHANGELOG, README, AGENTS.md, docs.

**Acceptance:** Off/On in App Settings have distinct ids and persist. Missing prefs and legacy `in_app_updates: false` load as on; explicit off still persists. Menu has **Settings…**. Repo Settings no longer lists UPDATES. Overlay paints on Welcome. Gates green.

**References:** in-tree About overlay (`views/about.rs`). rgitui settings is MIT approach-only for a separate window; GitComet is AGPL — approach-only. XERJ had no unique-id Off/On pattern.

## 2026-08-17 — Diff preview actually scrolls horizontally

**Intent:** Vertical overflow worked after `overflow_scroll`, but long lines still could not pan left/right. GPUI only enables X scroll when content is wider than the pane; flex-stretched rows were sized to the viewport, so `scroll_max.x` stayed 0. Wrap hunks in an inner column with `min_w` from the longest line (GPUI scrollable example uses an explicit child width). Do not bump `APP_VERSION`.

**Files:** `apps/desktop/src/views/diff_viewer.rs`, `tests.rs`.

**Acceptance:** Inner `diff-scroll-content` is wider than `diff-scroll` when a line is longer than the pane. Stage/Discard remain on the hunk header. Gates green.

**References:** GPUI 0.2.2 `examples/scrollable.rs` (explicit child width inside `overflow_scroll`). rgitui no-wrap (MIT) adapted. GitComet is AGPL — approach-only.

## 2026-08-17 — Cut product version 2.0.1

**Intent:** Bump product version to **2.0.1** (`APP_VERSION` + packager) for the Working Copy diff-preview scroll fix. Cargo workspace version stays `0.1.0`.

**Files:** `apps/desktop/src/views/about.rs`, `apps/desktop/Cargo.toml`, `apps/desktop/src/tests.rs`, CHANGELOG, README, AGENTS.md, PLAN.md, docs.

**Acceptance:** About and packager show `2.0.1`. Test asserts `APP_VERSION`. Changelog has a 2.0.1 section. Gates green.

**References:** in-tree version bump sites from the 2.0.0 cut.

## 2026-08-17 — Diff preview scrolls vertically and horizontally

**Intent:** The Working Copy right-hand changes preview (Stage Chunk / Discard Chunk) clips long lines and tall hunks because the pane is `overflow_hidden` with no scroll container. Add a two-axis scroll area so users can pan when lines are wider than the pane and when the hunk list is taller than the window. Keep hunk action buttons on the hunk header (header text ellipsizes; line body scrolls). No Git in `Render`. Do not bump `APP_VERSION`.

**Files:** `apps/desktop/src/views/diff_viewer.rs`, `working_copy.rs`, `tests.rs`.

**Acceptance:** Diff body uses a named `diff-scroll` container with `overflow_scroll`. Lines do not wrap. A GPUI test paints Working Copy with a loaded diff and finds `diff-scroll` and Stage Chunk. Gates green.

**References:** in-tree command-palette `overflow_y_scroll`; rgitui `rgitui_diff` (MIT) no-wrap + ancestor/per-row `overflow_x_scroll` for long diff lines, adapted. GitComet is AGPL — approach-only.

## 2026-08-16 — Record post-2.0 product plan as PLAN-v3.md

**Intent:** Write `docs/PLAN-v3.md` so 3.0 work (findability, GitHub OAuth/enterprise, daily-loop polish) is not mixed into remaining PLAN-v2 `gix` fallbacks. GitHub-only for 3.0; GitLab stays 3.1. No code.

**Files:** `docs/PLAN-v3.md`, `docs/README.md`, `AGENTS.md`, `PLAN.md`, `docs/PLAN-v2.md`.

**Acceptance:** PLAN-v3 has F/H/P checkbox groups, in-scope table, and non-goals. Docs index and agent rules point at it. Remaining checkout/merge/rebase/stash/push/hooks stay in PLAN-v2.

**References:** in-tree PLAN-v2, UI-IMPROVE remaining gaps, PLAN.md Phase 9 enterprise box.

## 2026-08-16 — Cut 2.0.0; About/Settings/menu Check for updates

**Intent:** Bump product version to **2.0.0** (`APP_VERSION` + packager). Surface in-app update info and a **Check for updates** control on About (plus Settings installed-version row and the GitRonimo application menu). Still opt-in; no check on launch. Cargo workspace version stays `0.1.0`.

**Files:** `apps/desktop/src/views/about.rs`, `settings.rs`, `workspace.rs`, `actions.rs`, `menus.rs`, `main.rs`, `tests.rs`, `apps/desktop/Cargo.toml`, CHANGELOG, README, AGENTS.md, docs.

**Acceptance:** About shows `Version 2.0.0`, update status copy, and **Check for updates**. Settings UPDATES lists the installed version. Application menu has **Check for Updates…**. Tests assert `APP_VERSION`, menu item, and About button bounds. Gates green.

**References:** in-tree About overlay and Settings Check now. XERJ had no usable About-update UI (GitComet hits are AGPL — approach-only). rgitui update checker is MIT link-only; install path stays original.

## 2026-08-16 — Remove unused handlers and dead_code allows

**Intent:** Source still compiled only because of `#[allow(dead_code)]` on superseded osascript prompts, unused UI helpers, and a leftover `search_focus_handle` (Command-F focuses the search `SingleLineInput`). Delete unused code, drop stale clippy allows, keep live paths (in-app text prompts, context menus, Push/Fetch toolbar). Do not bump `APP_VERSION`.

**Files:** `apps/desktop` (`main.rs`, `app_state.rs`, `views/components.rs`, `views/working_copy.rs`, `views/welcome.rs`, `views/workspace.rs`, `views/submodules.rs`, `tests.rs`), `crates/app_core` (stop re-exporting unused `ai_commit_endpoint_is_allowed`), docs.

**Acceptance:** No `#[allow(dead_code)]` left in desktop. `return_to_welcome` stays (used). `cargo clippy --workspace --all-targets --all-features -- -D warnings` and tests stay green. Empty-state test asserts copy the Working Copy view actually shows.

**References:** in-tree source.

## 2026-08-16 — Sync docs, AGENTS.md, and tests with in-tree 2.0 work

**Intent:** Source already ships PLAN-v2 A/E/D/G (gix default, LFS/stash extras, opt-in updater, opt-in AI commits) at product version 1.0.0. README, AGENTS.md, several `docs/` files, and SECURITY still describe the 1.0 Git-CLI-only client without those Settings surfaces. Align agent rules, user docs, and tests with the code. Do not bump `APP_VERSION`. No new crates.

**Files:** `AGENTS.md`, `README.md`, `SECURITY.md`, `docs/*`, `PLAN.md` (ADR filename), `crates/app_core/src/ai_commit.rs`, `apps/desktop/src/ai_commit.rs`, `apps/desktop/src/app_state.rs`, `apps/desktop/src/main.rs` (use shared key-policy helper).

**Acceptance:** README distinguishes tagged 1.0.0 from unreleased-in-tree features. AGENTS.md documents Git engine, AI commit, and updater rules. Tests cover loopback IPv6, empty endpoint/model defaults, HTTPS-requires-key, malformed completions, curl include parse, and activity classification. Gates stay green.

**References:** in-tree source. No peer retrieval.

## 2026-08-16 — Phase G: optional AI commit messages

**Intent:** PLAN-v2 Phase G. Settings toggle (default **off**) lets the user request a commit-message suggestion from a configured HTTPS (or localhost) OpenAI-compatible endpoint. The prompt is the staged unified diff only (capped, redacted). The suggestion fills the composer; the user must edit/commit as usual. Failure is a no-op. No telemetry. No new HTTP/AI crate (`curl` + `serde_json` already in tree). Do not send the GitHub PAT or the full repo.

**Files:** `crates/app_core` (preference, prompt/JSON/parse), `crates/git_domain` (diff excerpt), `crates/platform_macos` (Keychain key), `apps/desktop` (Settings, composer Suggest, palette), docs.

**Acceptance:** Preference defaults off. Empty staged diff / disabled toggle / missing remote key do not call the network. Fixture tests: URL allowlist, prompt contains only the supplied diff, completion JSON → subject/body, markdown fences stripped. Suggest does not invoke commit. API errors leave the composer unchanged.

**References:** rgitui `rgitui_ai` (MIT) — OpenAI-compatible chat JSON and fill-composer (not auto-commit), adapted; do **not** copy tool-calling or README/CLAUDE.md project context (PLAN: staged diff only). GitComet is AGPL — approach-only.

## 2026-08-16 — Phase D: in-app updates (opt-in)

**Intent:** PLAN-v2 Phase D. Settings toggle (default **off**) lets the user check GitHub Releases for a newer notarized `GitRonimo-v*.zip`, confirm, download, verify SHA-256, Gatekeeper-assess the extracted `.app`, then replace this bundle. No telemetry, no PAT, no unsigned bits, no new crates (`curl` like `hosting_github`). Do not check on launch.

**Files:** `crates/hosting_github` (public latest-release JSON), `crates/app_core` (preference), `apps/desktop` (`app_update.rs`, Settings, palette, confirm), docs.

**Acceptance:** Fixture tests: version compare, SHA256SUMS parse, HTTPS allowlist, release JSON assets. Preference defaults off. Disabled Check now is a no-op message. Non-`.app` runs refuse install. `codesign --verify --deep --strict` and `spctl --assess --type execute` must succeed before replace. Failed hash or Gatekeeper leaves the running app untouched.

**References:** rgitui `update_checker.rs` (MIT) — GitHub latest-release check + semver compare, adapted; it only opens a download link. Install/verify is original. GitComet announce-only is AGPL — approach-only.

## 2026-08-16 — Phase E: named stash snapshots

**Intent:** PLAN-v2 Phase E. Named, non-destructive stash snapshots: keep the working copy, store a recovery entry in the stash list. Tracked-only uses `git stash create` + `git stash store`. Untracked/pathspec uses `stash push` then `stash apply` so the tree is restored. System Git only. Do not put Git in stash-row `Render`.

**Files:** `crates/git_cli` (`create_stash_snapshot`), `apps/desktop` (prompt, Stashes header, palette), docs.

**Acceptance:** Temp-repo: dirty tracked file remains after snapshot; stash list has one named entry. Include-untracked snapshot leaves the untracked file in the worktree. Empty tree and `-` messages are refused. Desktop: Save snapshot… prompt with include-untracked; palette; Stashes header. Save/apply/pop/drop unchanged.

**References:** gitui/rgitui have no snapshot (XERJ). Magit snapshot is create+store (approach). GitComet is AGPL — approach-only.

## 2026-08-16 — Phase E: auto-stash around switch/pull

**Intent:** PLAN-v2 Phase E. Opt-in Settings toggle (default off) stashes dirty work before branch switch and pull, then reapplies it. Pull uses Git `--autostash`. Switch uses `git stash push --include-untracked` + operation + `stash pop` on system Git. Do not put Git in Settings `Render`.

**Files:** `crates/app_core` (preference), `crates/git_cli` (`maybe_autostash`, pull flag), `apps/desktop` (Settings, checkout/pull/sync/workflow), docs.

**Acceptance:** Preference defaults off and persists. Temp-repo: overlapping dirty switch fails without autostash and succeeds with it (WIP follows or remains as a named stash on pop conflict). Pull with `--autostash` keeps a non-overlapping dirty file. Settings Off/On; checkout, tracking checkout, detached checkout, Pull, and Sync honor the flag.

**References:** gitui checkout is keep-vs-discard, not autostash (XERJ). rgitui has no autostash. Implement originally using Git `--autostash` / stash push-pop. GitComet is AGPL — approach-only.

## 2026-08-16 — Phase E: stash drag-and-drop partial apply

**Intent:** PLAN-v2 Phase E. Apply a subset of a stash onto the working copy without dropping the stash. Stash file rows are selectable and draggable onto the Working Copy sidebar; Git stays in `main.rs`. Mutations stay on system Git (`git restore --source=<stash> --worktree --staged -- paths`).

**Files:** `crates/git_cli` (`apply_stash_paths`), `apps/desktop` (stash file selection, GPUI `StashPathDrag`, Working Copy drop target, palette), docs.

**Acceptance:** Temp-repo test: two dirty files stashed, restore one path, the other stays at HEAD; `..` / absolute paths and non-stash refs are refused. Desktop: click/Cmd-click stash files, Apply selected files, drag onto Working Copy. Stash list is unchanged after a partial apply.

**References:** gitui has no stash DnD (TUI apply-all). rgitui/XERJ had no stash-file drop target. Bookmark-folder `on_drag`/`on_drop` in `sidebar.rs` is the in-window pattern (adapted). GitComet is AGPL — approach-only.

## 2026-08-16 — Phase E: Git LFS fetch and pull UI

**Intent:** Start [`PLAN-v2.md`](PLAN-v2.md) Phase E. The Git LFS status view already lists changed paths. Add fetch (download objects) and pull (download + checkout into the worktree) on **system Git**, with the same cancellable network operation as remotes. Do not put Git in LFS-row `Render`.

**Files:** `crates/git_cli` (`lfs_fetch`, `lfs_pull`), `apps/desktop` (LFS view actions, palette, `run_network_command`), docs.

**Acceptance:** Typed `git lfs fetch` / `git lfs pull` with an optional remote. Temp-repo test (skip if Git LFS is missing): skip-smudge clone keeps a pointer; pull materializes content. LFS view and palette run fetch/pull against the default remote, cancellable, then refresh Working Copy and the LFS list. No new crates.

**References:** gitui has no LFS (issue #2812). XERJ returned no usable gitui/rgitui fetch/pull UI; implement originally. GitComet is AGPL — approach-only.

## 2026-08-16 — Phase A: gix history, trees/diffs, stage/commit, HTTPS fetch/clone

**Intent:** Finish PLAN-v2 Phase A prefer-gix items. History (rev-walk), tree/blob reads and unified diffs, low-level stage/unstage/commit, and HTTPS fetch/clone use `gix`. System Git remains fallback. SSH/`file://` clone/fetch, hunks, hooks, and `commit.gpgsign` stay on `git_cli`. Fetch/clone cancel via `AtomicBool` (gix) and `GitChild` (CLI fallback).

**Files:** `crates/git_domain` (`CommitRequest`, `LoadedDiff`), `crates/app_core/src/git_engine.rs` (history/object/index/network ports), `crates/git_gix` (history/objects/mutate/network + gix features), `crates/git_cli` (trait impls + re-exports), `apps/desktop` (`git_backend`, call sites, interrupt), docs.

**Acceptance:** Dual-backend tests on temp repos: history page (Current + pagination), tree entries + blob bytes, file/commit diffs (hunk line kinds, not byte-identical headers), stage/unstage/commit/amend, HTTP URL routing (gix refuses ssh/file). Desktop history, Working Copy, composer, fetch, and clone use the ports. `cargo deny check` accepts the expanded `gix` graph.

**References:** gitui (MIT) `asyncgit/src/sync/logwalker.rs` — `gix` rev-walk + commit-time order, adapted. GitComet history/index mappers are AGPL — approach-only.

## 2026-08-16 — Phase A: gix worktree status and untracked listing

**Intent:** Migrate working-copy status (index vs HEAD, index vs worktree, untracked, optional ignored) to `gix`. System Git remains fallback. `stash_count` uses the `refs/stash` reflog; `in_progress_operation` stays a git-dir file check. Do not write index stat updates from status.

**Files:** `crates/app_core/src/git_engine.rs` (`GitRefQuery::worktree_status`), `crates/git_gix` (`status` feature + mapper), `crates/git_cli` (trait impl), `apps/desktop/src/git_backend.rs`, `apps/desktop/src/main.rs` (welcome snapshot + `load_working_copy`), docs.

**Acceptance:** Dual-backend tests on a temp repo: untracked, unstaged/staged/both, ignored on/off, stash count, unusual filename. Entries match `git_cli` after sorting by path (document rename-score or typechange deviations if any). Desktop Working Copy and welcome snapshot use the engine port. `cargo deny check` accepts the expanded `gix` graph.

**References:** gitui (MIT) `asyncgit/src/sync/status.rs` — `status().into_iter()` IndexWorktree + TreeIndex, adapted. GitComet status mapper is AGPL — approach-only.

## 2026-08-16 — Phase A: gix default Git engine (discover / HEAD / refs)

**Intent:** Start [`PLAN-v2.md`](PLAN-v2.md) Phase A. `gix` (gitoxide library, not the CLI) becomes the default engine for repository discovery, HEAD, and ref snapshots. System Git remains fallback and the Settings override. `git_domain` stays types-only.

**Files:** `docs/adr/0003-gix-default-git-engine.md`, `crates/git_gix/`, `crates/app_core` (preference + `GitRefQuery`), `crates/git_cli` (`GitRefQuery` + `head_status`), `apps/desktop` (router, Settings), workspace `Cargo.toml` / `Cargo.lock`, docs.

**Acceptance:** Dual-backend tests on a temporary repo: discover, HEAD (branch / detached / unborn), ref snapshot (local/remote/tags/ahead-behind) match `git_cli`. Settings “Use system Git” persists. `cargo deny check` accepts the pinned `gix` graph. Unmigrated operations still use `git_cli`.

**References:** gitui (MIT) `gix::discover` / `references()` — adapted. GitComet gix+CLI split is approach-only.

## 2026-08-16 — Narrow 2.0.0 plan to gix + D/E/G

**Intent:** Replace the broad post-1.0 hosting/a11y/i18n plan with `gix` (gitoxide) as the main internal Git engine and system Git as fallback. Keep only in-app updates (D), LFS/stash extras (E), and optional AI commits (G).

**Files:** `docs/PLAN-v2.md`, `docs/README.md`, `docs/todo-v1.md`, `AGENTS.md`, `PLAN.md` (pointer).

**Acceptance:** `PLAN-v2.md` has phases A, D, E, G only. No product code changes.

## 2026-08-16 — Save 2.0.0 plan

**Intent:** Record the post-1.0.0 deferred work (OAuth, enterprise GitHub, other hosts, VoiceOver, updater, localization, LFS UI, stash extras, optional CLIs/AI) as `docs/PLAN-v2.md` so 1.0 tagging does not mix with v2 scope.

**Files:** `docs/PLAN-v2.md`, `docs/README.md`, `docs/todo-v1.md`, `AGENTS.md`, `PLAN.md` (pointer).

**Acceptance:** Docs index and `todo-v1.md` point at `PLAN-v2.md`. No product code changes.

## 2026-08-14 — Message history clock time

**Intent:** Message history popup shows the local hour and minute when each line was recorded (`HH:MM`), not a relative age (`24s`, `1m`).

**Files:** `crates/platform_macos/src/clock.rs`, `crates/platform_macos/src/lib.rs`, `crates/platform_macos/Cargo.toml`, `apps/desktop/src/views/workspace.rs`, `apps/desktop/src/tests.rs`, `docs/desktop-shell.md`.

**Acceptance:** Popup timestamps match `HH:MM`. Unit test checks the format. Click-to-select and activity coalescing are unchanged.

## 2026-08-14 — Drag Working Copy files to other macOS apps

**Intent:** Drag a file (or the current multi-selection) from the Working Copy staging list onto another macOS app so that app opens the path. GPUI `on_drag` is in-window only; this uses AppKit `NSDraggingItem` file URLs (`public.file-url`). Only paths that already exist under the worktree are offered. Deleted/missing entries are skipped.

**Files:** `docs/adr/0002-macos-file-drag.md`, `crates/platform_macos/src/{lib.rs,file_drag.rs}`, `crates/platform_macos/Cargo.toml`, `apps/desktop/src/{app_state.rs,main.rs,tests.rs,views/working_copy.rs}`, `PLAN.md`, `docs/keyboard-shortcuts.md`, `docs/implementation-boundaries.md`, `CHANGELOG.md`.

**Acceptance:** Unit tests cover path picking (multi-select, missing files, `..` rejection). Drag starts after a small mouse movement on a status row, not on a plain click. Cocoa FFI stays in `platform_macos`.

## 2026-08-14 — PLAN §18 temp files, textconv, URL redaction

**Intent:** Close remaining 1.0.0 security checklist items: commit-message files `0o600` and always unlinked; `diff --numstat` uses `--no-ext-diff --no-textconv`; Git stderr/activity text redacts URL userinfo and token prefixes.

**Files:** `crates/git_cli/src/lib.rs`, `apps/desktop/src/app_state.rs`, `apps/desktop/src/tests.rs`, `PLAN.md`, `docs/todo-v1.md`.

**Acceptance:** Tests cover 0o600 + unlink, numstat flags, and redaction. PLAN §18 items that this implements are checked.

## 2026-08-14 — Cut product version 1.0.0

**Intent:** Ship the existing 0.9 daily client as **1.0.0**. Close doc lies (screens links, gpui-component baseline, notarization “incomplete”). Keep OAuth, enterprise GitHub, VoiceOver, and in-app updates as known limitations. Network progress bar should follow Git `%` lines instead of a fake 45% start / 92% cap.

**Files:** `apps/desktop/src/views/about.rs`, `apps/desktop/Cargo.toml`, `apps/desktop/src/tests.rs`, `apps/desktop/src/main.rs`, `views/workspace.rs`, `views/components.rs`, `CHANGELOG.md`, `README.md`, `PLAN.md`, `docs/*`.

**Acceptance:** About and packager show `1.0.0`. README points at GitHub latest. Known limitations no longer claim notarization is missing. Merge/rebase are documented as continue/abort + interactive rebase view (not a missing feature).

## 2026-08-14 — Remove third-party Git-client product names from docs

**Intent:** Describe GitRonimo as its own client. Do not name other commercial Git GUIs in product docs, agent rules, or code comments.

**Files:** `PLAN.md`, `AGENTS.md`, `README.md`, `CHANGELOG.md`, `docs/UI-*.md`, `docs/todo-v1.md`, `docs/desktop-shell.md`, `docs/work-log.md`, desktop view comments, `apps/desktop/src/tests.rs`.

**Acceptance:** Repo search for those product names returns no matches.

## 2026-08-14 — Document v1 leftover work

**Intent:** Analyze project `*.md` docs against shipped 0.9.2 and write a single v1 todo at `docs/todo-v1.md`.

**Files:** `docs/todo-v1.md`, `docs/README.md`.

**Acceptance:** Todo lists remaining product work, doc hygiene, and explicit out-of-v1 items; docs index links it.

## 2026-08-14 — Dock icon.png vs About gitronimo-icon.png

**Intent:** Dock/Finder icon is repo-root `icon.png`. About overlay stays `assets/gitronimo-icon.png`.

**Files:** `assets/gitronimo.icns`, `apps/desktop/Cargo.toml`, `docs/packaging.md`, `CHANGELOG.md`, `docs/desktop-shell.md`.

**Acceptance:** Packaged `.app` icns is built from `icon.png`; About still embeds `assets/gitronimo-icon.png`.

## 2026-08-14 — App logo.png, README docs/logo.png, bin/build

**Intent:** About and dock branding use repo-root `logo.png`. README hero image is `docs/logo.png`. Add `bin/build` to produce a local unsigned `GitRonimo.app`.

**Files:** `logo.png`, `docs/logo.png`, `assets/gitronimo.icns`, `apps/desktop/src/assets.rs`, `views/about.rs`, `Cargo.toml`, `README.md`, `bin/build`, packaging docs.

**Acceptance:** About loads `logo.png`; README points at `docs/logo.png`; `./bin/build` packages `GitRonimo.app`.

## 2026-08-14 — Bump product version to 0.9.2

**Intent:** Align About / packager `APP_VERSION` and README with product version **0.9.2**.

**Files:** `apps/desktop/src/views/about.rs`, `apps/desktop/Cargo.toml`, `apps/desktop/src/tests.rs`, `README.md`, `CHANGELOG.md`, current-version mentions in `docs/`.

**Acceptance:** About and bundle metadata show `0.9.2`.

## 2026-08-14 — App icon from gitronimo-icon.png + README logo

**Intent:** Dock/app icon from `assets/gitronimo-icon.png` (regenerate `gitronimo.icns`). README shows `assets/gitronimo-logo.png` plus `docs/screenshot.png`.

**Files:** `assets/gitronimo.icns`, `apps/desktop/Cargo.toml`, `README.md`, `docs/packaging.md`.

**Acceptance:** Packager `icons` uses the new icns built from `gitronimo-icon.png`; README has logo then screenshot.

## 2026-08-14 — Bump product version to 0.9.1

**Intent:** Align About / packager `APP_VERSION` and README with the shipped GitHub tag **v0.9.1**.

**Files:** `apps/desktop/src/views/about.rs`, `apps/desktop/Cargo.toml`, `apps/desktop/src/tests.rs`, `README.md`, `CHANGELOG.md`, current-version mentions in `docs/`.

**Acceptance:** About and bundle metadata show `0.9.1`; README current release is 0.9.1.

**Verification:** `about_dialog_uses_the_release_version`; fmt/clippy on desktop.

## 2026-08-14 — Release workflow: notary wait retry + existing GitHub release

**Intent:** Latest `v0.9` job imported the cert, signed, and uploaded to notary (`036ece19-86af-4c2e-9536-111822052206`), then `notarytool --wait` hit an App Store Connect HTTP timeout. `gh release create` would also fail because `v0.9` already exists as a notes-only pre-release.

**Files:** `.github/workflows/release.yml`, `docs/packaging.md`.

**Acceptance:** Notary submit/wait retries on timeout; signed zip is uploaded as an Actions artifact; publish updates an existing tag release instead of failing.

## 2026-08-13 — 0.9 release prep (docs + dual-arch builds)

**Intent:** Align README, AGENTS.md, and docs with shipped 0.9 product (GitRonimo menu/bundle, About overlay, Workflow, Services removed, `APP_VERSION`). Promote CHANGELOG to **0.9**. Document and produce unsigned Apple Silicon and Intel `.app` zips.

**Files:** `README.md`, `AGENTS.md`, `CHANGELOG.md`, `docs/*`, `.github/workflows/ci.yml`, `rust-toolchain.toml`.

**Acceptance:** Docs match current UX and packaging paths (`GitRonimo.app`); `APP_VERSION` documented as the bump site; arm64 and x86_64 bundles exist under `target/` with SHA-256 sums. Signing/notarization remain CI-secret / tag workflow.

**Verification:** `cargo fmt --check`, clippy `-D warnings`, workspace tests, `cargo deny check`. `lipo -archs` reports `arm64` and `x86_64` on the two `GitRonimo.app` executables; Info.plist `CFBundleShortVersionString` is `0.9`. Zips: `target/dist/GitRonimo-0.9-macos-arm64.zip` and `target/dist/GitRonimo-0.9-macos-x86_64.zip`.

## 2026-08-13 — About overlay black + release version 0.9

**Intent:** About panel uses a black card. Show product version **0.9** from a dedicated constant (not Cargo `0.1.0`) so it can be bumped after each release.

**Files:** `apps/desktop/src/views/about.rs`, `apps/desktop/src/tests.rs`, `docs/desktop-shell.md`, `CHANGELOG.md`.

**Acceptance:** Overlay card is black with light copy; About shows `Version 0.9`; `APP_VERSION` is the single bump site.

**Verification:** `cargo fmt --check`, clippy `-D warnings`, desktop About tests, `cargo deny check`.

## 2026-08-13 — About GitRonimo (menu + overlay)

**Intent:** The macOS application menu showed `gitronimo-desktop` (debug binary name). Rename the visible app to **GitRonimo**, add **About GitRonimo**, and show a centered overlay: app icon, name, version, “Made in Austin ✩ Texas”, and https://aomega.co. No Acknowledgements/License buttons.  About layout; original Gitronimo chrome and `assets/gitronimo-icon.png`.

**Files:** `apps/desktop/Cargo.toml` (bin + packager name), `assets.rs`, `actions.rs`, `menus.rs`, `app_state.rs`, `main.rs`, `views/about.rs`, `workspace.rs`, `tests.rs`, CI/packaging docs.

**Acceptance:** Menu bar title is GitRonimo when running the desktop binary; About opens the overlay; site link uses `open_url`; gates pass.

**Verification:** `cargo fmt --check`, clippy `-D warnings`, workspace tests, `cargo deny check`.

## 2026-08-13 — Crash-report panics (double-lease + drag cursor)

**Intent:** Local crash reports under `~/Library/Application Support/Gitronimo/crash-reports/` plus matching macOS IPS stacks: (1) `entity_map.rs` double-lease while rendering the branch context menu (`submenu_item` nested `GitronimoApp` read); (2) `window.rs:2364` `set_window_cursor_style` debug-assert during mouse-down (GPUI copies `.cursor_*()` onto `on_drag` and applies it as a window cursor). Cargo future-incompat (`block`, `proc-macro-error2`) is transitive GPUI/macOS and is not patched here.

**Files:** `views/ref_context_menu.rs`, `views/components.rs`, `views/workspace.rs`, `views/sidebar.rs`, `app_state.rs`, `main.rs`, `tests.rs`, docs.

**Acceptance:** Opening a local-branch context menu (and Push To submenu) draws without double-lease; resize/bookmark drag sources do not put `mouse_cursor` on the same element as `on_drag`; hover still shows col-resize via ancestor style; gates pass.

**Verification:** `cargo fmt --check`, clippy `-D warnings`, workspace tests, `cargo deny check`. rgitui `layout.rs` resize handle keeps cursor+drag together (MIT, not copied) because that combo trips gpui 0.2.2’s paint-phase assert.

## 2026-08-13 — Command-F focuses toolbar search

**Intent:** The toolbar search field shows ⌘F but the shortcut was not bound, so it did nothing. Focus the visible search input (welcome repositories or in-repo files).

**Files:** `actions.rs` (`FocusSearch`), `keymap.rs` (`cmd-f`), `menus.rs`, `main.rs`, `workspace.rs`, `toolbar.rs` (hint on both shells), `tests.rs`, docs.

**Acceptance:** Command-F focuses the toolbar search from welcome and from an open repository; binding test asserts the keystroke.

**Verification:** `cargo fmt --check`, clippy `-D warnings` on gitronimo-desktop, `command_f_is_bound_to_focus_search` test passes.

## 2026-08-13 — Command-H hides the app

**Intent:** Match Command-Q: standard macOS hide shortcut and an application menu entry.

**Files:** `actions.rs` (`Hide`), `keymap.rs` (`cmd-h`), `menus.rs` (Gitronimo → Hide Gitronimo), `main.rs` (`cx.on_action(… cx.hide())`), `workspace.rs` (shortcut overlay), `tests.rs`, `README.md`, `docs/keyboard-shortcuts.md`.

**Acceptance:** Command-H hides from any window state; the menu item shows ⌘H; binding test asserts the keystroke.

**Verification:** `cargo fmt --check`, clippy `-D warnings` on gitronimo-desktop, `command_h_is_bound_to_hide` / `command_q_is_bound_to_quit` tests pass.

## 2026-08-13 — Workflow tab (core)

**Intent:** Replace the Workflow placeholder with a functional page inspired by Workflows overview: choose GitHub Flow / GitLab Flow / git-flow or auto-detect from existing branches; start topic branches with prefixes; finish with merge/squash/rebase; sync a topic onto its parent. Deferred: Graphite CLI, git-flow CLI, restack stacks, auto-archive protection.

**Files:** `crates/app_core` workflow model + prefs, `git_cli` squash/ff-only merge, `views/workflow.rs`, toolbar/welcome/sidebar/`main.rs`/`app_state.rs`, docs.

**Acceptance:** Workflow tab (welcome + in-repo) lists templates and applied config; Start/Finish/Sync mutate via typed Git; config persists per repo; gates pass.

**Verification:** `cargo fmt --check`, clippy `-D warnings`, workspace tests, `cargo deny check` all pass. workflow templates overview / choosing / configuring / topic-branch guides are approach-only (templates, auto-detect, Start/Finish/Sync). Graphite CLI, git-flow CLI, restack, and auto-archive protection remain deferred.

## 2026-08-13 — Remove Services navigation and view

**Intent:** Drop the Services welcome tab, in-repo destination, palette command, and hosted-repo browser. GitHub token connect/sign-out moves to Settings so Pull Requests still have an account path.

**Files:** `views/services.rs` (delete), toolbar/welcome/sidebar/working_copy/settings/pull_requests, `app_state.rs`, `main.rs`, docs.

**Acceptance:** No Services tab or palette entry; Bookmarks/Workflow remain; Settings connects GitHub; PRs no longer link to Services; gates pass. **Done.**

**Follow-up:** Keep a 72×48 empty slot in the toolbar where Services was so Bookmarks/Workflow stay aligned.

**Verification:** `cargo fmt --check`, clippy `-D warnings`, workspace tests, `cargo deny check` all pass. GitHub Keychain connect remains in Settings for Pull Requests.

## 2026-08-12 — Sidebar HEAD ahead/behind badge 

**Intent:** Match the current-branch pill: `HEAD` plus compact `↑N` / `↓N` for unpublished / unpulled commits. reuse porcelain ahead/behind already shown in the toolbar.

**Files:** `apps/desktop/src/views/components.rs` (`format_divergence_arrows`, `head_badge`), `sidebar.rs`, `toolbar.rs`, `branches_review.rs`, `welcome.rs`, `tests.rs`, docs.

**Acceptance:** HEAD row shows `HEAD ↑1` (and `↓N` when behind) in a muted pill; toolbar tracking uses the same `↑N` / `↓N` order; in-sync HEAD stays `HEAD` only; gates pass. **Done.**

**Verification:** `cargo fmt --check`, clippy `-D warnings`, workspace tests, `cargo deny check` all pass.

## 2026-08-12 — Stashes (core parity)

**Intent:** Match the stash guide core flows: save dialog (message + untracked + path-limited), apply dialog (delete-after / restore index), stash changeset detail, branch-from-stash. rgitui `stashes_panel.rs` for Apply/Pop/Drop/Branch shape (MIT).

**Files:** `git_domain`/`git_cli` stash APIs (`CreateStashRequest`, `--index`, `stash_branch`, list `%ct`), `app_state` dialogs, `stashes.rs`, `working_copy.rs`, `toolbar.rs`, `workspace.rs`, `main.rs`, palette, keymap `cmd-shift-s`, docs.

**Acceptance:** Save/apply dialogs; WC Stash selected; detail shows date/files/diff; Branch…; list refresh; gates pass. **Done.**

**Verification:** `cargo fmt --check`, clippy `-D warnings`, workspace tests, `cargo deny check` all pass. rgitui `stashes_panel.rs` MIT for Apply/Pop/Drop/Branch action shape.

## 2026-08-12 — Palette: history/commit actions coverage

**Intent:** After the History commit context menu, expose the same user-facing handlers (and a few adjacent ones) in the command palette so they are discoverable without right-click.

**Files:** `app_state.rs` (`PaletteCommand` / `PALETTE_COMMANDS`), `main.rs` (`run_palette_command`), `docs/desktop-shell.md`, `docs/work-log.md`, `CHANGELOG.md`.

**Acceptance:** Palette lists amend, create tag, history filter, selected-commit copy/checkout/reset/revert/patch/export/compare, branch-from-selected, rebase onto, merge revision, stash untracked, branches review; gates still pass. **Done.**

## 2026-08-12 — History commit context menu 

**Intent:** Right-click a History commit to open a grouped menu (copy, checkout detach, reset/revert/rebase with confirms, amend/reword when HEAD, create branch/tag, patch, export, compare). Original menu chrome reused from the ref context menu.

**Files:** `crates/git_cli` (detach/reset/format-patch), `app_state.rs` (CommitContext, confirms, reset choice), `views/commit_context_menu.rs`, `history.rs`, `workspace.rs`, `main.rs`, `docs/desktop-shell.md`, `docs/UI-IMPROVE.md`, `docs/work-log.md`.

**Acceptance:** Right-click opens menu; Reset/Revert/Delete gated to HEAD-branch history scope; Hard reset / Revert / Delete confirm; Amend & Edit Message only on HEAD; Edit disabled with reason; gates pass. **Done.**

## 2026-08-12 — Docs: shell chrome functional pass

**Intent:** After the shell feature commits (message history, force-delete confirm, pins, palette), sync `docs/` and `AGENTS.md` with the shipped behavior.

**Files:** `docs/desktop-shell.md` (new), `docs/README.md`, `docs/architecture.md`, `docs/keyboard-shortcuts.md`, `docs/troubleshooting.md`, `docs/UI-IMPROVE.md`, `CHANGELOG.md`, `README.md`, `AGENTS.md`, `docs/work-log.md`.

**Acceptance:** Agents and humans can find activity/palette/pin/confirm rules without reading the diff; work-log/UI-IMPROVE/CHANGELOG reflect 2026-08-12 shell work.

## 2026-08-12 — Extend command palette coverage + scroll

**Intent:** Command palette only showed the first ~9 items (list clipped, not scrollable). Expose toolbar and common Git/shell actions in the searchable list and make the list scroll.

**Files:** `app_state.rs` (`PaletteCommand` / `PALETTE_COMMANDS`), `main.rs` (`run_palette_command`), `workspace.rs` (scrollable list), `docs/work-log.md`.

**Acceptance:** Palette lists Fetch/Pull/Push/Sync, stash, stage, settings, create branch, etc.; typing filters; mouse wheel scrolls past the viewport; Enter still runs the selection.

## 2026-08-12 — Message history: successes, errors, scroll

**Intent:** The Message history popup should retain success notices (e.g. push complete), errors, and confirmations—not only refresh chatter—and the list must scroll when it exceeds the popup height.

**Files:** `app_state.rs` (kinds), `main.rs` (`set_activity` coalesce), `workspace.rs` (scroll + row styling), `components.rs` (`activity_color`), `docs/work-log.md`.

**Acceptance:** After push + refreshes, history still shows the push complete line; errors/confirmations appear with distinct colors; mouse wheel scrolls the list.

## 2026-08-12 — Persist pinned branches across relaunch

**Intent:** Pinned branches must look the same after quitting and reopening. Prefs already store pins, but unsynchronized preference writes (window geometry, widths, etc.) can overwrite them, and nested pins stay hidden inside folders so relaunch looks unchanged.

**Files:** `crates/app_core/src/lib.rs` (serialize preference RMW), `apps/desktop/src/views/sidebar.rs` (pins flat atop BRANCHES), `docs/work-log.md`, `docs/UI-IMPROVE.md`, `docs/desktop-shell.md`.

**Acceptance:** Pin a nested branch → quit → reopen → it still appears flat above other BRANCHES in pin order (no PINNED label); concurrent geometry saves do not clear pins.

## 2026-08-12 — Unmerged branch delete confirmation dialog

**Intent:** Safe branch delete failures (not fully merged) currently flash in the activity bar. Show a modal (“Could Not Delete Branch”) with Cancel / Delete (force), and keep the initial delete prompt as Cancel / Delete only.

**Files:** `app_state.rs` (`AppConfirmDialog`), `main.rs` (delete error → dialog), `workspace.rs` (overlay), `docs/work-log.md`.

**Acceptance:** Delete unmerged branch → Yes → modal with unmerged copy; Cancel closes; Delete runs `git branch -D`; activity log still records the failure.

## 2026-08-12 — Activity message history popup

**Intent:** Bottom activity bar gets a small button that opens a popup of the last 50 status/error/notification messages so users can review what happened.

**Files:** `app_state.rs` (log + toggle), `main.rs` (`set_activity`), `workspace.rs` (button + popup), `docs/work-log.md`.

**Acceptance:** Messages append to a capped log; button opens anchored popup; newest first; click outside / button again closes; current activity line unchanged. Later: successes/errors/confirmations retained (refresh coalesced); list scrolls.

## 2026-08-12 — sidebar HEAD vs selection

**Intent:** Match the branch rows: selection is a full-width accent ribbon with light text; HEAD is a trailing badge. When HEAD is the selected row, the badge sits on the ribbon (light outlined pill). When another branch is selected, HEAD keeps a muted badge without a ribbon.

**Files:** `components.rs` (`head_badge` variants), `sidebar.rs` (row styling), `branches_review.rs`, `docs/work-log.md`.

**Acceptance:** Selected history branch = accent ribbon; HEAD-on-selection badge matches ribbon; HEAD-not-selected shows muted badge only; selected non-HEAD still gets the ribbon.

## 2026-08-12 — Keep composer open for Amend / Sign-off

**Intent:** Checking Amend or Sign-off must keep the commit subject block expanded after focus leaves the fields.

**Files:** `main.rs` (`sync_commit_composer_expanded`, `toggle_commit_sign_off`), `commit_composer.rs` (checkbox mouse-down), `single_line_input.rs`.

**Acceptance:** With empty subject/body, checking Amend or Sign-off keeps the expanded composer open; unchecking both (and clearing fields) allows collapse.

## 2026-08-12 — Faster commit composer expand

**Intent:** Clicking Commit Subject felt delayed before the description/options appeared. Expand on mouse-down (not only focus-in) and use a short ease-out fade so the panel opens immediately but still smoothly.

**Files:** `single_line_input.rs` (expand on mouse-down), `commit_composer.rs` (snappier reveal animation), `docs/work-log.md`.

**Acceptance:** Subject click opens description + options with no noticeable pause; fade is short (ease-out); collapse behavior unchanged.

## 2026-08-12 — Branch delete confirmation + network progress chrome

**Intent:** Delete branch from the sidebar must show a Yes/No confirmation popup from any view (not only Working Copy). Network ops (fetch/pull/push/…) must show an in-progress bar in the bottom-left UI so the user can see work is happening.

**Files:** `workspace.rs` (delete confirm overlay; activity-bar progress), `working_copy.rs` (remove WC-only delete strip), `main.rs` (`cancel_branch_delete`, clear pending on confirm), `sidebar.rs` / `components.rs` (keep footer), `docs/UI-IMPROVE.md`, `docs/work-log.md`.

**Acceptance:** Delete… → Cancel/Delete modal; Delete runs safe `git branch -d`; unmerged refusal opens “Could Not Delete Branch” for force (`-D`); progress bar visible bottom-left during network ops.

## 2026-08-12 — Double-click sidebar branch switches checkout

**Intent:** activate: double-click a local or remote branch in the sidebar checks it out. Single-click still opens scoped History; tags stay History-only on double-click.

**Files:** `views/sidebar.rs` (click_count gate), `main.rs` (`activate_ref_from_double_click`, remote tracking path), `git_cli` (`checkout_tracking_branch`), `docs/UI-IMPROVE.md`.

**Acceptance:** Double-click local branch → `git switch`; double-click remote without local → `git switch --track`; existing local short name → switch to it; already-on-HEAD shows a short activity note; single-click History unchanged.

## 2026-08-11 — Checkbox click target and file-name column

**Intent:** Staging by checkbox felt broken in the running app even though the handler is correct. The visible box is 14px inside a 22px row with no padding, so a near miss hit the row instead — and a row click on a full selection clears it, which looks like nothing happened. Give the checkbox a row-height hit area, surface ignored clicks during an in-flight mutation, and fix the file-name column, which stripped leading letters from staged rows and showed the raw `.M` prefix on unstaged ones.

**Files:** `views/working_copy.rs` (hit area, `display_path`), `main.rs` (`toggle_path_staged` activity message), `tests.rs` (rendered-click test using `debug_selector`/`debug_bounds`).

**Acceptance:** A simulated click 2px inside the corner of the checkbox stages the whole selection and keeps it selected; file names render without status prefixes; staged rows keep their full path.

## 2026-08-11 — Command-Q quits the app

**Intent:** Standard macOS quit shortcut and an application menu entry, which the window was missing.

**Files:** `actions.rs` (`Quit`), `keymap.rs` (`cmd-q`), `menus.rs` (Gitronimo → Quit Gitronimo), `main.rs` (`cx.on_action(… cx.quit())`), `workspace.rs` (shortcut overlay), `tests.rs`, `README.md`, `docs/keyboard-shortcuts.md`.

**Acceptance:** Command-Q quits from any window state; the menu item shows ⌘Q; window geometry is already persisted on every bounds change, so nothing is lost on quit. Binding test asserts the keystroke.

## 2026-08-11 — Single-file selection reachable again after Select All

**Intent:** With every file selected (Command-A, or a preserved selection after batch staging), a plain row click only cycled all/none, so no single file could be picked and staged. Limit that toggle to repeated clicks on the same row.

**Files:** `app_state.rs` (`file_list_select_all_toggle: Option<GitPath>` replaces the bool), `main.rs` (`select_status_path`), `tests.rs`, `AGENTS.md`, `docs/keyboard-shortcuts.md`.

**Acceptance:** Select all → click row A clears → click row B selects only B → click A twice cycles none/all. Lists with a single visible file always single-select. GPUI tests against a temporary repository cover selection and batch/single checkbox staging.

## 2026-08-11 — Batch checkbox direction follows the whole selection

**Intent:** With several files selected, a checkbox click should check every selected box (stage all) and a second click should clear them (unstage all). Previously the clicked row's own staged state chose the direction, so clicking a staged row inside a partly staged selection unstaged everything.

**Files:** `main.rs` (`should_stage_selection`, `path_is_staged`, `toggle_path_staged`), `working_copy.rs` (`entry_is_staged` visibility), `docs/keyboard-shortcuts.md`.

**Acceptance:** Select all → click any checkbox → every box checked; click again → all cleared. Mixed selection stages first. Single-file clicks still toggle only that file. Unit tests cover the three cases.

## 2026-08-11 — Push dialog (destination + push options)

**Intent:** Toolbar Push opens a "Push HEAD" dialog: title + description naming the local HEAD branch, Destination dropdown (remote branch), Options list — Push All Tags, Force Push, Recurse Submodules (with verify/on-demand mode), Skip Hooks — and Cancel / Push HEAD buttons.

**Files:** `app_state.rs` (`PushDialogState`, `SubmodulePushMode`), `main.rs` (open/close/toggles/confirm, `push_command_args`), `workspace.rs` (overlay), `toolbar.rs`, `working_copy.rs` (branch menu Push…), docs.

**Acceptance:** Push opens dialog with destination prefilled from upstream; toggles change the composed command; Push HEAD runs `git push --progress [...] <remote> HEAD:<branch>`; Cancel closes without pushing; Force Push uses `--force-with-lease` per AGENTS safety rule; Sync stays immediate.  screenshot.

## 2026-08-11 — Pull dialog (remote branch + rebase option)

**Intent:** Toolbar Pull opens a dialog: Remote Branch dropdown, collapsible Options with "Use Rebase Instead of Merge", Pull/Cancel. Confirm runs `git pull --progress [--rebase] [remote branch]`.

**Files:** `app_state.rs` (`PullDialogState`), `main.rs` (open/confirm/args), `workspace.rs` (overlay), `toolbar.rs`, `working_copy.rs` (context menu Pull…), docs.

**Acceptance:** Pull button shows dialog; choose remote branch; expand Options → rebase checkbox; Pull runs network command; Cancel closes without pulling.  guides / GitComet PullMode.

**Done:** `PullDialogState` overlay; toolbar/context-menu open; `git pull --progress [--rebase] remote branch`; unit tests for arg split; Sync stays immediate.

## 2026-08-11 — Sidebar branch click opens History 

**Intent:** Left-click a local/remote branch or tag in the sidebar opens History scoped to that ref (middle commit list + right changeset detail),. Right-click keeps the context menu. Highlight the active sidebar ref while viewing its history; auto-select the tip commit so panel 3 shows file changes.

**Files:** `main.rs` (`select_ref_context`, history load select), `sidebar.rs` (row highlight), `docs/UI-IMPROVE.md`, `docs/work-log.md`.

**Acceptance:** Click branch → History for that branch; click commit → file changes in inspector; selected branch highlighted; right-click menu unchanged.

**Done:** `select_ref_context` → `show_ref_history` for branch/tag; tip commit auto-selected on fresh history load; sidebar accent for named History scope; docs updated.

## 2026-08-11 — Stay on Working Copy after commit

**Intent:** After a successful commit/amend from Working Copy, do not navigate to History; clear the composer, refresh status, and remain on Working Copy. Still remember the new OID so History can reveal it when the user opens that view later.

**Files:** `apps/desktop/src/main.rs` (commit success path), `docs/UI-IMPROVE.md`, `docs/work-log.md`.

**Acceptance:** Commit or Amend leaves the user on Working Copy with refreshed file list; History is not auto-opened.

**Done:** removed `navigate_to(History)` / eager `load_history` from commit success path; still sets `history_reveal_oid` for later History open; clears selection/diff; docs updated.

## 2026-08-11 — Documentation pass + README screenshots

**Intent:** Align all docs with current beta scope (Working Copy multi-select, History/sidebar polish, Services/PRs/stashes); add committed Gitronimo screenshots to README; index docs under `docs/README.md`.

**Files:** `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `AGENTS.md`, `.gitignore`, `docs/README.md`, `docs/screens/README.md`, `docs/screens/gitronimo-*.png`, `docs/UI-IMPROVE.md`, `docs/UI-PLAN.md`, `docs/architecture.md`, `docs/troubleshooting.md`, `docs/keyboard-shortcuts.md`.

**Acceptance:** README shows welcome, repositories, working copy, history, and branch menu; beta scope matches implemented features; docs index links all major files; product screenshots are committed.

**Done:** five `gitronimo-*.png` captures committed; README/CHANGELOG scope updated; docs index and screens README added.

## 2026-08-11 — Working Copy select all / toggle deselect / batch checkbox

**Intent:** Cmd/Ctrl+A selects all files in the visible Working Copy list; when all are selected, clicking any selected row deselects all; the next row click re-selects all (toggle). When multiple files are selected (Cmd-A, Shift, or Cmd-click), clicking a row checkbox stages/unstages all selected files and keeps list selection.

**Files:** `actions.rs`, `keymap.rs`, `main.rs`, `app_state.rs`, `workspace.rs`, `working_copy.rs` (`visible_status_paths`).

**Acceptance:** Cmd-A in WC selects all changed files; click selected row clears all; next click selects all again; normal single-click still works otherwise. Checkbox on a multi-selected row stages/unstages all selected files and keeps the file selection.

**Done:** `SelectAllStatusFiles` action; `visible_status_paths`; toggle deselect/reselect; `run_mutation(..., preserve_selection)` for checkbox batch staging; docs updated (`AGENTS.md`, `keyboard-shortcuts.md`, `UI-IMPROVE.md`).

## 2026-08-11 — History list full-width flat rows

**Intent:** Match the central History list: no per-row card backgrounds (only selected row highlighted); rows span full pane width; branch/ref pills stay readable (contrast + truncate, don’t crush into subject).

**Files:** `docs/work-log.md`, `apps/desktop/src/views/history.rs`.

**Acceptance:** unselected rows transparent/full-bleed; selected = accent full width; ref pills readable; no floating grey cards.

**Done:** rows `w_full` with bg only when selected; month headers flat; ref pills high-contrast + truncated; gates pass.

## 2026-08-11 — History central list match the rows

**Intent:** Central History commit list must match the: multi-lane colored graph + node dots; two-line rows (author/date over hash+subject+ref pills); solid accent selection with inverted text. Approach adapted from rgitui graph paint (MIT).

**Files:** `docs/work-log.md`, `crates/git_domain` (`GraphRow.active_lanes`), `apps/desktop/src/views/history.rs`.

**Acceptance:** Graph shows colored lanes/nodes; two-line row layout; selection is accent blue.

**Done:** `GraphRow.active_lanes`; multi-color through-lanes + node dots (rgitui MIT approach); two-line author/date + hash/subject/pills; accent selection.

## 2026-08-11 — History layout + trim workspace nav

**Intent:** History should match the commit list + detail inspector (scope header, month groups, denser rows with ref pills, Changeset/Tree detail). Remove Pull Requests, Branches Review, and Reflog from the left sidebar (keep enum/palette so nothing breaks).

**Files:** `docs/work-log.md`, `apps/desktop/src/views/history.rs`, `apps/desktop/src/views/sidebar.rs`, `apps/desktop/src/views/components.rs` (date helpers), `app_state.rs` / `main.rs` / `workspace.rs` only if filter choice prompt is needed.

**Acceptance:** History fills the pane with list+detail; scope header instead of action-button strip; sidebar workspace is Working Copy / History / Stashes / Settings; PRs/Branches Review/Reflog still reachable from palette; gates + relaunch.

**Done:** History list fills pane (flex_1/h_full); scope header + Filter choice; month groups, denser rows, ref pills, Changeset detail; sidebar trimmed; `show_history` from sidebar; default scope All refs.

## 2026-08-11 — Branch actions as right-click popup

**Intent:** Working Copy should stay (composer + files only). The inline “Local branch: …” action panel must become a right-click context popup on sidebar branches/remotes/tags (context-menu pattern), not replace the WC pane.

**Files:** `docs/work-log.md`, `working_copy.rs`, `sidebar.rs`, `workspace.rs`, `main.rs` (`open`/`close` ref menu).

**Acceptance:** WC has no inline branch menu; right-click branch opens anchored popup with Checkout/History/Merge/…; outside click / action closes; left-click does not open the panel.

**Done:** Inline panel removed from WC; sidebar right-click opens `ref_context_menu_overlay`; menu actions close popup; gates + relaunch.

## 2026-08-11 — Amend checkbox loads HEAD + Amend button

**Intent:** Checking Amend shows last commit short hash beside the checkbox, fills subject (and body) from that commit, switches the primary button to active “Amend”, and clicking it runs `git commit --amend` with staged changes / message.

**Files:** `docs/work-log.md`, `crates/git_cli` (`head_commit_summary`), `app_state.rs`, `main.rs`, `commit_composer.rs`.

**Acceptance:** toggle fills fields + hash; button label/enablement; amend succeeds; uncheck restores prior draft; gates.

## 2026-08-11 — Description as 4-line textarea + focus ring

**Intent:** Detailed description should be a ~4-line tall textarea with the same blue focus border as Commit Subject when focused. Do not change subject field behavior/layout.

**Files:** `docs/work-log.md`, `single_line_input.rs` (`composer_multiline_shell`), `commit_composer.rs` (pass focus flag only).

**Acceptance:** body shell ~4 lines tall; focus_ring when `commit_body_focused`; subject untouched; relaunch.

## 2026-08-11 — Full-width Detailed description only

**Intent:** Commit Subject already full-width — do not touch it. Detailed description renders as a ~0-width vertical slit; make that field alone span the commit card content width (same as subject).

**Files:** `docs/work-log.md`, `commit_composer.rs`, `single_line_input.rs` (`composer_multiline_shell` only).

**Acceptance:** description shell matches subject width; subject code path unchanged; gates + relaunch.

## 2026-08-11 — Shared COMPOSER_FIELD_HEIGHT; relaunch

**Intent:** Subject and description share one `COMPOSER_FIELD_HEIGHT` (32) via one shell helper. Never collapse when subject/body trimmed non-empty, amend on, or either focused. Kill old app and relaunch so the build is what the user sees.

**Files:** `docs/work-log.md`, `single_line_input.rs`, `main.rs` (verify sync).

**Acceptance:** shared const; keep-open rule; gates + relaunch.

## 2026-08-11 — Match description height; keep expanded on subject text

**Intent:** Description control same outer height as subject (32px). Stay expanded when subject or body has text (or focus/amend); collapse only when both unfocused and both empty and amend off.

**Files:** `docs/work-log.md`, `single_line_input.rs`, `main.rs`.

**Acceptance:** matching heights; collapse rules updated; gates pass.

## 2026-08-11 — Restore always-visible Commit Subject

**Intent:** Collapsed commit card showed only branch + Stage All/Commit — subject/text fields gone. Absolute-fill shells likely collapsed to zero-size. Restore simple always-on subject shell (flex + definite height, visible field bg); details only when expanded; no max_h clip.

**Files:** `docs/work-log.md`, `commit_composer.rs`, `single_line_input.rs`.

**Acceptance:** collapsed shows subject full width; expand shows description; editable; gates pass.

## 2026-08-11 — Restore description; full-width + tighter type

**Intent:** Expanded composer showed Amend/footer but description was empty/clipped (`max_h` + overflow). Make description a normal full-width block; align field borders with Stage All row; smaller text + ~10% more vertical padding in subject/description.

**Files:** `docs/work-log.md`, `commit_composer.rs`, `single_line_input.rs`.

**Acceptance:** description visible when expanded; fields match footer width; typography/padding updated; gates pass.

## 2026-08-11 — Force full-width composer fields (absolute fill)

**Intent:** Subject/description bordered boxes still content-narrow despite flex/cached attempts. Replace with definite `w_full` chrome + `absolute().inset_0()` fill; counter inside subject shell. Entity stretched via explicit StyleRefinement + absolute containing block.

**Files:** `docs/work-log.md`, `apps/desktop/src/views/single_line_input.rs`, `commit_composer.rs`.

**Acceptance:** bordered fields span card content width; counter trailing inside subject; focus/expand intact; gates pass.

## 2026-08-11 — Full-width commit composer inputs

**Intent:** Subject and description bordered fields must stretch across the commit card (counter stays trailing). Fix Entity layout that stayed intrinsic/narrow despite flex shells.

**Files:** `docs/work-log.md`, `apps/desktop/src/views/single_line_input.rs`, `commit_composer.rs` as needed.

**Acceptance:** both fields full-width; click/focus still work; toolbar/prompt shells OK; gates pass.

## 2026-08-11 — Fix unclickable Commit Subject

**Intent:** Commit Subject (and description when expanded) must focus on click so caret appears, typing works, and expand runs. Root cause likely zero-width Entity hitbox inside composer shells.

**Files:** `docs/work-log.md`, `apps/desktop/src/views/single_line_input.rs`, `commit_composer.rs` / `main.rs` if focus wiring needed.

**Acceptance:** click subject focuses + expands; description clickable when open; gates pass.

## 2026-08-11 — Commit composer expand/collapse

**Intent:** Match the commit card expand/collapse: collapsed shows branch + subject + Stage All/Commit; expand on subject focus to reveal description, Amend/Sign-off, author; stay open if body text or amend; collapse when both unfocused and body empty and amend off. Soft height animation if feasible.

**Files:** `docs/work-log.md`, `commit_composer.rs`, `single_line_input.rs` (focus hooks if needed), `app_state.rs`, `main.rs`.

**Acceptance:** expand/collapse rules work; Stage All/Commit always visible; gates pass.

## 2026-08-11 — Working Copy commit card redesign

**Intent:** Redesign commit composer into a raised card: branch header, subject + always-visible description, Amend/Sign-off checkboxes + author, Stage All / Commit footer. Keep file filter/list below, visually separate. No AI generate; original copy and assets.

**Files:** `docs/work-log.md`, `apps/desktop/src/views/commit_composer.rs`, `working_copy.rs`, `single_line_input.rs` (body Enter/newline + shell), possibly `components.rs`.

**Acceptance:** card layout matches IA; body always editable; amend/sign-off clear; Stage All/Commit wired; gates pass.

## 2026-08-11 — In-repo sidebar polish

**Intent:** Clearer branch-tree hierarchy (indent + chevron/folder/branch icons) and replace unicode nav glyphs with Heroicons outline; denser section labels/rows/subtle dividers. Original branding only.

**Files:** `docs/work-log.md`, `apps/desktop/assets/icons/*`, `assets.rs`, `views/icons.rs`, `views/sidebar.rs`, `views/components.rs` (section/badge if needed).

**Acceptance:** nested branches clearly indented; outline icons on workspace nav + ref tree; gates pass.

## 2026-08-11 — Compact text prompts + bookmark drag preview

**Intent:** (1) Shrink "New group" / other `text_prompt_overlay` dialogs so height hugs content (no huge empty region below buttons). (2) Show human-friendly drag preview (repo name) for `BookmarkRepoDrag` instead of empty element; keep folder/root drops working.

**Files:** `docs/work-log.md`, `apps/desktop/src/views/workspace.rs`, `apps/desktop/src/views/sidebar.rs`.

**Acceptance:** text prompts compact; drag shows repo name near cursor; drops still work; gates pass.

## 2026-08-11 — Fix welcome + and palette buttons

**Intent:** Sidebar footer `+` must open a anchored popup (New Group / Add Repository); toolbar Palette button must open the command palette (`open_command_palette`). Investigate click swallowing / dispatch_action vs direct call; keep Services/Bookmarks/Workflow in toolbar only.

**Files:** `docs/work-log.md`, `apps/desktop/src/app_state.rs`, `main.rs`, `views/sidebar.rs`, `views/toolbar.rs`, `views/workspace.rs`.

**Acceptance:** `+` click opens anchored menu and actions work; Palette opens command palette; fmt/clippy/test/deny pass.

## 2026-08-11 — Welcome/bookmarks UI polish (six issues)

**Intent:** Fix welcome/bookmarks UX from annotated screenshots: (1) sidebar `+` footer padding/hit target with New Group / Add Repository menu; (2) remove unwanted welcome detail rename/"some repo" field and redundant sidebar border gap (keep resize handle); (3) ensure folder/group tree renders with chevron/folder icons, nested repos, create via `+`, rename/delete, DnD; (4) shell tabs more horizontal space + icon-over-label; (5) toolbar search padding/placeholder/alignment; (6) replace unicode/letter placeholders with Heroicons-style outline SVGs (MIT, vendored; no new crates).

**XERJ:** searched `sidebar folder tree bookmark`, toolbar icon+label, welcome plus menu; adapted approach from in-repo sidebar/toolbar + rgitui SVG AssetSource pattern (MIT); GitComet approach-only.

**Files:** `docs/work-log.md`, `apps/desktop/assets/icons/*`, `apps/desktop/src/assets.rs`, `apps/desktop/src/views/icons.rs`, `sidebar.rs`, `welcome.rs`, `toolbar.rs`, `components.rs`, `single_line_input.rs`, `app_state.rs`, `main.rs`.

**Acceptance:** footer `+` comfortable; no demo description field; folders visible/creatable; shell tabs + search polished; outline icons attributed; fmt/clippy/test/deny pass.

**Verification:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` all pass. Icons: Heroicons v2.1.5 outline (MIT) in `apps/desktop/assets/icons/` with `README.md` + `HEROICONS-LICENSE.txt`.

## 2026-08-11 — Bookmark folder tree + toolbar shell tabs

**Intent:** bookmarks: explicit folders (create/rename/delete, expand/collapse, drag repos between folders; delete folder unwraps repos to root). Move Services/Bookmarks/Workflow from left rail into the top toolbar and keep them visible when a repository is open.

**Design:**
- Persist `bookmark_folders` + `repository_folder` map in `RecentRepositoryDocument` (serde defaults, same schema version).
- Replace path-parent auto-grouping with explicit folder membership; root = repos with no folder id.
- Toolbar always shows Services / Bookmarks / Workflow; activating them returns to welcome with that shell view.
- Welcome sidebar: chevron toggle, folder/repo rows, DnD via distinct drag types, + opens Add Repo / New Folder choice; text prompts for create/rename.

**Files:** `crates/app_core`, `app_state.rs`, `main.rs`, `toolbar.rs`, `sidebar.rs`, `welcome.rs`, `workspace.rs`, `components.rs`, work-log.

**Acceptance:** folders CRUD + toggle + DnD; delete folder leaves repos at root; shell tabs in toolbar on welcome and in-repo; no left welcome rail.

## 2026-08-11 — Welcome bookmarks layout 

**Intent:** fix welcome Repositories pane mess (overflow onto rail, branch lines looking like extra repos) and tighten the layout: contained sidebar column, single-line folder rows, toolbar search only, dashed drop-zone empty state.

**Files:** `sidebar.rs` (welcome sidebar/rows), `welcome.rs` (empty state / actions), work-log.

**Acceptance:** sidebar does not overlap the icon rail; repo rows are one line; empty state reads as a drop zone; search lives in the toolbar.

## 2026-08-11 — Full-height draggable panes + left toolbar title

**Intent:** make sidebar|content and list|diff dividers full-height and draggable with persisted widths; left-align toolbar title (repo › branch).

**Files:** `crates/app_core` (persist widths), `apps/desktop` workspace/sidebar/working_copy/toolbar, work-log.

**Acceptance:** both dividers span content height; drag left/right updates and remembers widths across relaunch; toolbar title left-aligned.

## 2026-08-11 — Welcome page text/layout fixes

**Intent:** fix welcome UI issues from screenshot: hide Prev/Next on first page, stop Bookmarks rail label wrapping, stop repo list name truncation beside branch.

**Files:** `apps/desktop/src/views/toolbar.rs`, `components.rs` (`welcome_rail_tab`), `sidebar.rs` (`welcome_repo_row`), `docs/work-log.md`

**Acceptance:** welcome toolbar has no Prev/Next; Bookmarks fits on one line in the rail; selected repo name shows in full (branch on second line or truncated secondarily).

## 2026-08-11 — GPUI choice overlays + reword / merge-tool prompts

**Intent:** replace remaining high-frequency osascript `choose from list` and text dialogs (set merge tool, merge PR method, reword last commit, open merge tool path) with in-app GPUI overlays.

**Design:** add `ChoicePromptKind` + `choice_prompt_overlay` (fixed option list with click / ↑↓ / Enter / Escape, same scrim pattern as command palette); migrate `SetMergeTool` and PR merge method picker onto it (PR merge keeps a Confirm step); extend `TextPromptKind` for reword subject/body (two-step) and optional conflict path for merge tool.

**Files (planned):**
- `apps/desktop/src/app_state.rs` — `ChoicePromptKind`, choice state fields, text prompt variants
- `apps/desktop/src/views/workspace.rs` — choice overlay render; wire into root layout
- `apps/desktop/src/main.rs` — open/confirm/cancel choice; migrate prompts; remove targeted osascript
- `docs/work-log.md` — this entry

**Acceptance checks:**
- Set merge tool uses GPUI list (no osascript choose-from-list).
- Merge pull request method + confirm uses GPUI overlays.
- Reword last commit uses two-step GPUI text prompts.
- Open in merge tool path uses GPUI text prompt (blank = all conflicts).
- Full workspace gates: fmt, clippy `-D warnings`, test, deny.

**Verification:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` all pass.

## 2026-08-11 — GPUI command palette + Enter-to-confirm

**Intent:** replace the largest remaining osascript cluster (`choose from list` command palette) with a searchable in-app GPUI overlay, and add Enter-to-confirm (Escape-to-cancel) on the existing text prompt overlay.

**Design:** typed `PaletteCommand` + label table; `command_palette_overlay` reuses `overlay_scrim` / `single_line_input` (new `TextFieldBinding::CommandPalette`); filter is case-insensitive substring; click or Enter runs the selected/first match; Escape closes. Text prompt overlay gains Enter/Escape via SingleLineInput actions. Optionally migrate single-field osascript prompts (drop commit, browse tree, history search/reference, rebase onto) onto `TextPromptKind` if low-risk.

**Files (planned):**
- `apps/desktop/src/app_state.rs` — palette state, `PaletteCommand`, extra TextPromptKinds
- `apps/desktop/src/views/single_line_input.rs` — CommandPalette binding, Enter/Escape actions
- `apps/desktop/src/views/workspace.rs` — command palette overlay; focus pending
- `apps/desktop/src/main.rs` — open/close/dispatch palette; remove osascript choose-from-list; migrate prompts
- `docs/work-log.md` — this entry

**Acceptance checks:**
- Command-Shift-P opens searchable GPUI palette (no osascript list).
- Typing filters commands; Enter runs highlighted/first match; Escape/scrim closes.
- Text prompts confirm on Enter and cancel on Escape when the field is focused.
- Command dispatch remains typed (no shell-string Git).
- Full workspace gates: fmt, clippy `-D warnings`, test, deny.

**Verification:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` all pass. Command palette is in-app; drop/browse/history/rebase/autosquash prompts use GPUI overlays with Enter/Escape.

## 2026-08-11 — GPUI text prompts + Branches Review filter

**Intent:** replace frequent osascript text dialogs with in-app GPUI overlays (same pattern as branch rename) and default Branches Review to diverged/unpublished branches with a Show all toggle.

**Design:** introduce `TextPromptKind` + unified `text_prompt_overlay` (`overlay_scrim` + `single_line_input` with `TextFieldBinding::TextPrompt`); migrate branch rename into the shared prompt; replace osascript for create branch from ref/commit, file history path, blame path, and two-step compare refs; Branches Review filters to unpublished or ahead/behind branches unless `branches_review_show_all` is set, with header toggle and selection by branch name.

**Files changed:**
- `apps/desktop/src/app_state.rs` — `TextPromptKind`, text prompt state, review filter flag, selection by name
- `apps/desktop/src/views/single_line_input.rs` — `TextPrompt` binding
- `apps/desktop/src/views/workspace.rs` — unified text prompt overlay
- `apps/desktop/src/main.rs` — prompt begin/confirm/cancel, remove osascript for targeted flows
- `apps/desktop/src/views/branches_review.rs` — diverged filter + Show all toggle
- `apps/desktop/src/views/working_copy.rs`, `history.rs`, `compare.rs`, `file_history.rs`, `blame.rs` — call GPUI prompts

**Acceptance checks:** create branch from ref/tag/commit menu uses GPUI overlay; file history and compare refs palette/view actions use GPUI overlays (compare is two-step); branch rename still uses overlay; Branches Review defaults to diverged/unpublished with Show all; full workspace gates pass.

## 2026-08-11 — Branches Review list + GPUI branch rename

**Intent:** continue post–UI-PLAN polish on `feature/ui-improvements`: flesh out Branches Review beyond empty state and replace the branch-rename osascript dialog with an in-app GPUI prompt.

**Design:** extend `NamedRef` / `ref_snapshot` with upstream and ahead/behind from `git for-each-ref`; Branches Review two-pane lists local branches with divergence badges and a detail panel (HEAD marker, upstream, checkout); branch rename uses `overlay_scrim` + `single_line_input` (`TextFieldBinding::BranchRename`) instead of AppleScript.

**Files changed:**
- `crates/git_domain/src/lib.rs` — upstream/ahead/behind on `NamedRef`
- `crates/git_cli/src/lib.rs` — extended ref format + parser test
- `apps/desktop/src/app_state.rs` — branch review selection, rename prompt state
- `apps/desktop/src/views/single_line_input.rs` — `BranchRename` binding
- `apps/desktop/src/views/branches_review.rs` — branch list + detail
- `apps/desktop/src/views/workspace.rs` — rename overlay
- `apps/desktop/src/main.rs` — wire rename prompt, init state
- `docs/UI-PLAN.md` — Branches Review deferral note

**Acceptance checks:** Branches Review lists local branches with upstream divergence; selecting a branch shows detail and checkout; rename branch from context menu uses GPUI overlay (no osascript); ref parser test covers ahead/behind; full workspace gates pass.

## 2026-08-11 — UI polish pass (~1% remaining parity)

**Intent:** close remaining polish gaps from `docs/UI-PLAN.md` deferrals/sign-off without OAuth/enterprise scope.

**Design:** replace welcome repo description osascript dialog with GPUI `SingleLineInput` (`TextFieldBinding::RepoDescription`); load upstream/ahead/behind into `WelcomeRepoSnapshot` and show badges on welcome sidebar rows + detail panel; add Branches Review sidebar entry with two-pane empty state; add `overlay_scrim` theme token for Quick Open overlay in light/dark.

**Files changed:**
- `apps/desktop/src/views/single_line_input.rs` — `RepoDescription` binding, fifth input in bundle
- `apps/desktop/src/views/welcome.rs` — inline description field, upstream/tracking detail rows
- `apps/desktop/src/views/sidebar.rs` — upstream badges on repo rows, Branches Review nav
- `apps/desktop/src/views/branches_review.rs` — new stub view
- `apps/desktop/src/views/working_copy.rs`, `toolbar.rs`, `mod.rs` — route Branches Review
- `apps/desktop/src/app_state.rs` — snapshot upstream fields, list snapshot cache
- `apps/desktop/src/main.rs` — batch snapshot load, remove osascript description prompt
- `apps/desktop/src/views/workspace.rs` — overlay_scrim token
- `crates/ui_kit/src/theme.rs` — `overlay_scrim` light/dark
- `docs/UI-PLAN.md` — deferrals + token inventory

**Acceptance checks:** welcome description editable inline; repo list shows branch/upstream divergence badges; Branches Review reachable with empty state; Quick Open scrim uses theme token; fmt/clippy pass; desktop/ui_kit tests pass.

## 2026-08-11 — UI plan execution (Phases 0–10)

**Intent:** implement `docs/UI-PLAN.md` — reach UI consistency across welcome and in-repo views.

**Design:** Phase 0 creates UI-PLAN.md; Phase 1 adds GPUI `SingleLineInput` for search/composer; Phases 2–9 cover welcome shell, toolbar, WC polish, settings, remote progress, secondary views, theme; Phase 10 sign-off.

**Files (planned):** `docs/UI-PLAN.md`, `views/single_line_input.rs`, `views/settings.rs`, `views/components.rs`, `views/toolbar.rs`, `views/working_copy.rs`, `views/commit_composer.rs`, `views/diff_viewer.rs`, `views/welcome.rs`, `views/sidebar.rs`, secondary view modules, `git_cli`, `app_state.rs`, `main.rs`, `ui_kit/theme.rs`, `docs/UI-IMPROVE.md`

**Acceptance checks:** all UI-PLAN phases checked off; sign-off table complete; workspace gates pass; app launches.

## 2026-08-10 — UI consistency pass (cross-view design language)

**Intent:** unify typography, selection states, row heights, empty states, panel headers, and two-pane layouts across all views across views — full-width accent list selection, shared section headers/detail rows, 28px panel headers, 22px compact rows, Services/PRs two-pane parity with Stashes/Remotes.

**Design:** extract `section_header`, `detail_section`, `detail_row`, `view_panel_header`, `two_pane_view`, `accent_list_row`, `head_badge`, layout constants in `components.rs`; nav rows edge-to-edge accent (no rounded inset); welcome/services/PRs/history empty states via `centered_empty_state`; services + pull requests adopt list|detail split; welcome detail uses shared detail helpers; theme documents layout heights.

**Files changed:**
- `apps/desktop/src/views/components.rs` — shared layout helpers and constants
- `apps/desktop/src/views/sidebar.rs` — full-width nav selection, section headers
- `apps/desktop/src/views/welcome.rs` — shared detail/empty state helpers
- `apps/desktop/src/views/services.rs` — two-pane layout, accent list rows
- `apps/desktop/src/views/pull_requests.rs` — two-pane layout, accent list rows
- `apps/desktop/src/views/stashes.rs` — shared panel header (28px)
- `apps/desktop/src/views/remotes.rs` — shared panel header (28px)
- `apps/desktop/src/views/history.rs` — shared empty state
- `apps/desktop/src/views/working_copy.rs` — shared empty state
- `crates/ui_kit/src/theme.rs` — color tokens (light/dark)
- `apps/desktop/src/views/components.rs` — layout height constants + shared helpers
- `docs/UI-IMPROVE.md` — consistency pass status

**Acceptance checks:** all list selections use full-width accent bars; section headers match across welcome/sidebar/detail; panel headers 28px; empty states use icon + title + detail pattern; Services/PRs match Stashes/Remotes two-pane density; all workspace gates pass; app launches.

## 2026-08-10 — interior views polish (sidebar, WC, history, diff, stashes, remotes)

**Intent:** close visual gaps inside opened-repository views across views (overview-05/06/08, workflow-03/04/05/06) — repo sidebar IA, WC composer layout, file-row selection/badges, toolbar subtitle, history density, diff line backgrounds, stashes/remotes two-pane layouts.

**Design:** drop welcome-only Repositories/Services headers from in-repo sidebar; uppercase section labels + scrollable ref tree with remote branches under Remotes; commit row (subject + count, then Stage All | Commit); accent file-row selection + square status badges; toolbar subtitle `View (branch - N Changed Files)`; structured history rows + segmented Changeset/Tree header; diff added/removed row backgrounds; stashes/remotes list|detail split.

**Files changed:**
- `apps/desktop/src/views/sidebar.rs` — in-repo IA, section labels, scroll area, remotes grouping
- `apps/desktop/src/views/commit_composer.rs` — two-row commit area
- `apps/desktop/src/views/working_copy.rs` — selection, badges, empty diff state
- `apps/desktop/src/views/toolbar.rs` — subtitle format, Quick Open label
- `apps/desktop/src/views/history.rs` — row density, detail header toggle
- `apps/desktop/src/views/diff_viewer.rs` — line row backgrounds
- `apps/desktop/src/views/commit_detail.rs` — two-pane changeset layout
- `apps/desktop/src/views/stashes.rs` — list + detail two-pane
- `apps/desktop/src/views/remotes.rs` — list + detail two-pane
- `apps/desktop/src/views/components.rs` — shared sidebar label, badge, empty state, segmented toggle
- `docs/UI-IMPROVE.md` — interior polish status

**Acceptance checks:** in-repo sidebar has no welcome clutter; WC composer uses subject + count then Stage All | Commit; file rows 22px with square badges and accent selection; history/detail/stashes/remotes use two-pane density; diff lines show add/remove backgrounds; all workspace gates pass; app launches.

## 2026-08-10 — UI parity pass: welcome rail, inline search, remote progress, polish

**Intent:** close remaining `docs/UI-IMPROVE.md` gaps — welcome vertical Services/Bookmarks/Workflow rail (§1.1), always-visible inline toolbar search (§4), persistent remote progress in sidebar footer (§1.5), visual parity (toolbar density, commit area, file rows, diff tabs, detail headers, activity bar).

**Design:** add `WelcomeShellView::Workflow` and a left icon rail on welcome; replace osascript search prompts with focusable GPUI inline search fields synced to `welcome_repo_search` / `worktree_file_search`; sidebar footer shows indeterminate progress bar during fetch/pull/push plus last result when idle; polish commit composer focus border + right-aligned Commit, diff staged/unstaged segmented tabs, welcome detail REPOSITORY/WORKING COPY/REMOTES headers, HEAD badge and chevrons, activity bar success coloring.

**Files changed:**
- `apps/desktop/src/app_state.rs` — Workflow shell view, search focus handle, network progress
- `apps/desktop/src/main.rs` — inline search handlers, progress tick, Workflow routing
- `apps/desktop/src/views/workspace.rs` — welcome rail slot, activity bar polish
- `apps/desktop/src/views/welcome.rs` — vertical rail, Workflow placeholder, detail headers
- `apps/desktop/src/views/toolbar.rs` — inline search, remove horizontal welcome tabs
- `apps/desktop/src/views/sidebar.rs` — inline sidebar filter, progress footer, HEAD badge
- `apps/desktop/src/views/components.rs` — inline search field, rail tab, progress bar
- `apps/desktop/src/views/commit_composer.rs` — focus border, Commit placement
- `apps/desktop/src/views/diff_viewer.rs` — staged/unstaged tab styling, hunk header
- `crates/ui_kit/src/theme.rs` — separator token usage (if needed)
- `docs/UI-IMPROVE.md` — updated remaining gaps

**Acceptance checks:** welcome shows vertical Services/Bookmarks/Workflow rail; toolbar and sidebar search filter live; sidebar footer shows progress during remote ops and last result when idle; WC/diff/welcome/detail match the density; all workspace gates pass; app launches.

## 2026-08-10 — UI alignment: toolbar, sidebar, history, welcome search, WC polish

**Intent:** close remaining gaps from `docs/UI-IMPROVE.md` § Remaining gaps — toolbar search/stash/refresh cluster, edge-to-edge sidebar selection, History inline Changeset/Tree toggle, welcome repo filter, Working Copy subject count / Stage All / column headers / softer row selection.

**Design:** stacked labeled toolbar buttons (Fetch/Pull/Push/Sync, Apply/Save stash, Refresh); clickable search fields wired to osascript filter prompts (`welcome_repo_search`, `worktree_file_search`); full-width accent sidebar selection; reuse Commit Detail changeset/tree panels in History inspector; composer subject remaining-char count (50) + inline Stage All; Status|Filename column header parity; raised-background file row selection.

**Files changed:**
- `apps/desktop/src/app_state.rs` — search filter fields
- `apps/desktop/src/main.rs` — init/clear search, prompt helpers
- `apps/desktop/src/views/toolbar.rs` — grouped stacked actions + search
- `apps/desktop/src/views/sidebar.rs` — edge-to-edge selection, welcome filter field
- `apps/desktop/src/views/history.rs` — inline Changeset/Tree inspector
- `apps/desktop/src/views/commit_detail.rs` — pub(crate) shared panels
- `apps/desktop/src/views/commit_composer.rs` — char count, Stage All
- `apps/desktop/src/views/working_copy.rs` — file filter, headers, softer selection
- `apps/desktop/src/views/components.rs` — toolbar search + stacked button helpers
- `docs/UI-IMPROVE.md` — updated remaining gaps table

**Acceptance checks:** toolbar shows search + labeled remote/stash/refresh clusters; welcome sidebar filters recents; History detail pane toggles Changeset/Tree; sidebar selection is full-width blue; WC composer shows 50-char count and Stage All; file list headers align; all workspace gates pass; app launches.

## 2026-08-10 — UI audit follow-up: welcome tabs, sidebar badges, doc honesty

**Intent:** close the most visible remaining gaps after auditing `docs/UI-IMPROVE.md` against current screenshots — welcome lacks Services/Repositories tabs (§1.1), sidebar lacks HEAD badge and Working Copy count badge, grouping toggle exists but has no UI (§1.2), remote-activity footer only appears during in-flight ops (§1.5 partial).

**Design:** add `WelcomeShellView` segmented control in the welcome toolbar (Repositories | Services) routing welcome content and sidebar; expose the existing `repositories_grouped` toggle in the welcome sidebar header; show a white pill count badge on selected Working Copy nav row and a HEAD pill on the checked-out branch; show last fetch/pull/push result in sidebar footer when idle; update `UI-IMPROVE.md` implementation status with a Remaining gaps subsection.

**Files changed:**
- `apps/desktop/src/app_state.rs` — `WelcomeShellView`, `welcome_shell_view`, `last_network_result`
- `apps/desktop/src/main.rs` — welcome tab setter, network result tracking, remove dead_code on grouping toggle
- `apps/desktop/src/views/toolbar.rs` — welcome shell tabs, Prev/Next navigation labels
- `apps/desktop/src/views/sidebar.rs` — grouping toggle, HEAD badge, badge styling, services welcome sidebar, idle remote footer
- `apps/desktop/src/views/welcome.rs` — route Services tab to hosting view
- `apps/desktop/src/views/services.rs` — hide repo-only nav on welcome shell
- `docs/UI-IMPROVE.md` — honest status + remaining gaps

**Acceptance checks:** welcome toolbar shows Repositories/Services tabs; Services tab loads hosting view; welcome sidebar grouping toggle works; current branch shows HEAD badge; Working Copy badge inverts on selection; sidebar footer shows last remote result when idle; all workspace gates pass; app launches.

## 2026-08-10 — UI alignment: Welcome / Repositories view

**Intent:** replace the dashed drop-zone welcome screen with a Repositories browser — grouped sidebar list plus a rich detail panel and bottom action bar (UI-IMPROVE §1.2).

**Design:** sidebar shows grouped recent repositories with subtle selection; main content is a detail card (name, Open/Delete, location, last opened, branch, changed-file count, remote URL, committer identity) or an empty “Select a repository” state. Load Git metadata asynchronously when a repo is selected. Bottom toolbar holds Add / Create / Clone. Drag-drop stays on the workspace root, de-emphasized visually.

**Files changed:**
- `apps/desktop/src/views/welcome.rs` — two-pane detail layout, empty state, action bar
- `apps/desktop/src/views/sidebar.rs` — grouped welcome repo list, no emoji icons
- `apps/desktop/src/app_state.rs` — `WelcomeRepoSnapshot` and selection snapshot fields
- `apps/desktop/src/main.rs` — snapshot load, remove-from-recents with confirmation, Open wiring
- `crates/app_core/src/lib.rs` — `RecentRepositoryStore::remove`

**Acceptance checks:** welcome shows detail panel for selected repo; empty state when none selected; Open opens repo; Delete removes from recents after confirm; Add/Create/Clone in bottom bar; drag-drop still works; all workspace gates pass; app launches.

## 2026-08-10 — Visual polish pass (Working Copy, toolbar, sidebar)

**Intent:** close the visual gap based on user screenshots — remove emoji clutter, harsh selection blue, and bulky commit/file-list chrome.

**Design:** use theme `selection` for list rows (dark text on subtle tint), compact commit composer (subject-only until body filled, option chips, disabled Commit state, staged count), segmented Modified/All Files control, text-only status badges, branch breadcrumb with ⎇ glyph, text-only sidebar nav, cleaner toolbar height and icons.

**Files changed:**
- `crates/ui_kit/src/theme.rs` — softer selection colors
- `apps/desktop/src/views/components.rs` — theme-aware status badges, disabled primary button, commit option chips
- `apps/desktop/src/views/commit_composer.rs` — compact composer
- `apps/desktop/src/views/working_copy.rs` — branch breadcrumb, file list rows/tabs, remove file-type emojis
- `apps/desktop/src/views/sidebar.rs` — text-only nav, muted badges
- `apps/desktop/src/views/toolbar.rs` — slimmer toolbar, cleaner icons
- `apps/desktop/src/views/welcome.rs` — updated primary button API, drop zone icon

**Acceptance checks:** no emoji icons in Working Copy rows/branch; selection uses theme tint; commit area compact; Modified/All Files segmented control; all workspace gates pass; app launches.


**Intent:** finish UI-IMPROVE §1.6 / §2.3 — expose the existing `worktree_show_all_files` mode with a Modified/All Files control and a single flat changed-file list in Modified mode.

**Design:** add a segmented header above the file list (`Modified` / `All Files`) that calls `toggle_worktree_show_all`. In Modified mode, render one flat list of changed entries (staged, unstaged, untracked, conflicts) with per-file stage checkboxes and status badges instead of separate Staged/Unstaged/Untracked/Conflicts sections. Show a meaningful empty state when there are no changes. Keep the All Files mode backed by `tracked_files`.

**Files changed:**
- `apps/desktop/src/views/working_copy.rs` — file list header toggle, flat modified list, empty state, removed legacy `status_group_view`
- `apps/desktop/src/main.rs` — expose `toggle_worktree_show_all` to views

**Acceptance checks:** Working Copy shows Modified/All Files toggle; Modified mode lists all changed files in one pane; All Files mode lists tracked + untracked files; toggle loads tracked files asynchronously; empty modified state is visible; all workspace gates pass.

**Verification:** rebuilt app launched as `target/debug/gitronimo-desktop`. All gates pass.

## 2026-08-10 — UI polish: sidebar, working copy, branch menu, toolbar

**Intent:** match the UI across all major views based on new screenshots (001-004) and the UI plan.

**Design:** 
- **Sidebar (001)**: Added "Repositories" and "Services" sections. Repositories section header; Services section shows connected account (GitHub/GitLab) or "Add Service" placeholder. Branch tree retains expand/collapse with "›"/"⌄" indicators.
- **Working Copy (004)**: Branch breadcrumb uses 🐘 (branch icon) + bold branch-name › tracking). Commit composer: subject field with accent border when filled, description field expands on content, action buttons (Description, Amend ✓/Amend, Sign-off ✓/Sign-off, Author, Commit primary). File list shows status badges (M/A/D/?), file icons, paths with ellipsis. Diff view retains Staged/Unstaged tabs, chunk info, hunk actions.
- **Branch Context Menu (003)**: Local Branch: Checkout, View History, Merge into Current…, Rebase Current onto…, Rename…, Delete. Remote Branch: New Branch from Here…, View History, Pull, Delete. Tag: New Branch from Here…, View History, Delete. Remote: Fetch. Sections separated by dividers.
- **Toolbar (002)**: Center shows repo name › branch › tracking (ahead ↑, behind ↓). Subtitle shows view + changed file count. Right side: Fetch/Pull/Push/Sync, Palette, Open.

**Files changed:**
- `apps/desktop/src/views/sidebar.rs` — Repositories/Services sections, ServiceAccount display
- `apps/desktop/src/views/working_copy.rs` — branch_context_view (branch icon), commit_composer_view (subject/body fields, action buttons), ref_context_menu_view (Pull for remote branches)
- `apps/desktop/src/views/commit_composer.rs` — subject/body fields with placeholder, accent borders, Amend/Sign-off toggle labels
- `apps/desktop/src/views/toolbar.rs` — branch_info with ahead/behind arrows, subtitle with changed count
- `apps/desktop/src/main.rs` — pull_branch method for remote branch pulling

**Acceptance checks:** sidebar shows Repositories/Services; working copy has branch breadcrumb, commit composer, file list, diff; branch right-click shows menu with Pull for remotes; toolbar shows branch sync status; all gates pass; app rebuilds and launches.

**Verification:** rebuilt app launched as `target/debug/gitronimo-desktop`. All gates pass.

## 2026-08-10 — Toolbar, navigation, context menu, column proportions

**Intent:** match the toolbar layout, back/forward navigation including return to repos list, branch context menu with full items, and 50/50 column split.

**Design:** add repo switcher icon on toolbar far left, add `return_to_welcome` method that clears state and returns to Welcome, set `came_from_welcome` flag when opening a repo from Welcome so Back button returns there. Rewrite `ref_context_menu_view` with items: Checkout, View History, separator, Merge into Current, Rebase Current onto, separator, Rename, Delete for local branches; New Branch from Here, View History, Delete for remote/tag; Fetch for remotes. Add `menu_item` (24px height, hover highlight) and `menu_separator` helpers. Add `request_branch_delete`, `merge_branch_into_current`, `rebase_current_onto`, `prompt_rename_branch` action methods. Balance column min-widths to 340px each.

**Files changed:**
- `apps/desktop/src/views/toolbar.rs` — repo switcher icon, removed clickable repo name div, too_many_lines allow
- `apps/desktop/src/app_state.rs` — `came_from_welcome: bool` field
- `apps/desktop/src/main.rs` — `return_to_welcome`, `request_branch_delete`, `merge_branch_into_current`, `rebase_current_onto`, `prompt_rename_branch`, `came_from_welcome` init, `begin_open_path` sets flag, `navigate_back` checks flag
- `apps/desktop/src/views/working_copy.rs` — full context menu with 7 items per local branch, `menu_item`/`menu_separator` helpers, balanced column widths

**Acceptance checks:** toolbar has repo switcher icon far left + back/forward + centered repo name + action icons far right; back button returns to repos list when opened from Welcome; branch right-click shows Checkout, View History, Merge, Rebase, Rename, Delete with separators; columns are 50/50; all gates pass; app rebuilds and launches.

**Verification:** rebuilt app launched as `target/debug/gitronimo-desktop`. All gates pass.

## 2026-08-10 — Button/spacing consistency pass

**Intent:** audit all button sizes, column widths, and spacing across every view to ensure visual consistency.

**Design:** standardize all action buttons to h=26px, px=2, text_sm, rounded(4px). Fix primary_window_action_button and window_action_button which used px=3 py=2 (~32px) and had border_1. Fix validated_action_button disabled variant which used px=2 py=1 (~18px). Fix diff hunk buttons from py_0.5 to py_1 for better readability. Add font_weight::MEDIUM to Staged/Unstaged tabs. Fix column header Status width from 36px to 44px to match row content (checkbox 14 + badge 14 + gap 8 + padding 8).

**Files changed:**
- `apps/desktop/src/views/components.rs` — `primary_window_action_button`, `window_action_button`, `validated_action_button` all standardized to h=26 px=2
- `apps/desktop/src/views/diff_viewer.rs` — hunk buttons py_0.5→py_1, tabs get font_weight MEDIUM
- `apps/desktop/src/views/working_copy.rs` — column header Status width 36→44px

**Acceptance checks:** all buttons in the same area are identical height/padding; column headers align with row content; no visual gaps between sections; all gates pass; app rebuilds and launches.

**Verification:** rebuilt app launched as `target/debug/gitronimo-desktop`. All gates pass.

## 2026-08-10 — Welcome page + Working Copy alignment

**Intent:** match the Welcome page (centered drop zone) and Working Copy (branch breadcrumb, inline chunk buttons).

**Design:** replace the Welcome repo-detail card with a centered dashed-border drop zone showing a folder icon and "Drop Folder or URL to Add Git Repository" with Add/Create/Clone buttons below. Simplify the welcome sidebar to use folder icons. Rewrite branch_context_view as a breadcrumb (branch > tracking). Move Discard Chunk / Stage Chunk buttons into each hunk header row inline (removing the separate controls row).

**Files changed:**
- `apps/desktop/src/views/welcome.rs` — rewritten to centered drop zone, free function `welcome_drop_zone`
- `apps/desktop/src/views/sidebar.rs` — folder icon `📁` for welcome repo list
- `apps/desktop/src/views/working_copy.rs` — branch breadcrumb, dead code suppression
- `apps/desktop/src/views/diff_viewer.rs` — inline Discard Chunk / Stage Chunk buttons per hunk, removed `hunk_controls_row`
- `apps/desktop/src/app_state.rs` — `#[allow(dead_code)]` on `repositories_grouped`
- `apps/desktop/src/main.rs` — `#[allow(dead_code)]` on 5 temporarily unused methods

**Acceptance checks:** Welcome page shows centered drop zone with folder icon and Add/Create/Clone buttons; sidebar shows folder icons for repos; Working Copy shows branch breadcrumb (branch > tracking); diff hunk headers have inline Discard/Stage Chunk buttons; all gates pass; app rebuilds and launches.

**Verification:** rebuilt app launched as `target/debug/gitronimo-desktop`. All gates pass.

## 2026-08-10 — Diff pane alignment (Staged/Unstaged tabs, chunk info)

**Intent:** add the diff pane header with filename, Staged/Unstaged tab toggle, chunk/insertion/deletion counts, and clean up clippy errors from the broader alignment pass.

**Design:** extract the diff header and selection controls into dedicated helpers, add tab toggle between Staged and Unstaged views, and count additions/deletions per diff. Fix all clippy warnings (`dead_code`, `too_many_lines`, `redundant_closure`, `too_many_arguments`, `map_or_else`).

**Files changed:**
- `apps/desktop/src/views/diff_viewer.rs` — Staged/Unstaged tabs, chunk info bar, filename header, extracted `diff_header`/`selection_controls`/`staged_unstaged_tabs` helpers, `px` import, `new_path` fix
- `apps/desktop/src/views/toolbar.rs` — `icon_toolbar_button` takes `disabled` flag instead of `.opacity(0.3)`, `#[allow(clippy::redundant_closure)]`
- `apps/desktop/src/views/working_copy.rs` — removed unused `count` variable, extracted `file_type_icon()` helper
- `apps/desktop/src/main.rs` — `#[allow(dead_code)]` on `prompt_fetch_remote`, `publish_current`, `request_force_with_lease`
- `apps/desktop/src/views/sidebar.rs` — `#[allow(clippy::too_many_arguments)]` on `nav_row_with_badge`

**Acceptance checks:** diff pane shows filename header with Staged/Unstaged tab toggle; chunk/insertion/deletion counts displayed; all clippy warnings resolved; `cargo fmt`, `cargo clippy -D warnings`, 100 tests, `cargo deny check` all pass; app rebuilds and launches.

**Verification:** rebuilt app launched as `target/debug/gitronimo-desktop`. All gates pass.

## 2026-08-09 — Repositories surface redesign

**Intent:** replace the remaining card-heavy welcome surface after comparing the running UI with Repositories screenshots.

**Design:** model the welcome state as a repository browser: a narrow left repository tree with compact grouped recents and bottom Add/Create/Clone actions, plus a wide right detail workspace for the selected repository. Remove the feature-card dashboard treatment, keep the existing original Gitronimo palette, and make repository selection visually distinct from opening.

**Files (planned):**
- `apps/desktop/src/app_state.rs` — selected recent repository state.
- `apps/desktop/src/main.rs` — selection/reset helpers and open-selected action.
- `apps/desktop/src/views/sidebar.rs` — repository-browser welcome sidebar.
- `apps/desktop/src/views/welcome.rs` — repository detail workspace and empty state.
- `docs/UI-IMPROVE.md`, `docs/work-log.md` — record the information-architecture change.

**Acceptance checks:** welcome renders as a repository browser rather than stacked feature cards; recents can be selected and opened; Add/Create/Clone remain available; empty/loading/error states remain clear; all workspace gates pass.

**Verification:** the welcome state now has a compact repository tree sidebar with selectable recents, double-click open behavior, and bottom Add/Create/Clone actions. The main pane shows the selected repository's path and open action instead of feature cards. Full formatting, Clippy, workspace tests, and cargo-deny gates pass; the rebuilt app is running as `target/debug/gitronimo-desktop` (PID 69259).

## 2026-08-09 — UI reference alignment / dense Working Copy shell

**Intent:** align Gitronimo's visual hierarchy with `docs/screens/t-vs-g.png` without copying third-party assets or branding.

**Design:** keep the current original palette, but make the shell denser and more intentional: compact toolbar chrome, a tighter content gutter, a sidebar with clear navigation grouping, and a horizontal Working Copy workspace with the file list as the primary pane and the selected diff as the adjacent detail pane. Branch/sync controls remain available but are visually subordinate to the commit-and-review workflow.

**Files (planned):**
- `apps/desktop/src/views/workspace.rs` — reduce shell padding and strengthen the content/inspector split.
- `apps/desktop/src/views/toolbar.rs` — compact repository toolbar hierarchy.
- `apps/desktop/src/views/sidebar.rs` — denser navigation grouping and active-state treatment.
- `apps/desktop/src/views/working_copy.rs` — horizontal file-list/diff layout and compact action grouping.
- `apps/desktop/src/views/components.rs` — shared compact control styling.
- `docs/UI-IMPROVE.md`, `docs/work-log.md` — record the visual alignment and its original-design boundary.

**Acceptance checks:** Working Copy presents files and diff side by side at normal desktop widths, the layout remains readable at the existing narrow-window fallback, toolbar/sidebar hierarchy is visibly denser, no third-party assets are added, and all workspace gates pass.

**Verification:** the Working Copy now uses a responsive wrapping two-pane review workspace, with the commit composer and file groups in the leading pane and the selected diff in the adjacent pane. Branch context is compact, sync actions are promoted to an icon-oriented toolbar, the commit area is a subject/action strip, and the former large Branch/Sync cards no longer push the review surface below the fold. Shell padding, sidebar glyphs/grouping, shared action buttons, section cards, and active destinations were tightened to match the compact professional reference without copying its assets or branding. Format, Clippy, workspace tests, and cargo-deny pass; the rebuilt app is running as `target/debug/gitronimo-desktop` (PID 22837).

**Follow-up verification:** the repository browser now replaces the remaining feature-card welcome dashboard, with a compact selectable repository tree and a selected-repository detail workspace. The latest rebuilt app is running as `target/debug/gitronimo-desktop` (PID 7141).

## 2026-08-09 — UI overhaul

**Intent:** ground-up redesign of the desktop shell to match the clean native macOS aesthetic, not just incremental polish.

**Design:**
- Sidebar: clean source-list navigation with compact rows, subtle headers, active-state highlighting, no heavy borders.
- Toolbar: compact unified bar with navigation, repository context, and action buttons.
- Working Copy: compact branch/tracking strip, dense commit subject/action area, clean file list with status icons and stage checkboxes, adjacent diff pane.
- Commit composer: compact subject field + commit button + options row.
- Components: rounded controls, consistent height, no card-based grouping.
- Welcome: compact repository browser with selectable recents and detail workspace.

**Files:**
- `apps/desktop/src/views/sidebar.rs` — source-list redesign.
- `apps/desktop/src/views/toolbar.rs` — compact unified toolbar.
- `apps/desktop/src/views/working_copy.rs` — Working Copy hierarchy.
- `apps/desktop/src/views/commit_composer.rs` — compact commit area.
- `apps/desktop/src/views/components.rs` — rounded compact controls.
- `apps/desktop/src/views/welcome.rs` — repository browser welcome.
- `docs/work-log.md` — this entry.

**Acceptance checks:** clean native-style chrome, dense file list with stage checkboxes, compact commit area, repository browser welcome, all gates pass.

**Verification:** full formatting, Clippy with warnings denied, workspace tests, and cargo-deny pass. The rebuilt app is running as `target/debug/gitronimo-desktop` (PID 71496).

## 2026-08-09 — visual alignment pass

**Intent:** close remaining visual gaps between Gitronimo and information architecture by adopting tighter density, flush sidebar-content surfaces, colored status badges, and a two-pane commit detail layout.

**Changes:**
- **workspace.rs** — removed `p_4` from content area so sidebar and content share a flush surface.
- **working_copy.rs** — restructured `file_review_workspace` to place the branch context, commit composer, and file list in the left column with the diff adjacent on the right; added a 1px divider between panes.
- **commit_composer.rs** — redesigned the commit area to show the subject field full-width at the top, the description inline when present, and the amend/sign-off/author/commit controls in a single bottom row.
- **sidebar.rs** — reorganized to match the Workspace/Branches structure; added Pull Requests, Reflog, and Settings nav rows; moved the change count into a badge on the Working Copy row; removed the separate Status section.
- **history.rs** — restructured into a two-pane layout with a compact toolbar row at the top, a scrollable commit list on the left, and the selected commit's metadata and changed-file summary on the right.
- **toolbar.rs** — reduced toolbar height and button sizes to match the compact icon-toolbar density.
- **components.rs** — added `status_badge_info` to map git status codes to colored badge characters and background/foreground colors.

**Acceptance checks:** flush sidebar-to-content surface, compact toolbar, colored status badges, two-pane commit detail, all gates pass.

**Verification:** formatting, Clippy with warnings denied, 100 workspace tests, and cargo-deny pass. The rebuilt app is running as `target/debug/gitronimo-desktop` (PID 31111).

## 2026-08-09 — UI-IMPROVE item 12 / Pull Requests

**Intent:** complete the Pull Requests collaboration surface after stabilizing the Services provider boundary.

**Design:** extend the provider-neutral hosting port with list/detail/create/comment/merge operations and explicit `MergeMethod` values. The GitHub adapter maps files, comments, state, rate limits, and API errors without exposing tokens. The desktop adds a split list/detail Pull Requests view, selected hosted-repository context, background stale-result guards, explicit merge confirmation/method choice, comment/create prompts, and checkout through a typed GitHub pull-ref fetch followed by local branch creation.

**Files (planned):**
- `crates/git_domain/src/lib.rs`, `crates/app_core/src/lib.rs` — PR models and hosting operations.
- `crates/hosting_github/src/lib.rs` — GitHub PR endpoints and fixture parser tests.
- `crates/git_cli/src/lib.rs` — typed pull-request ref fetch and local-bare integration test.
- `apps/desktop/src/app_state.rs`, `main.rs`, `views/pull_requests.rs`, `views/services.rs`, `views/mod.rs`, `views/working_copy.rs` — PR state, actions, and UI.
- `PLAN.md`, `docs/UI-IMPROVE.md`, `docs/work-log.md` — checklist/status updates.

**Acceptance checks:** open PRs list and load details; create/comment/merge use background provider calls; merge requires an explicit method and confirmation; checkout fetches `pull/<number>/head` through typed Git arguments; no public-network test or credential is used; full workspace gates pass.

**Verification:** GitHub fixture tests cover PR summaries, files, comments, HTTP response parsing, and provider repository parsing. A local bare repository test verifies typed pull-request ref fetching. The desktop exposes the split PR list/detail view and all requested actions without public-network tests or real credentials. Full formatting, Clippy, workspace tests, and cargo-deny gates pass.

## 2026-08-09 — UI-IMPROVE item 11 / Services vertical slice

**Intent:** implement the first real Services slice: GitHub account connection, secure token persistence, account/repository listing, and clone handoff to the existing Git CLI boundary.

**Design:** `git_domain` owns provider-neutral non-secret account and hosted-repository models. `app_core` owns `SecretStore` and `HostingService` ports. `platform_macos` implements the Keychain port through the macOS `security` tool without persisting tokens in app preferences or state. `hosting_github` owns GitHub API endpoints, response parsing, authentication/rate-limit mapping, and a typed curl transport; no UI or Git logic enters that crate. The desktop Services view stores only account metadata and hosted repositories, never the token.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — provider-neutral service/account/repository models.
- `crates/app_core/src/lib.rs` — `SecretStore`, `HostingService`, secret key, and hosting errors.
- `crates/platform_macos/` — Keychain-backed `SecretStore` implementation.
- `crates/hosting_github/` — GitHub API adapter and JSON/HTTP parser tests.
- `Cargo.toml`, `apps/desktop/Cargo.toml`, `Cargo.lock` — workspace and exact dependencies.
- `apps/desktop/src/app_state.rs`, `main.rs`, `views/services.rs`, `views/mod.rs`, `views/working_copy.rs`, `views/sidebar.rs` — Services state, background flows, and view.
- `docs/UI-IMPROVE.md`, `docs/work-log.md` — status and security notes.

**Acceptance checks:** token entry uses an obscured prompt and is stored only in Keychain; account validation and repository listing use a background GitHub request; rate-limit/auth errors become explicit UI states; hosted repository clone uses the existing typed Git boundary; parser and workspace gates pass without public-network tests.

**Verification:** `hosting_github` parses GitHub response headers and repository JSON through fixture tests; `platform_macos` scopes the Keychain item by provider and account without exposing a secret in models. The Services view connects, refreshes, signs out, reports expired/rate-limited states, lists repositories, and hands selected clones to `git_cli::clone_repository`. Full formatting, Clippy, workspace tests, and cargo-deny gates pass. No public GitHub request or real token was used during tests.

## 2026-08-09 — UI-IMPROVE item 10 / Repositories view

**Intent:** finish the Repositories welcome-surface item from `docs/UI-IMPROVE.md` before beginning provider-backed Services or Pull Requests.

**Design:** replace the welcome-only recent list with a Repositories surface that groups local recents by parent folder when enabled, keeps a flat recent view when disabled, exposes Add existing / Create new / Clone entry points, and shows the current open state. Create new initializes a selected folder through the typed Git CLI boundary, then opens the resulting repository. Clone prompts for a URL or local path and destination parent, runs the typed Git clone boundary, and opens the resulting repository.

**Files (planned):**
- `crates/git_cli/src/lib.rs` — typed `init_repository` mutation and temporary-repository integration test.
- `apps/desktop/src/app_state.rs` — repository grouping state.
- `apps/desktop/src/main.rs` — create-repository folder flow and palette/navigation wiring.
- `apps/desktop/src/views/welcome.rs` — Repositories surface, grouping toggle, create/add/clone actions, and recent metadata.
- `docs/UI-IMPROVE.md`, `docs/work-log.md` — record completion and clone behavior.

**Acceptance checks:** recents render grouped and flat; grouping toggles without losing recents; Create new initializes and opens a repository; Add existing still opens a selected repository; Clone completes for a local source and opens the result; parser/mutation and workspace gates pass.

**Verification:** local clone and initialization integration tests pass, including canonical path handling. The Repositories view now renders grouped or flat recents and exposes Add existing, Create new, and Clone. The complete workspace gates pass.

## 2026-08-09 — Phase 8 / Git LFS status

**Intent:** implement the remaining feasible Phase 8 checklist item, `Git LFS status`, without adding a new dependency or treating LFS as a repository mutation.

**Design:** `git_cli` runs `git lfs status --porcelain` with typed arguments and parses each status line into a `LfsEntry` containing the index/worktree status bytes and raw path. The desktop adds an LFS view reachable from the sidebar and command palette, with refresh, status explanations, and the empty state for repositories without changed LFS files. A temporary repository integration test uses the installed Git LFS executable and verifies a modified tracked LFS object is reported.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `LfsEntry`.
- `crates/git_cli/src/lib.rs` — `lfs_status`, porcelain parser, parser test, and temporary-repository integration test.
- `apps/desktop/src/app_state.rs` — `RepositoryView::Lfs`, LFS entries, and load token.
- `apps/desktop/src/main.rs` — LFS loading and palette/navigation wiring.
- `apps/desktop/src/views/lfs.rs`, `views/mod.rs`, `views/working_copy.rs`, `views/sidebar.rs` — LFS view and navigation.

**Acceptance checks:** the parser preserves raw paths and status columns; a real temporary repository reports a modified LFS file; the view loads in the background and shows loading, empty, success, and failure states; all required workspace gates pass.

**Verification:** `git lfs status --porcelain` is parsed without a new dependency, preserving both status columns and path bytes. Parser tests cover valid raw paths and malformed records; a temporary repository with Git LFS 3.5.1 verifies a modified tracked LFS object. The desktop exposes Git LFS from the sidebar and command palette, with background loading and an empty state. The full format, Clippy, workspace test, and cargo-deny gates pass.

## 2026-08-09 — UI-IMPROVE "now" items (remaining)

**Intent:** finish the remaining `docs/UI-IMPROVE.md` §3 "now" items. Items 2 (Modified-only / All-files toggle), 3 (per-file stage checkbox), 4 (Back/Forward toolbar buttons), 5 (reveal new commit in History after commit), and 8 (remote-activity sidebar footer) were already implemented; this unit covers items 1, 6, 7, and 9.

**Design:**
- Item 1 — move the commit composer so it sits directly above the Working Copy file groups.
- Items 6 + 7 — a first-class `Commit Detail` view reached by double-clicking a History row, with a `Changeset / Tree` mode toggle: Changeset shows the commit metadata, changed-file list, and read-only diff; Tree reuses `git ls-tree` browsing at that commit.
- Item 9 — add `Stashes` and `Remotes` as sidebar destinations with views. `git_cli` gains `stash_list` (NUL-field / 0x1e-record parsing of `git stash list`) plus reference-parameterized `apply_stash`/`pop_stash`/`drop_stash`; the existing "latest stash" helpers delegate to them. The Remotes view lists each remote's name + fetch URL with a per-remote Fetch action.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `StashEntry { reference, oid, subject }`.
- `crates/git_cli/src/lib.rs` — `stash_list`, `apply_stash`, `pop_stash`, `drop_stash`, `parse_stash_records`, `GitStatusError::ParseStash`, unit parser test + temporary-repository integration test.
- `apps/desktop/src/app_state.rs` — `RepositoryView::CommitDetail`/`Stashes`/`Remotes`, `HistoryDetailMode`, `history_detail_mode`, `stashes`, `selected_stash`, `pending_stash_action_ref`.
- `apps/desktop/src/main.rs` — `show_commit_detail`, `open_commit_detail_from_history`, `toggle_history_detail_mode`, `show_stashes`/`load_stashes`, `show_remotes`, stash apply/pop/drop by reference with confirmation, palette entries, constructor/reset wiring.
- `apps/desktop/src/views/commit_detail.rs`, `views/stashes.rs`, `views/remotes.rs` — the three views.
- `apps/desktop/src/views/working_copy.rs` — route new `RepositoryView` variants; move composer above file groups.
- `apps/desktop/src/views/history.rs` — double-click opens Commit Detail.
- `apps/desktop/src/views/sidebar.rs` — Working Copy / History / Stashes / Remotes nav destinations with badges.
- `PLAN.md`, `docs/work-log.md` — no PLAN.md checkboxes are owned by UI-IMPROVE items; work-log only.

**Acceptance checks:**
- The commit composer renders directly above the file list in Working Copy.
- Double-clicking a History row opens Commit Detail; the Changeset/Tree toggle switches between changed-files+diff and the commit tree.
- The Stashes view lists stashes and applies/pops/drops a selected stash (pop/drop require confirmation); Remotes lists remotes and fetches a selected one.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the commit composer now sits immediately before the Working Copy file groups. History rows open `Commit Detail` on a double click; its Changeset mode shows metadata, changed paths, and a read-only diff, while Tree mode loads the selected commit through the existing `ls-tree` boundary. The sidebar and command palette expose Stashes and Remotes; stash parsing and reference-specific apply/pop/drop are covered by two parser tests plus a temporary-repository integration test. Final gates pass: formatting, Clippy with warnings denied, 90 workspace tests, and cargo-deny. The rebuilt app is running as `target/debug/gitronimo-desktop` (PID 94254); startup emitted no runtime error.



**Intent:** prepare professional-UI guidance and enable GPUI skills in opencode.

**Design:** the user relaxed the third-party-asset rule; `AGENTS.md` now forbids copying third-party icons/glyphs/design assets into shipped code but permits attributed screenshots under `docs/` for UI/UX reference (never shipped). `PLAN.md` §28 risk updated to match. `PLAN.md` was also edited by the user to drop GitLab/Bitbucket/Azure DevOps auth checkboxes (GitHub only) and to mention OpenCode alongside Codex in the execution protocol.

**Files:**
- `.opencode/opencode.json` — registers `./skills` (11 Apache-2.0 GPUI skills from Zed) as opencode project skills.
- `AGENTS.md`, `PLAN.md` — asset/attribution policy update.
- `docs/UI-IMPROVE.md` — UI/UX improvement plan for GitRonimo views.
- `docs/screens/` — optional product screenshot inventory.

**Acceptance checks:** the skills load after opencode restart; every screenshot is attributed in `docs/UI-IMPROVE.md`; no assets are added to the shipped app. Note: the model cannot read images, so `docs/screens/*.png` (the user's own captures) and the downloaded guides could not be visually verified; the plan is grounded in the guides' text.



**Intent:** implement the `Signed-commit status` checklist item by exposing Git's `%G?` signature validation per commit.

**Design:** `git_domain` gains `CommitSignatureStatus` (good/bad/unknown/none/expired/good-expired/revoked/error/other with a display label) and `CommitSignature` (status + signer). `git_cli::commit_signature` runs `git show --no-patch --format=%G?%x00%GS <oid>` and parses the two fields. The desktop adds a palette action `Check commit signature…` that prompts for a ref (defaulting to the selected history commit, else `HEAD`) and reports the verdict and signer in the activity line. `GitExecutable::run_env` is generalized from `&'static str` pairs to owned `OsString` pairs so the integration test can sign with a temporary `GNUPGHOME`.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `CommitSignatureStatus`, `CommitSignature`.
- `crates/git_cli/src/lib.rs` — `commit_signature`, parser, and a unit test of the parser plus a temporary-repository integration test using a real gpg key fixture; `run_env` takes `(OsString, OsString)`.
- `apps/desktop/src/main.rs` — `prompt_check_commit_signature`, palette entry.
- `PLAN.md`, `docs/work-log.md` — mark `Signed-commit status` complete.

**Acceptance checks:**
- `commit_signature` reports the `%G?` verdict and signer for a commit.
- The desktop prompts for a ref and shows the verdict.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the full gates pass after this unit. `git_cli::commit_signature` queries `%G?`/`%GS` with a NUL separator; the parser unit test covers every verdict letter plus an unknown one, and a temporary-repository integration test first asserts an unsigned commit reports `None`, then (when `gpg` is available) generates a throwaway key in a `chmod 700` temp homedir, signs a commit with `GNUPGHOME` set via the generalized `run_env`, and asserts the parsed verdict is `Good` with the fixture identity as signer. The desktop adds `Check commit signature…` to the palette, defaulting the prompt to the selected history commit's oid. `Signed-commit status` is now checked in PLAN.md.

## 2026-08-09 — Phase 8 / external merge-tool integration

**Intent:** implement the `External merge-tool integration` checklist item by configuring a named merge tool and launching it on conflicted files.

**Design:** `git_cli` gains `set_merge_tool` (`git config merge.tool <tool>`, with `mergetool.keepBackup false`) and `run_merge_tool` (`git mergetool --no-prompt [-t <tool>] [-- <path>]`). The desktop adds palette actions `Set merge tool…` (choose FileMerge/Meld/KDiff3/VimDiff via osascript) and `Open in merge tool…` (prompt a path, or run for all conflicts when blank), launched on a background task so the window stays responsive while the GUI tool runs.

**Files (planned):**
- `crates/git_cli/src/lib.rs` — `set_merge_tool`, `run_merge_tool`, and a temporary-repository integration test using a no-op tool.
- `apps/desktop/src/main.rs` — `prompt_set_merge_tool`, `prompt_run_merge_tool`, palette entries.
- `PLAN.md`, `docs/work-log.md` — mark `External merge-tool integration` complete.

**Acceptance checks:**
- `set_merge_tool` persists `merge.tool` in the repository config.
- `run_merge_tool` invokes the tool on a conflicted file and exits cleanly.
- The desktop drives tool selection and launch from the palette.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the full gates pass after this unit. `git_cli::set_merge_tool` writes `merge.tool` and disables `mergetool.keepBackup`; `run_merge_tool` invokes `git mergetool --no-prompt` with an optional tool and path. A temporary-repository integration test creates a merge conflict, persists `merge.tool=noop`, registers a trusted no-op tool command, asserts `git config merge.tool` reads back the name, runs `run_merge_tool` on the conflicted file, and verifies the file is staged (no `Unmerged` entry) with no `.orig` backup left behind. The desktop adds `Set merge tool…` (osascript tool picker: opendiff/meld/kdiff3/vimdiff/bc3) and `Open in merge tool…` (optional path, launched on a background task so the GUI tool never blocks the window). `External merge-tool integration` is now checked in PLAN.md. Workspace suite totals 83 tests (48 git_cli, 17 desktop, 8 app_core, 8 git_domain, 2 ui_kit).

## 2026-08-09 — Phase 8 / conflict-resolution UI

**Intent:** implement the `Conflict-resolution UI` checklist item with a typed resolve boundary and a dedicated desktop view.

**Design:** `git_domain` gains `ConflictSide` (Ours/Theirs). `git_cli::resolve_conflict` runs `git checkout --ours|--theirs -- <path>` followed by `git add -- <path>` to mark the file resolved, and `read_working_file` reads the raw working-tree copy (conflict markers included) for display. The desktop gains a Conflicts view listing the conflicted entries from status, each with `Take ours`, `Take theirs`, and `View` (marker content shown in a Monaco block), plus Refresh; the existing working-copy Continue/Abort flow completes the operation after files are resolved. All resolve actions run through `run_worktree_mutation` so the working copy refreshes and the conflict disappears.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `ConflictSide`.
- `crates/git_cli/src/lib.rs` — `resolve_conflict`, `read_working_file`, and a temporary-repository integration test.
- `apps/desktop/src/app_state.rs` — `RepositoryView::Conflicts`, conflict state.
- `apps/desktop/src/main.rs` — `show_conflicts`, `view_conflict`, `resolve_conflict`, palette entry.
- `apps/desktop/src/views/conflicts.rs` — the view.
- `PLAN.md`, `docs/work-log.md` — mark `Conflict-resolution UI` complete.

**Acceptance checks:**
- `resolve_conflict` keeps the chosen side's content and stages the file so status shows no conflict.
- The Conflicts view lists unmerged files and drives Take ours/theirs and View.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the full gates pass after this unit. `git_cli::resolve_conflict` checks out the requested side (`--ours`/`--theirs`) and stages the file; `read_working_file` exposes the marker content. A temporary-repository integration test creates a real merge conflict, asserts the working copy contains `<<<<<<<` markers, resolves to `--ours`, verifies the current-branch content survives and status reports no unmerged entry, then continues the merge. The desktop adds the Conflicts view (`Take ours`, `Take theirs`, `View` with a Monaco marker preview) reachable from the palette. `Conflict-resolution UI` is now checked in PLAN.md. Workspace suite totals 82 tests (47 git_cli, 17 desktop, 8 app_core, 8 git_domain, 2 ui_kit).

## 2026-08-09 — Phase 8 / squash, fixup, reword, and drop

**Intent:** implement the `Squash/fixup/reword/drop` checklist item as typed Git mutations plus command-palette actions.

**Design:** `git_cli` gains `autosquash(repository, target, message)`: it commits staged changes with `git commit --squash=<target> -m <message>` (or `--fixup=<target>` when no message is given, which discards the new commit's message) and then folds them in with `git rebase --autosquash --interactive <target>^` under a no-op sequence editor. It also gains `drop_commit(repository, target)` implemented as `git rebase --onto <target>^ <target>`, which replays the branch without the targeted commit. Reword reuses the existing `commit(CommitRequest { amend: true, .. })` path. The desktop adds four command-palette entries: `Squash staged changes…`, `Fixup staged changes…` (target defaults to `HEAD`), `Drop commit…` (free-form oid/ref), and `Reword last commit…` (amend prompt), all run through `run_worktree_mutation` so the working copy and refs refresh on success.

**Files (planned):**
- `crates/git_cli/src/lib.rs` — `autosquash`, `drop_commit`, and temporary-repository integration tests.
- `apps/desktop/src/main.rs` — `prompt_autosquash`, `prompt_drop_commit`, `prompt_reword`, palette entries.
- `PLAN.md`, `docs/work-log.md` — mark `Squash/fixup/reword/drop` complete.

**Acceptance checks:**
- `autosquash` folds staged changes into the target commit, keeping the target subject for fixup and combining the message for squash.
- `drop_commit` removes the targeted commit while keeping later commits.
- Reword amends HEAD with a new subject/body.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the full gates pass after this unit. `git_cli::autosquash` resolves `target` to a full oid first (so the fixup commit's HEAD move cannot break the base), commits staged changes with `--squash`/`--fixup` under `GIT_EDITOR=true`, then folds with `git rebase --autosquash --interactive <target>^` under a no-op sequence editor; `drop_commit` runs `git rebase --onto <target>^ <target>`. A temporary-repository integration test stages a change and verifies a fixup folds into HEAD keeping the subject, a squash folds in keeping the subject while combining the "squash note" body, and a `drop_commit("HEAD~1")` removes the middle commit (its file disappears) while the tip and base survive. Reword reuses the existing `commit(CommitRequest { amend: true, .. })`. The desktop adds palette actions `Squash staged changes…`, `Fixup staged changes…` (target prompt defaulting to `HEAD`), `Drop commit…`, and `Reword last commit…` (subject/body prompts). `Squash/fixup/reword/drop` is now checked in PLAN.md. Workspace suite totals 81 tests (46 git_cli, 17 desktop, 8 app_core, 8 git_domain, 2 ui_kit).

## 2026-08-09 — Phase 8 / interactive rebase plan editor

**Intent:** implement the `Interactive rebase plan editor` checklist item as a typed Git boundary plus a desktop view that edits and saves the rebase todo.

**Design:** `git_domain` gains `RebaseAction` (pick/reword/edit/squash/fixup/drop/exec/other with a `next_action` cycler for the UI) and `RebaseTodoItem` carrying the action and its verbatim argument tail. `git_cli` gains `start_rebase` (`git rebase --interactive <base>` with `GIT_SEQUENCE_EDITOR=:` and `GIT_EDITOR=true` so the default plan is generated and applied without spawning an editor), `rebase_plan` (reads `.git/rebase-merge/git-rebase-todo`, the same location the in-progress detector already uses), `save_rebase_plan` (writes the todo verbatim from edited items), and `rebase_abort`/`rebase_skip` for the paused states; `continue_operation(Rebase)` already exists. The desktop gains a Rebase view listing the plan with per-item action cycling and move-up/move-down, plus Save plan / Continue / Abort / Skip / Start rebase actions.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `RebaseAction`, `RebaseTodoItem`.
- `crates/git_cli/src/lib.rs` — `start_rebase`, `rebase_plan`, `save_rebase_plan`, `rebase_abort`, `rebase_skip`, a `parse_rebase_todo` helper, and unit + temporary-repository integration tests (conflict pause, plan rewrite to squash, continue).
- `apps/desktop/src/app_state.rs` — `RepositoryView::Rebase`, rebase state.
- `apps/desktop/src/main.rs` — `show_rebase`, `load_rebase_plan`, `prompt_start_rebase`, `save/continue/abort/skip` wiring, command-palette entry.
- `apps/desktop/src/views/rebase.rs` — the view.
- `PLAN.md`, `docs/work-log.md` — mark `Interactive rebase plan editor` complete.

**Acceptance checks:**
- `rebase_plan` parses a paused interactive rebase's todo; `save_rebase_plan` rewrites it and `git rebase --continue` applies the new plan.
- The Rebase view lists items, cycles actions, reorders rows, and drives Start/Save/Continue/Abort/Skip.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the full gates pass after this unit. `git_domain` adds `RebaseAction` (with `verb`, `next` cycle, and `from_verb`) and `RebaseTodoItem`; `git_cli` adds `start_rebase` (interactive rebase with a no-op sequence editor), `rebase_plan`/`save_rebase_plan` (read/write `.git/rebase-merge/git-rebase-todo`), `rebase_abort`, `rebase_skip`, and `parse_rebase_todo`. A unit test round-trips a three-line todo (skipping comments); an integration test starts a conflicting rebase, confirms `rebase_plan` exposes the remaining todo (git moves the paused patch into `done`, matching `--edit-todo` semantics), rewrites `pick` to `fixup`, resolves the conflict, continues, and verifies the final history folds both feature commits into one (base + main conflict + squashed feature = 3 commits, HEAD "feature one"). The desktop gains a Rebase view with action cycling, move up/down, Save plan, Start rebase…, Continue, Abort, and Skip. `Interactive rebase plan editor` is now checked in PLAN.md. Workspace suite totals 80 tests (45 git_cli, 17 desktop, 8 app_core, 8 git_domain, 2 ui_kit).

## 2026-08-09 — Phase 8 / submodule status, update, and open

**Intent:** implement the `Submodule status/update/open` checklist item as a typed Git boundary with a desktop view.

**Design:** `git_domain` gains `SubmoduleEntry` (`path`, status flag, `oid`, `description`). `git_cli::submodule_list` runs `git submodule status` and parses the per-line `<flag> <oid> <path> (<describe>)` format (`-` uninitialized, `+` checked-out differs from the index, `U` conflict, ` ` clean); `submodule_update` runs `git submodule update --init` with an optional path argument. The desktop gains a Submodules view listing entries with status, an Update-all action, and an Open action that reveals the submodule directory in Finder (`open <abs path>`, macOS-only, via typed `Command`).

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `SubmoduleEntry`.
- `crates/git_cli/src/lib.rs` — `submodule_list`, `submodule_update`, a `parse_submodule_status` helper, and a temporary-repository integration test using a local submodule fixture.
- `apps/desktop/src/app_state.rs` — `RepositoryView::Submodules`, submodule state.
- `apps/desktop/src/main.rs` — `show_submodules`, `load_submodules`, `prompt_submodule_update`, `prompt_open_submodule`, command-palette entry.
- `apps/desktop/src/views/submodules.rs` — the view.
- `PLAN.md`, `docs/work-log.md` — mark `Submodule status/update/open` complete.

**Acceptance checks:**
- `submodule_list` reports each submodule with status flag, oid, and path; update initializes a submodule from its remote.
- The Submodules view lists entries and drives Update/Open from prompts.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the full gates pass after this unit. `git_cli::submodule_list` parses `git submodule status` lines into `SubmoduleEntry` (flag, oid, path, description) and `submodule_update` initializes a submodule from its remote; a temporary-repository integration test registers a local `file://` submodule (using `-c protocol.file.allow=always`), confirms the entry parses as clean/uninitialized, then initializes it via `submodule_update` and re-checks that every submodule is clean. The desktop gains a Submodules view (flag glyph, path, state, oid, per-row Update…/Open buttons plus Update all…) driven through a confirmation prompt and a macOS `open` command resolved against the worktree root. `Submodule status/update/open` is now checked in PLAN.md. Workspace suite totals 78 tests (43 git_cli, 17 desktop, 8 app_core, 8 git_domain, 2 ui_kit).

## 2026-08-09 — Phase 8 / worktree list, create, and remove

**Intent:** implement the `Worktree list/create/remove` checklist item as a typed Git boundary with a desktop view.

**Design:** `git_domain` gains `WorktreeEntry` (`path`, `head` oid, optional `branch`, `dirty`, `main`). `git_cli::worktree_list` runs `git worktree list --porcelain` and parses the `worktree/HEAD/branch/detached` blocks; `add_worktree` runs `git worktree add <path> -b <branch>` and `remove_worktree` runs `git worktree remove <path>` with an explicit force opt-in, both as typed arguments. The desktop gains a `Worktrees` view listing entries with Add (path + branch name prompts) and Remove (path prompt) actions behind the command palette.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `WorktreeEntry`.
- `crates/git_cli/src/lib.rs` — `worktree_list`, `add_worktree`, `remove_worktree`, a `parse_worktree_list` helper, and temporary-repository integration tests.
- `apps/desktop/src/app_state.rs` — `RepositoryView::Worktrees`, worktree state.
- `apps/desktop/src/main.rs` — `show_worktrees`, `load_worktrees`, `prompt_add_worktree`, `prompt_remove_worktree`, command-palette entry.
- `apps/desktop/src/views/worktrees.rs` — the view.
- `PLAN.md`, `docs/work-log.md` — mark `Worktree list/create/remove` complete.

**Verification:** the full gates pass after this unit. `git_cli::worktree_list` parses `git worktree list --porcelain` blocks (path, HEAD oid, branch or detached, main flag) and flags the main worktree dirty from `git status --porcelain`; `add_worktree` creates a linked worktree with a new branch and `remove_worktree` removes it, proven by a temporary-repository integration test (1 → 2 → 1 worktrees with the expected branch); the desktop gains a Worktrees view with Add/Remove prompts and a shared `run_worktree_mutation` lifecycle. `Worktree list/create/remove` is now checked in PLAN.md. Workspace suite totals 77 tests (42 git_cli, 17 desktop, 8 app_core, 8 git_domain, 2 ui_kit).

## 2026-08-09 — Phase 8 / compare refs, browse tree at commit, and export file at revision

**Intent:** implement three related commit-inspection checklist items — `Compare refs`, `Browse tree at commit`, and `Export file at revision` — as typed Git boundaries with desktop views.

**Design:** `git_cli::diff_refs` runs `git diff --no-ext-diff --no-textconv --binary <left> <right>` and parses through the existing `parse_unified_diff` into a `LoadedDiff`; the desktop Compare view renders it read-only in monospace. `git_domain` gains `TreeEntry` (`name`, `kind: Tree/Blob/Commit`, `oid`, `mode`); `git_cli::tree_entries` runs `git ls-tree -z <oid>` and `git_cli::file_at_revision` runs `git show <oid>:<path>`, both bounded by the existing output reader. The desktop Tree view lists top-level entries, drills into subdirectories by accumulating path segments, shows blob content when a file is selected, and `Export file at revision…` saves the current blob to a user-chosen folder (osascript `choose folder`) through `std::fs` after the bytes return from Git. Git refs/oids are passed as typed arguments, never shell strings.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `TreeEntry`, `TreeEntryKind`.
- `crates/git_cli/src/lib.rs` — `diff_refs`, `tree_entries`, `file_at_revision`, and temporary-repository integration tests.
- `apps/desktop/src/app_state.rs` — `RepositoryView::Compare`, `RepositoryView::Tree`, compare/tree/file state.
- `apps/desktop/src/main.rs` — `prompt_compare_refs`, `load_compare`, `prompt_browse_tree`, `load_tree`, `load_tree_blob`, `export_selected_blob`, command-palette entries.
- `apps/desktop/src/views/compare.rs`, `apps/desktop/src/views/tree.rs` — the two views.
- `PLAN.md`, `docs/work-log.md` — mark `Compare refs`, `Browse tree at commit`, and `Export file at revision` complete.

**Verification:** the full gates pass after this unit. `git_cli::diff_refs` parses a two-ref diff through `parse_unified_diff`; `git_cli::tree_entries` parses `git ls-tree -z` (including `<oid>:<subdir>` drill-down) and `git_cli::file_at_revision` reads blob bytes via `git show <oid>:<path>`, all proven by a temporary-repository integration test (changed-file presence in the diff, a `dir` tree entry at the root, and the correct bytes for HEAD vs. the parent revision); the desktop gains read-only Compare and drill-down Tree views (with blob preview and folder-based export) reachable from the command palette. `Compare refs`, `Browse tree at commit`, and `Export file at revision` are now checked in PLAN.md. Workspace suite totals 76 tests (41 git_cli, 17 desktop, 8 app_core, 8 git_domain, 2 ui_kit).

## 2026-08-09 — Phase 8 / file history and blame

**Intent:** implement the next two Phase 8 checklist items — `File history` and `Blame` — as typed Git boundaries with desktop views, following the reflog unit's pattern.

**Design:** `git_domain` gains `FileHistoryRequest { path, limit }` and `BlameLine { oid, author, content }`; both reuse the existing `HistoryCommit` and `CommitIdentity` types. `git_cli::file_history` runs `git log --no-decorate --follow --max-count=N --format=%H%x00%P%x00%an%x00%ae%x00%at%x00%cn%x00%ce%x00%ct%x00%s%x00%b%x1e -- <path>` and parses through the existing `parse_history_records` helper. `git_cli::blame` runs `git blame --line-porcelain -- <path>`; a new `parse_blame` helper walks records (headers between `\t`-prefixed content lines), extracting the source oid, `author`, `author-mail`, and `author-time` for each line. The desktop gains a `views/file_history.rs` (bounded commit list for a prompted path) and a `views/blame.rs` (line list with source oid and author), both reachable from the command palette through an osascript path prompt.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `FileHistoryRequest`, `BlameLine`.
- `crates/git_cli/src/lib.rs` — `file_history`, `blame`, `parse_blame`, and temporary-repository integration tests for both.
- `apps/desktop/src/app_state.rs` — `RepositoryView::FileHistory`, `RepositoryView::Blame`, state for the loaded entries.
- `apps/desktop/src/main.rs` — `show_file_history`, `load_file_history`, `show_blame`, `load_blame`, command-palette entries.
- `apps/desktop/src/views/file_history.rs`, `apps/desktop/src/views/blame.rs` — the two views.
- `PLAN.md`, `docs/work-log.md` — mark `File history` and `Blame` complete.

**Verification:** the full gates pass after this unit. `git_cli::file_history` reuses `parse_history_records` over `git log --follow`, `git_cli::blame` parses `--line-porcelain` with a new `parse_blame` record walker, both proven by temporary-repository integration tests (newest-first file history with a missing-path empty result, and line attribution to the introducing commit); the desktop gains `FileHistory` and `Blame` views reachable from the command palette with path prompts, plus a Blame↔File-history shortcut. `File history` and `Blame` are now checked in PLAN.md. Workspace suite totals 75 tests (40 git_cli, 17 desktop, 8 app_core, 8 git_domain, 2 ui_kit).

## 2026-08-09 — Phase 8 / reflog and restore lost branch

**Intent:** implement the first self-contained Phase 8 unit: read a bounded HEAD reflog in the desktop and restore a deleted branch by recreating it at a reflog entry's commit. Covers the `Reflog` and `Restore lost branch from reflog` checklist items.

**Design:** `git_domain` gains pure `ReflogRequest` (`reference: Option<String>`, `limit`) and `ReflogEntry` (`old_oid`, `new_oid`, `selector` like `HEAD@{2}`, `identity`, `subject`) types. `git_cli::reflog` shells out to `git reflog --max-count=N --format=%H%x00%gD%x00%gs%x00%cn%x00%ce%x00%ct%x1e` (NUL-separated fields, 0x1e-separated records, mirroring the history parser), parses records with a new `parse_reflog_records` helper, then derives each entry's `old_oid` from the following entry's `new_oid` — exact because git's reflog chain records the ref value before and after each action. `git_cli::restore_branch_from_reflog` recreates the branch with `git branch <name> <oid>`, letting Git validate the name and refuse an existing branch. The desktop adds a `RepositoryView::Reflog` view (`reflog` list state, `selected_reflog` selection, `show_reflog`/`load_reflog` background load with the same stale-load token guard as history), routes `up`/`down` selection there, exposes it via the command palette and a `reflog_view`, and restores a branch from the selected entry by prompting for a name (reusing the osascript dialog pattern) before running the typed mutation and refreshing refs. A relative-time helper renders entry age without a new dependency. Deleting a branch removes its own reflog, so recovery targets the commit recorded in HEAD's reflog while the branch was checked out.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `ReflogRequest`, `ReflogEntry`.
- `crates/git_cli/src/lib.rs` — `reflog`, `restore_branch_from_reflog`, `parse_reflog_records`, `GitStatusError::ParseReflog`, and a temporary-repository integration test covering chained old-oids and restoring a deleted branch.
- `apps/desktop/src/app_state.rs` — `RepositoryView::Reflog`, `reflog`, `reflog_load_token`, `selected_reflog`.
- `apps/desktop/src/main.rs` — `show_reflog`, `load_reflog`, `move_reflog_selection`, `prompt_restore_branch_from_reflog`, `restore_branch_from_reflog`; reflog routing in the `up`/`down` handlers and command palette.
- `apps/desktop/src/views/reflog.rs` — `reflog_view`; dispatch in `working_copy.rs`.
- `apps/desktop/src/tests.rs` — a test that reflog selection moves and clamps within the loaded entries.
- `PLAN.md`, `docs/work-log.md` — mark the `Reflog` and `Restore lost branch from reflog` items complete.

**Acceptance checks:**
- `reflog` returns newest-first entries with `%gD` selectors and committer timestamps, each chaining its `old_oid` to the newer entry's oid; a deleted branch's commit still appears in the HEAD reflog.
- `restore_branch_from_reflog` recreates the branch at the selected oid and is refused by Git for an existing name.
- The Reflog view loads in the background, `up`/`down` move and clamp the selection, and Restore prompts for a name, runs the mutation, and refreshes refs on success.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the full gates pass after this unit. `git_domain` adds the pure reflog types; `git_cli::reflog` parses `%H/%gD/%gs/%cn/%ce/%ct` records (NUL fields, 0x1e records), derives exact `old_oid` chains, and `restore_branch_from_reflog` recreates a deleted branch at its reflog commit while Git refuses an existing name, all proven by a temporary-repository integration test; the desktop gains the `Reflog` view reachable from the command palette with `up`/`down` selection and a Restore action that prompts for a branch name before running the typed mutation. `Reflog` and `Restore lost branch from reflog` are now checked in PLAN.md. Workspace suite totals 73 tests (38 git_cli, 17 desktop, 8 app_core, 8 git_domain, 2 ui_kit), and the git_cli suite is stable across three repeated runs.

## 2026-08-08 — Phase 7 / recovery journal, conflict overview, and group completion

**Intent:** close the two remaining Phase 7 gaps and mark the whole group complete: record pre-operation refs for every history-changing operation the app performs, surface a conflict overview in the operation banner, and flip every Phase 7 checkbox and exit criterion.

**Design:** `git_domain` gains pure `RecoveryRecord` (`old_head`, `head_name`, `branch_tips`) and `RecoveredBranchTip` types; `git_cli::recovery_snapshot` reads HEAD's oid (`rev-parse HEAD`), the symbolic branch (`symbolic-ref --quiet HEAD`), and every local branch tip (`for-each-ref refs/heads --format=%(refname) %(objectname)`, safe because Git ref names cannot contain spaces) — the refs a merge, cherry-pick, revert, rebase, or abort/continue can move. `app_core` gains a versioned, bounded `RecoveryJournalStore` (20 entries, atomic temp+rename write, corrupt files quarantined under the same policy as preferences) persisted at `~/Library/Application Support/Gitronimo/recovery-journal.json`. The desktop captures the snapshot before every confirmed abort/continue (the only history-changing operations the UI currently runs; while a merge/cherry-pick/revert/rebase is paused, HEAD still holds the true pre-operation refs), records it in the journal, and only runs the mutation when the snapshot succeeds; a journal write failure is surfaced in the activity area without blocking the operation. The operation banner now includes an `operation_conflict_overview` line naming the conflicted-file count and the next step, completing the conflict-overview item.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `RecoveryRecord`, `RecoveredBranchTip`; `serde` dependency and derives on those types and `GitPath`.
- `crates/git_cli/src/lib.rs` — `recovery_snapshot`, `trim_oid`, and a temporary-repository integration test proving the recorded refs match the pre-merge state after HEAD moves.
- `crates/app_core/src/lib.rs` — `RecoveryJournalStore`, `RecoveryJournalEntry`, `RecoveryJournalStoreError`, `RecoveryJournalDocument`, and store tests (persistence newest-first, 20-entry bound, newer-schema rejection).
- `apps/desktop/src/main.rs` — `recovery_journal_path`, journal snapshot before `confirm_operation_action`'s abort/continue dispatch.
- `apps/desktop/src/views/working_copy.rs` — `operation_conflict_overview` helper and banner line.
- `apps/desktop/src/tests.rs` — a test for the conflict-overview copy.
- `PLAN.md`, `docs/work-log.md` — mark all Phase 7 items and exit criteria complete.

**Acceptance checks:**
- `recovery_snapshot` records HEAD oid, the symbolic branch, and every local branch tip; after a fast-forward merge moves HEAD, the recorded `old_head` still equals the true pre-operation commit.
- Confirming an abort or continue records a journal entry (repository, timestamp, refs) before Git runs; a snapshot failure prevents the mutation; a journal write failure is reported without blocking.
- The journal persists newest-first across a reload, is bounded to 20 entries, and rejects a newer schema without overwriting it.
- The operation banner shows a conflict overview naming the conflicted-file count and next step.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the full gates pass after this unit. `git_domain` adds the pure recovery-record types; `git_cli::recovery_snapshot` captures HEAD + the symbolic branch + local branch tips, proven by a temporary-repository test that rebases the recorded state against a fast-forward merge that moves HEAD; `app_core`'s versioned, bounded `RecoveryJournalStore` persists newest-first, caps at 20 entries, and rejects newer schemas; the desktop records the snapshot before every confirmed abort/continue and surfaces journal-write failures without blocking; the operation banner gains the `operation_conflict_overview` count line. All Phase 7 checkboxes and exit criteria are now checked. Workspace suite totals 71 tests (37 git_cli, 16 desktop, 8 app_core, 8 git_domain, 2 ui_kit).

## 2026-08-08 — Phase 7 / operation-state banner with confirmed abort and continue

**Intent:** surface an in-progress merge, cherry-pick, revert, or rebase in the desktop Working Copy view and let the user abort it or continue it after resolving and staging conflicts. This implements the `Add operation-state banner` checklist item, the `Abort merge`/`Abort rebase` items, and the `Continue operation after conflicts` item, and moves the exit criterion "The application accurately reflects in-progress Git operation state" to the UI.

**Design:** `working_copy.operation` (populated by `worktree_status`) drives a warning `state_panel` at the top of the Working Copy view that names the paused operation and its short target oid. Two controls request an Abort or Continue decision into `pending_operation_action`; a confirmation card then executes it through the existing background lifecycle. `confirm_operation_action` dispatches on `(OperationAction, InProgressOperation)`: abort maps to the matching `--abort` command and continue maps to `continue_operation`; success refreshes the working copy so the banner disappears. Because aborting discards conflict work and continue commits the staged resolution, both require the confirmation card; cancelling is a no-op.

**Files (planned):**
- `apps/desktop/src/app_state.rs` — `OperationAction` enum and `pending_operation_action: Option<OperationAction>`.
- `apps/desktop/src/main.rs` — `request_operation_abort`, `request_operation_continue`, `cancel_operation_action`, `confirm_operation_action`; initialize the field in both constructors and clear it on repository change.
- `apps/desktop/src/views/working_copy.rs` — `operation_banner_view` and `operation_confirmation_view`, wired into `repository_view`.
- `apps/desktop/src/tests.rs` — a test that requesting abort/continue sets the pending action and cancelling is a no-op.
- `PLAN.md`, `docs/work-log.md` — mark `Add operation-state banner`, `Abort merge`, `Abort rebase`, and `Continue operation after conflicts` complete.

**Acceptance checks:**
- A paused operation (from `working_copy.operation`) renders a warning banner naming the operation and target oid, with Abort and Continue controls.
- Requesting Abort or Continue shows a confirmation card; cancelling is a no-op.
- Confirm dispatches to the correct `--abort`/`--continue` Git command for each operation kind and refreshes the working copy on success.
- With no operation paused, no banner appears and no request can be recorded.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

## 2026-08-08 — Phase 7 / history operation mutations (merge, cherry-pick, revert, rebase)

**Intent:** add the typed Git boundary for the `safe history operations` group: merge, cherry-pick, revert, and rebase with abort and continue support, all gated on the in-progress operation detection. Conflicts pause the repository with the correct state marker; callers can abort (returning to the pre-operation state) or resolve, stage, and continue.

**Design:** each operation is a typed, non-shell `git` command: `merge <branch>`, `cherry-pick <oid>`, `revert --no-edit <oid>`, `rebase <base>`, plus the matching `--abort` forms. `continue_operation` dispatches on `InProgressOperation`: merge continues with `git commit --no-edit` (reusing `MERGE_MSG`), cherry-pick/revert with `--continue --no-edit`, and rebase with `git rebase --continue` under `GIT_EDITOR=true` (via a new bounded `run_env` that the existing `run` delegates to, preserving the 8 MB concurrent output reader). `git 2.51` prints merge-conflict diagnostics to stdout, so `command_error` now falls back to stdout when stderr is empty, making conflicts actionable. A new `NoOperationInProgress` error guards `continue_operation`.

**Files (planned):**
- `crates/git_cli/src/lib.rs` — `merge_branch`, `abort_merge`, `cherry_pick`, `abort_cherry_pick`, `revert_commit`, `abort_revert`, `rebase_branch`, `abort_rebase`, `continue_operation`; `run_env`; `command_error` stdout fallback; `NoOperationInProgress`; temporary-repository integration tests.
- `docs/work-log.md` — this entry.

**Acceptance checks:**
- A fast-forward merge and a conflicting merge both behave correctly; the conflict pauses with `Merge { oid }`, `abort_merge` returns to `None`, and resolve+stage+`continue_operation` finishes the merge with the resolved content.
- Conflicting cherry-pick and revert pause with their markers; each aborts and continues correctly after staging a resolution.
- A conflicting rebase pauses with `Rebase`; abort returns to the original branch, and resolve+stage+continue replays the change.
- `continue_operation` on `None` fails with `NoOperationInProgress` without running Git.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the full gates pass. All nine boundary methods build Git commands as separate typed arguments, and five new temporary-repository tests prove the fast-forward merge, the conflict/abort path for merge, resolve+stage+continue for merge/cherry-pick/revert/rebase, and the `NoOperationInProgress` guard. `git 2.51.1` merges conflict diagnostics to stdout, which `command_error` now surfaces when stderr is empty. The workspace suite totals 65 tests (36 git_cli, 14 desktop, 8 git_domain, 5 app_core, 2 ui_kit).

## 2026-08-08 — Phase 7 / in-progress operation detection

**Intent:** give the status model a reliable, UI-independent view of a history-changing Git operation (merge, cherry-pick, revert, rebase) that is paused awaiting user action. This is the foundation for the operation-state banner, recovery journal, and abort/continue actions in the `Phase 7 — safe history operations` group, and moves the exit criterion "The application accurately reflects in-progress Git operation state" forward without rendering anything yet.

**Design:** Git records paused operations as per-worktree state files under the absolute Git directory (`MERGE_HEAD`, `CHERRY_PICK_HEAD`, `REVERT_HEAD`, and the `rebase-merge`/`rebase-apply` directories), which `discover_repository` already exposes as `WorktreeRepository.git_dir`. `git_domain` gains the pure `InProgressOperation` enum (`None`, `Merge { oid }`, `CherryPick { oid }`, `Revert { oid }`, `Rebase`) and `WorktreeStatus.operation`. `git_cli::in_progress_operation` checks those files (reading the hex oid best-effort) and `worktree_status` attaches the result so the desktop's existing status refresh carries it. No UI is changed in this unit.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `InProgressOperation` enum and the `operation` field on `WorktreeStatus`.
- `crates/git_cli/src/lib.rs` — `in_progress_operation`, a `read_state_oid` helper, wiring into `worktree_status`, and temporary-repository integration tests that create real paused states via genuine merge, rebase, cherry-pick, and revert conflicts (then abort them).
- `docs/work-log.md` — this entry.

**Acceptance checks:**
- A genuine conflicting `git merge` leaves `MERGE_HEAD` and `in_progress_operation` reports `Merge` with the branch oid; `git merge --abort` returns to `None`.
- A genuine conflicting `git rebase` reports `Rebase`; `git rebase --abort` returns to `None`.
- Genuine conflicting `git cherry-pick` and `git revert` report their variants with the target oid; aborting each returns to `None`.
- A clean repository reports `None`.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the full gates pass. `git_domain::InProgressOperation` (`None`, `Merge { oid }`, `CherryPick { oid }`, `Revert { oid }`, `Rebase`) and `WorktreeStatus.operation` carry the paused-operation state, and `git_cli::in_progress_operation` checks `MERGE_HEAD`, `CHERRY_PICK_HEAD`, `REVERT_HEAD`, and the `rebase-merge`/`rebase-apply` directories under `git_dir`, reading the hex oid best-effort. `worktree_status` now attaches the result so the desktop status refresh reflects it. Three temporary-repository tests create genuine paused states (conflicting merge, conflicting rebase, conflicting cherry-pick + revert) and verify each reports the right variant with a non-empty oid and that `--abort` returns to `None`. The workspace suite totals 60 tests (31 git_cli, 14 desktop, 8 git_domain, 5 app_core, 2 ui_kit).

## 2026-08-08 — Phase 7 / discard a single unstaged hunk

**Intent:** complete the `Discard hunk` checklist item by letting the user discard one unstaged text hunk back to the index content, mirroring the tested `stage_hunk`/`unstage_hunk` foundation and the confirmed discard posture of the file- and line-level discards.

**Design:** `GitExecutable::discard_hunk` re-runs `git diff` for the selected path at apply time, extracts the requested hunk with the existing `single_hunk_patch` helper (reusing the raw file header), and pipes it to `git apply --reverse --recount --whitespace=nowarn`, restoring the index content for that hunk's lines in the working tree. Because discarding is destructive, the desktop flow reuses the confirmation pattern: `pending_hunk_discard: Option<(GitPath, usize)>` is set by `request_hunk_discard`, cleared by `cancel_hunk_discard`, and executed by `confirm_hunk_discard` through the same background lifecycle as `stage_diff_hunk`. The diff viewer gains a per-hunk "Discard hunk N" button on unstaged text diffs.

**Files (planned):**
- `crates/git_cli/src/lib.rs` — add `discard_hunk` and a temporary-repository integration test proving one hunk discards while a second hunk and the index remain untouched.
- `apps/desktop/src/app_state.rs` — `pending_hunk_discard: Option<(GitPath, usize)>` state.
- `apps/desktop/src/main.rs` — `request_hunk_discard`, `cancel_hunk_discard`, `confirm_hunk_discard`; initialize the field in both constructors and clear it on repository change and whenever the loaded diff is replaced.
- `apps/desktop/src/views/diff_viewer.rs` — per-hunk "Discard hunk N" control next to the existing Stage/Unstage control, shown only for unstaged, complete, text diffs.
- `apps/desktop/src/views/working_copy.rs` — `hunk_discard_confirmation_view` shown in the repository view.
- `apps/desktop/src/tests.rs` — a test that requesting a hunk discard sets the pending state and cancelling is a no-op.
- `PLAN.md`, `docs/work-log.md` — mark `Discard hunk` complete.

**Acceptance checks:**
- A temporary repository proves `discard_hunk` restores only the requested hunk to index content while another hunk's change remains in the working tree and the index stays unchanged.
- The confirmation flow requires a request before any Git command runs; cancellation is a no-op.
- The hunk discard control is available only for an unstaged, complete, text diff, and only when no mutation is in flight.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the full gates pass. `git_cli::discard_hunk` reuses `single_hunk_patch` against a fresh `git diff` and pipes it to `git apply --reverse --recount --whitespace=nowarn`; the new `discards_only_the_requested_unstaged_hunk` fixture proves only hunk 0 returns to index content while hunk 1's change survives and the staged diff stays empty. The desktop app gains `pending_hunk_discard`, `request_hunk_discard`/`cancel_hunk_discard`/`confirm_hunk_discard`, a per-hunk "Discard hunk N" control next to Stage/Unstage (unstaged diffs only), a `hunk_discard_confirmation_view` card, and a GPUI test covering request/cancel/staged-refusal. The workspace suite totals 57 tests (28 git_cli, 14 desktop, 8 git_domain, 5 app_core, 2 ui_kit); the earlier single-suite failure did not reproduce across repeated runs.

## 2026-08-08 — Phase 7 / line-level partial staging and discard

**Intent:** let the user stage only the added lines they select in an unstaged diff, and discard selected lines back toward the index, each through Git's own patch validation. This implements the `Stage selected lines` and `Discard selected lines` checklist items and reuses the single-hunk patch foundation.

**Design:** the diff model gains old/new line numbers so a pure partial-patch builder can recompute unified-diff hunk headers for a subset of change lines. Both commands re-run `git diff` for the selected path at apply time, keep every context line as an anchor, and emit only the selected additions/removals with a recomputed `@@` header. Staging pipes the patch to `git apply --cached --recount --whitespace=nowarn`; discarding pipes the same patch to `git apply --reverse --recount --whitespace=nowarn` (restoring the index content for those lines in the working tree). Discard requires a confirmation that names the path and line count.

**Files (planned):**
- `crates/git_domain/src/lib.rs` — `DiffLine` gains `old_line`/`new_line`; add `parse_hunk_header` and `selected_lines_patch` with fixture tests.
- `crates/git_cli/src/lib.rs` — record line numbers in `parse_unified_diff`; add `stage_lines`/`discard_lines` commands, the `patch_for_selected_lines` helper, a `PatchLinesUnavailable` error, and temporary-repository integration tests.
- `apps/desktop/src/app_state.rs` — `selected_diff_lines: Vec<(usize, usize)>` and `pending_line_discard: Option<(GitPath, Vec<(usize, usize)>)>` state.
- `apps/desktop/src/main.rs` — `toggle_diff_line`, `stage_selected_diff_lines`, `request_line_discard`, `cancel_line_discard`, `confirm_line_discard`; clear line selection whenever the loaded diff is replaced.
- `apps/desktop/src/views/diff_viewer.rs` — render per-line rows with line-number gutters, click-to-toggle selection on change lines, and Stage/Discard selected-lines controls.
- `apps/desktop/src/views/working_copy.rs` — `line_discard_confirmation_view` shown in the repository view.
- `apps/desktop/src/views/components.rs` — any shared line-row styling helper.
- `PLAN.md`, `docs/work-log.md` — mark `Stage selected lines` and `Discard selected lines` complete.

**Acceptance checks:**
- Line numbers are recorded on parsed diff lines and survive fixture parsing (context, addition, removal).
- `selected_lines_patch` keeps only selected change lines plus all context, recomputes `@@` headers, and returns `None` when nothing is selected.
- A temporary repository proves staging one selected added line leaves the other added line unstaged, and that the staged/unstaged diffs split exactly.
- A temporary repository proves discarding one selected added line removes it from the working tree while leaving the other change; discarding a selected removal restores the deleted line.
- Invalid selection indices fail without touching the index or working tree.
- The diff view offers line selection only for an unstaged, complete, text diff; discard requires confirmation and cancellation is a no-op.
- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

**Verification:** the full gates pass. `git_domain` adds `parse_hunk_header`, `selected_lines_patch` (which splits a hunk at unselected change lines, drops change-less segments, and recomputes each `@@` header from the walked positions), and 8 tests including a two-segment split fixture. `git_cli` records `old_line`/`new_line` on parsed diff lines, adds `stage_lines` and `discard_lines` over `git apply --cached` / `--reverse --recount`, and its 27 tests prove partial staging, partial discard, removal restore, out-of-range rejection, and line-number recording against temporary repositories. The desktop app gains the `selected_diff_lines`/`pending_line_discard` state, five mutations (`toggle_diff_line`, `stage_selected_diff_lines`, `request_line_discard`, `cancel_line_discard`, `confirm_line_discard`), a per-line diff view with old/new gutters and click-to-select change rows, a confirmation card, and two GPUI tests for selection toggling and discard cancellation; the workspace suite totals 55 tests.

## 2026-08-08 — UI decomposition / split main.rs into views/

**Intent:** Split the 4,300-line `apps/desktop/src/main.rs` into a small `main.rs` plus a `views/` module tree mirroring the window structure that `PLAN.md §7` and §8.1 already propose (toolbar · sidebar · working copy · history · diff · inspector · welcome · shared components). This is a pure structural refactor: no behavior change, no new dependencies, no GPUI logic moved into or out of domain crates.

**Why:** The current single-file desktop shell is the single largest blocker for the subsequent UI work-streams (inline commit composer, real toolbar, sidebar tree with icons, polished history, in-app dialogs, command palette). Each of those improvements needs a stable home in its own module; today every change collides in `main.rs`. `PLAN.md §7` already prescribes `apps/desktop/src/views/` and §7.1 lists `ui_kit` primitives that this split enables per-module work on.

**Files (planned):**
- `apps/desktop/src/main.rs` — trimmed to entry point, constants, panic/geometry helpers, window options, top-level dispatch, and the `impl GitronimoApp` block containing state-mutating methods (construction, observer setup, refresh, navigation, history load/selection, branch operations, network commands, mutation, stash, commit, diff hunk, watcher, context menu, prompts).
- `apps/desktop/src/app_state.rs` — `GitronimoApp` struct, `ShellState`, `ThemeMode`, `RepositoryView`, `LastAction`, `NetworkOperation`, `ForcePushState`, `ShortcutReferenceState`, `RefContext`, `RefKind`, `OpenedRepository`, `Mutation`, `StashAction`, free helpers (`network_failure_message`, `git_failure_message`, `repository_is_available`, `repository_unavailable_message`, `appearance_from_window`, `window_title`, `resize_width`, `shows_inspector`, `discard_selected`, `eligible_trash_path`).
- `apps/desktop/src/views/mod.rs` — module declarations + private re-exports.
- `apps/desktop/src/views/workspace.rs` — root `Render for GitronimoApp` layout plus shortcut reference and activity bar.
- `apps/desktop/src/views/toolbar.rs` — `workspace_toolbar`.
- `apps/desktop/src/views/sidebar.rs` — `sidebar_view`, `ref_rows`, `welcome_sidebar_view`, `status_badge`.
- `apps/desktop/src/views/working_copy.rs` — `repository_view`, `status_groups`, `status_group_view`, `context_menu_view`, `mutation_controls`, `navigation_controls`, `network_cancel_button`, `discard/stash/branch-delete/force-with-lease` confirmation views, `ref_context_menu_view`.
- `apps/desktop/src/views/history.rs` — `history_view`, `history_row_count`.
- `apps/desktop/src/views/commit_composer.rs` — `commit_composer_view`.
- `apps/desktop/src/views/diff_viewer.rs` — `diff_view`.
- `apps/desktop/src/views/welcome.rs` — `welcome_view`, `welcome_feature_card`.
- `apps/desktop/src/views/components.rs` — `StatusGroups`, `workspace_section`, `file_action_button`, `window_action_button`, `primary_window_action_button`, `mutation_button`, `validated_action_button`, `ActionTooltip` + its `Render`, `state_panel`, `loading_view`, `error_view`, `status_path`, `status_label`, `empty_status_message`, `activity_color`, `activity_label`.
- `apps/desktop/src/tests.rs` — extracted unit tests (kept in desktop crate; tests touch private items via `pub(crate)` re-exports).
- `docs/work-log.md`, `PLAN.md` — this entry plus checkbox mapping if applicable.

**Visibility change:** types and helpers that tests reach (e.g. `ShellState` variants, `GitronimoApp::welcome`, `window_options`, `resize_width`, `shows_inspector`, `window_title`, `network_failure_message`, `git_failure_message`, `crash_report_path`, `crash_report_body`, `eligible_trash_path`, `repository_is_available`, `keymap`, `GitPath`, `WorktreeRepository`, `LastAction`) become `pub(crate)` so the `tests` module can import them. No public API changes; binary crate only.

**Acceptance checks:**
- `cargo fmt --all -- --check` clean.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean.
- `cargo test --workspace --all-features` passes with the same set of tests as before.
- `cargo deny check` passes.
- The `.app` builds (`cargo build --release`) without new warnings.
- No dependency added; `Cargo.toml` untouched.
- No GPUI import added to `git_domain`, `app_core`, `git_cli`, `test_support`, or `ui_kit`.
- No Git/domain logic moved into render modules: render modules continue to call existing `GitronimoApp` methods; no new Git CLI calls introduced inside `views/`.
- After the split, `main.rs` retains the entry point and every state-mutating `GitronimoApp` method, while all window render code moved into `views/` (toolbar · sidebar · working copy · history · diff · commit composer · welcome · shared components).
- All existing tests retain their assertions (welcome window opens, keybindings dispatch, pane widths safe, error shell explicit, network failures actionable without echoing remote output, empty/loading copy explains next state, window titles distinguish welcome/loading/drafts, repository loss safe recovery, crash reports local, trash refuses unsafe paths).

**Verification:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features` (41 tests), and `cargo deny check` all pass. `main.rs` dropped from 4,355 to ~2,023 lines; the render code moved to `apps/desktop/src/views/` and shared shell types to `app_state.rs`. Only Cargo's upstream future-incompatibility notice for `block`/`proc-macro-error2` remains, as before the refactor.

## 2026-08-07 — Phase 6 / context-sensitive action validation

**Intent:** make branch and remote controls visibly unavailable when the current repository state cannot support them.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** controls explain missing branches, remotes, upstreams, or an attached branch before a click can start an operation; enabled controls keep their existing background lifecycle; validation helpers have focused tests for detached, branch-only, and upstream states.

## 2026-08-07 — Phase 6 / workspace navigation history

**Intent:** add predictable back and forward navigation between the Working Copy and History views without reloading repository state.

**Files:** `apps/desktop/src/actions.rs`, `apps/desktop/src/keymap.rs`, `apps/desktop/src/menus.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** explicit view changes enter a small in-memory history; back and forward preserve the active repository and loaded history; a new navigation clears forward history; shortcuts and menu labels expose the actions; pure navigation tests cover the state transitions.

## 2026-08-07 — Phase 6 / cancellation and temporary-file verification

**Intent:** lock in existing cancellation behavior and prove commit-message temporary files are removed after both successful and rejected commits.

**Files:** `crates/git_cli/src/lib.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** the existing child-process fixture proves cancellation exits unsuccessfully; the commit fixture compares process-specific temporary-message files before and after success and hook rejection; no cleanup path removes user data.

## 2026-08-07 — Phase 6 / bounded Git process output

**Intent:** cap captured Git stdout and stderr through the CLI boundary so a malformed or hostile repository cannot consume unbounded application memory.

**Files:** `crates/git_cli/src/lib.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** regular Git commands drain stdout and stderr concurrently with a fixed byte cap; an oversized stream returns a clear I/O error; the desktop network path uses the same bounded stderr reader; parser and local-remote integration tests remain intact.

## 2026-08-07 — Phase 6 / public-beta documentation

**Intent:** align public onboarding, architecture, troubleshooting, contribution, security, trademark, notices, and issue templates with the implemented local-first macOS beta.

**Files:** `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `TRADEMARKS.md`, `docs/architecture.md`, `docs/troubleshooting.md`, `docs/third-party-notices.md`, `.github/ISSUE_TEMPLATE/bug_report.md`, `.github/ISSUE_TEMPLATE/feature_request.md`, `PLAN.md`, and this work log.

**Acceptance checks:** documentation distinguishes implemented workflows from planned scope and unsigned development bundles from a notarized release; no credential or telemetry claim overreaches; contribution and issue flows preserve reproducible Git safety reports; document links resolve locally.

## 2026-08-07 — Phase 6 / local crash reports

**Intent:** retain a minimal local crash report for beta diagnosis without network access, automatic upload, or panic payload capture.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** application startup installs one panic hook; a report contains the timestamp and source location only; the report location is under Gitronimo application support; helper tests prove no panic payload or upload instruction is written.

## 2026-08-07 — Phase 6 / repository-loss and stale-lock recovery

**Intent:** safely stop using repositories that disappear while open and turn index-lock failures into clear, non-destructive recovery instructions.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** polling detects a missing worktree or Git directory before another refresh; the shell switches to an actionable error state and stops watching; failures mentioning `index.lock` instruct the user to verify no Git process is running and never delete the lock; focused unit tests cover availability and lock guidance.

## 2026-08-07 — Phase 6 / corrupt-preferences recovery

**Intent:** quarantine malformed preferences and recreate an empty, valid document so startup and subsequent preference writes remain usable.

**Files:** `crates/app_core/src/lib.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** malformed JSON is renamed to a non-destructive backup beside the preferences file; a fresh versioned document replaces it; newer schemas are still rejected without modification; tests prove the backup and recovered document behavior.

## 2026-08-07 — Phase 6 / commit-draft protection

**Intent:** prevent an unsaved commit subject or body from silently disappearing when a different repository is opened.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** changing repositories with a draft requires a native explicit discard decision; cancellation leaves the current repository and draft untouched; confirmed discard clears both draft fields before opening; title draft marking uses the same predicate.

## 2026-08-07 — Phase 6 / keyboard discovery and repository state

**Intent:** make core actions discoverable from a native command palette and shortcut reference, while reflecting the active repository and unsaved commit draft in the window title.

**Files:** `apps/desktop/src/actions.rs`, `apps/desktop/src/keymap.rs`, `apps/desktop/src/menus.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** Command-Shift-P opens a keyboard-accessible native command picker; every listed action routes through existing app methods; the title identifies the opened repository and draft state; the shortcut reference describes current bindings; unit tests cover title derivation and binding registration.

## 2026-08-07 — Phase 6 / stateful workspace polish

**Intent:** make the first-use, loading, empty, and failure states self-explanatory and visually distinct without adding a component dependency.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** opening and failure surfaces name the next action; working-copy sections distinguish loading from an empty repository; activity status has a visible state cue in both themes; the desktop test suite continues to create welcome and error shells successfully.

## 2026-08-07 — Phase 5 / interactive hierarchical ref browser

**Intent:** render slash-separated refs as expandable sidebar groups, retain expansion choices across launches, and expose safe context actions for local refs, remote refs, tags, and remotes.

**Files:** `crates/app_core/src/lib.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** hierarchy derives only from the already-loaded ref snapshot; expansion keys are persisted in the existing versioned preferences document; selecting a ref reveals context-appropriate actions without introducing Git work in render code; app-core tests prove persistence and desktop tests prove hierarchy/action selection helpers.

## 2026-08-07 — Phase 5 / advanced force-with-lease

**Intent:** offer force-with-lease only behind a separate confirmation, never through ordinary push or publish.

**Files:** `crates/git_cli/src/lib.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** the typed Git boundary uses `--force-with-lease` rather than `--force`; the UI has no default force path and requires a second explicit confirmation before starting the network operation; a local bare-remote fixture proves the argument can update a diverged branch.

## 2026-08-07 — Phase 5 / cancellable network operations

**Intent:** run fetch, pull, push, and publish through the existing piped Git child boundary so the UI can show an active operation and cancel it without blocking rendering.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** a network operation owns exactly one in-flight Git child; cancellation requests termination from the UI thread without waiting; completion clears the operation, refreshes refs and status only on success, and preserves a concise actionable failure category.

## 2026-08-07 — Phase 5 / local branch safety controls

**Intent:** expose current-branch rename and a deliberate two-step local-branch deletion flow over the already-tested Git mutation boundary.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** rename is available only from an attached local branch; deletion accepts only a current local ref, shows the affected branch before either safe or explicit force deletion, and each successful operation refreshes working-copy and ref state.

## 2026-08-07 — Phase 5 / branch from history selection

**Intent:** create and check out a local branch from the currently selected commit without using a text-supplied revision.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** the control is only meaningful with a selected history commit; its exact OID is supplied to the existing typed create-branch boundary; success refreshes refs and status.

## 2026-08-07 — Phase 5 / selected remote fetch

**Intent:** let users choose a configured remote for fetch while rejecting arbitrary dialog input before a Git process starts.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** the chooser accepts only names from the current ref snapshot; cancellation is a no-op; a valid remote uses the same background mutation and refresh lifecycle as the default fetch.

## 2026-08-07 — Phase 5 / remote controls

**Intent:** expose fetch, pull, push, and publish through the existing background mutation lifecycle without a default force path.

**Files:** `apps/desktop/src/main.rs` and this work log.

**Acceptance checks:** controls use the first configured remote only as an explicit default; pull/push require current upstream; publish sets upstream; all success paths refresh status and refs while failures retain Git's useful stderr in the activity area.

## 2026-08-07 — Phase 5 / remote mutation boundary

**Intent:** add typed fetch, pull, push, and publish commands backed by local bare-remote integration tests.

**Files:** `crates/git_cli/src/lib.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** the default or selected remote is passed as an individual argument; ordinary push/pull never imply force; publishing explicitly sets upstream; local-bare fixtures prove state propagation and failures return Git stderr to the UI.

## 2026-08-07 — Phase 5 / branch controls

**Intent:** expose safe checkout and create-from-HEAD controls through native prompts and the existing background mutation lifecycle.

**Files:** `apps/desktop/src/main.rs` and this work log.

**Acceptance checks:** branch commands never run on the render thread; in-flight work disables duplicate requests; success refreshes working-copy and ref state; Git's dirty-worktree failures remain visible in the activity status.

## 2026-08-07 — Phase 5 / local branch mutation boundary

**Intent:** add typed checkout, create, rename, and delete operations at the Git boundary before wiring confirmations into the UI.

**Files:** `crates/git_cli/src/lib.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** temporary repositories cover branch creation from HEAD and a selected ref, checkout, rename, safe merged deletion, unmerged refusal, forced deletion only through an explicit caller choice, dirty-worktree rejection, and detached HEAD status.

## 2026-08-07 — Phase 5 / ref sidebar

**Intent:** display the tested ref snapshot in the existing repository sidebar without moving Git work into rendering.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** opening a repository loads refs on the background executor; the sidebar exposes local/remote branches, tags, remotes, and current/upstream/ahead-behind status; stale repository loads are ignored.

## 2026-08-07 — Phase 5 / ref browser boundary

**Intent:** load one UI-independent snapshot of local and remote branches, tags, and remotes through typed Git commands before adding mutations.

**Files:** `crates/git_domain/src/lib.rs`, `crates/git_cli/src/lib.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** a real repository fixture proves local/remote branches, tags, remotes, current branch, upstream, and ahead/behind are represented without parsing human-oriented Git output; hierarchical names retain their path segments for the UI.

## 2026-08-07 — Phase 4 / history list overscan

**Intent:** use GPUI’s retained virtual list for a small, explicit history-row overdraw buffer above and below the viewport.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** the History list uses `ListState` with two row-heights of overdraw; source, page, and search changes reset its item count; only the visible range plus overdraw is constructed.

**Verification:** `ListState::new(..., px(56.0))` supplies a two-row overdraw buffer above and below the visible history viewport. The desktop target passes Clippy and its test suite after replacing `uniform_list`; no custom virtualizer or dependency was introduced.

## 2026-08-07 — Phase 4 / virtual history selection and source changes

**Intent:** keep the history browser virtualized while preserving mouse selection, and reject a page that returns after its history source changes.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** visible virtual rows retain their source indexes and invoke the existing guarded inspector loader on click; changing between current/all/named history clears the previous page; stale page responses cannot update the new source; the UI only constructs the visible `uniform_list` range.

**Verification:** the all-ref traversal uses an opaque `all:<skip>` cursor, so subsequent pages do not duplicate the first union-of-refs page. The focused real-repository test covers current, all-ref, and named history. The workspace Clippy and test gates pass; the only output is the pre-existing upstream future-incompatibility warning for `block 0.1.6` and `proc-macro-error2 2.0.1`. GPUI 0.2.2 exposes only the visible range from `uniform_list`, not a configurable overscan range, so that checklist item remains deliberately open rather than simulated.

## 2026-08-07 — Phase 4 / keyboard history navigation

**Intent:** make history selection navigable without mouse input and prevent a stale inspector request from replacing a newer selection.

**Files:** `apps/desktop/src/actions.rs`, `apps/desktop/src/keymap.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** Up/Down select adjacent loaded commits only in History; a monotonically increasing selection token rejects stale inspector responses; navigation reuses the same background inspector request.

## 2026-08-07 — Phase 4 / selected-commit inspection

**Intent:** load changed paths and a bounded unified diff for the selected history commit through the existing Git process boundary.

**Files:** `crates/git_cli/src/lib.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** a temporary repository proves commit file listing and commit diff loading; inspector requests run off the UI thread; stale selection responses do not replace a newer inspector.

## 2026-08-07 — Phase 4 / history interactions

**Intent:** make the bounded history browser useful without expanding the query scope: filter loaded rows, copy OIDs, reveal HEAD, and show commit metadata.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** search filters subject/author/OID in the loaded page and cannot overwrite a newer repository state; the inspector exposes commit details; copy uses the platform clipboard; Reveal HEAD selects the matching loaded row.

## 2026-08-07 — Phase 4 / history browser shell

**Intent:** connect the bounded history adapter to an asynchronous desktop History view with graph lanes, commit selection, basic inspector data, and explicit paging.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** entering History starts a background first-page request; rows show graph lane, author, subject, time, and ref decorations; selecting a row updates an inspector; Load more uses the page cursor rather than full-history loading; stale repository responses are ignored.

## 2026-08-07 — Phase 4 / graph layout

**Intent:** add a deterministic pure lane layout that supports linear history, branches, two-parent merges, and an explicit octopus fallback while carrying lanes across history pages.

**Files:** `crates/git_domain/src/lib.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** graph fixtures assert lane assignment for linear, branch, merge, and octopus shapes; passing the returned lane state into a later page preserves the next row’s lane.

## 2026-08-07 — Phase 4 / bounded history data

**Intent:** add the smallest cursor-paged, UI-independent history model: bounded commits with parents, author/committer data, subject/body, plus decorations loaded separately.

**Files:** `crates/git_domain/src/lib.rs`, `crates/git_cli/src/lib.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** typed Git requests load a bounded page for current/all/selected refs; each record retains parents and metadata; a cursor limits subsequent pages; decorations are not embedded in the main history log; temporary repositories prove page boundaries and all-ref selection.

## 2026-08-07 — Phase 3 / composer identity and shortcut

**Intent:** complete the composer feedback loop by loading the repository’s configured author identity and making the subject prompt reachable from the keyboard.

**Files:** `apps/desktop/src/actions.rs`, `apps/desktop/src/keymap.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** opening a repository loads and displays `Name <email>` or a precise missing-identity message; Command-Shift-C opens subject entry; commit failures keep the current drafts unchanged.

## 2026-08-07 — Phase 3 / native commit composer

**Intent:** expose the already-tested secure commit boundary through an accessible macOS composer flow without a third-party component library.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** subject and body are collected through native keyboard-accessible prompts; drafts, amend, and sign-off state stay in the app; commit is enabled only with staged content and a non-empty subject; author identity is shown or reported missing; a rejected commit retains both drafts.

## 2026-08-07 — Phase 3 / confirmed discard and Finder trash

**Intent:** make discard explicit and reversible where possible: restore tracked paths with Git, move eligible untracked paths to Finder’s Trash, and refuse symlinks, nested repositories, non-UTF-8 names, and unsafe paths.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** the first destructive click shows affected-path consequences and a distinct confirmation; tracked paths use the existing tested Git restore; untracked paths are sent to Finder with arguments rather than shell interpolation; validation tests cover unsafe paths, symlinks, and nested repositories.

## 2026-08-07 — Phase 2 / file actions and bounded diff expansion

**Intent:** finish the selected-file affordances without adding a UI framework: copy the resolved path, invoke Finder or the default editor through typed platform commands, and reload a deliberately truncated diff only when the user asks.

**Files:** `crates/git_cli/src/lib.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** default diff loading is capped at one megabyte; “Load full diff” explicitly reloads the same selected staged or unstaged path; Copy path writes the resolved worktree path to the platform clipboard; Finder reveal and editor open are passed as individual process arguments; replacing or leaving a repository releases the prior watcher.

## 2026-08-07 — Phase 2 / porcelain status model

**Intent:** add the smallest UI-independent, byte-safe working-copy status model and parser needed to represent Git porcelain-v2 output accurately.

**Files:** `crates/git_domain/src/lib.rs`, `crates/git_cli/src/lib.rs`, and this work log.

**Acceptance checks:** a typed Git command reads porcelain-v2 with branch data and optional ignored files; parser tests cover headers, detached HEAD, upstream divergence, ordinary, rename/copy, unmerged, untracked, ignored, submodule, and non-UTF-8 paths; temporary-repository integration proves the command and stash count.

**Deferred:** sidebar, working-copy groups, diff rendering, mutations, and watcher lifecycle remain separate Phase 2 checklist groups.

**Verification:** the `git_cli` focused suite passes seven tests, including a temporary repository status/stash integration and raw parser fixtures for every porcelain-v2 record class. The desktop focused suite renders the status shell and passes all four tests. Both checks are run with the pinned Rust 1.97.1 toolchain; Cargo reports the existing upstream future-incompatibility warning for `block 0.1.6` and `proc-macro-error2 2.0.1`.

## 2026-08-07 — Phase 2 / working-copy shell

**Intent:** present status data without blocking rendering, with sidebar counts and selectable Working Copy groups.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** opening or refreshing a repository requests status in GPUI's background executor; staged, unstaged, untracked, and conflict groups have sidebar counts; rows support normal and additive selection; a right-click exposes the owned contextual action surface.

**Deferred:** the contextual actions themselves are implemented with their copy/Finder/editor checklist items, while diff rendering and filesystem watching remain separate groups.

## 2026-08-07 — Phase 2 / unified diff model

**Intent:** define and test the smallest pure model that retains unified-diff file metadata, text hunks, binary state, and missing-final-newline markers.

**Files:** `crates/git_domain/src/lib.rs`, `crates/git_cli/src/lib.rs`, and this work log.

**Acceptance checks:** parser fixtures cover text hunks, rename metadata, binary files, and no-final-newline markers without decoding path bytes; command/UI integration and large-output truncation remain separate diff checklist items.

## 2026-08-07 — Phase 2 / diff loading

**Intent:** load selected staged or unstaged diffs through typed Git arguments in the background, with a bounded display result.

**Files:** `crates/git_cli/src/lib.rs`, `apps/desktop/src/main.rs`, and this work log.

**Acceptance checks:** temporary repositories prove separate cached and worktree diffs; selected rows load without rendering-thread Git calls; over-limit output shows an explicit load-more affordance.

## 2026-08-07 — Phase 3 / Git mutation boundary

**Intent:** add typed, non-shell Git staging and unstaging operations before attaching destructive UI controls.

**Files:** `crates/git_cli/src/lib.rs` and this work log.

**Acceptance checks:** real temporary repositories cover one, many, all, and unborn-repository staging/unstaging paths; command failures remain actionable to the caller.

## 2026-08-07 — Phase 3 / staging controls

**Intent:** expose the tested staging boundary through a single-flight Working Copy control surface.

**Files:** `apps/desktop/src/main.rs` and this work log.

**Acceptance checks:** selected and all stage/unstage controls execute off the UI thread, disable while a mutation is active, and refresh status plus clear stale diff data afterward.

## 2026-08-07 — Phase 3 / commit boundary

**Intent:** commit through typed Git arguments and an atomically-created temporary message file, preserving UI drafts when Git rejects the operation.

**Files:** `crates/git_cli/src/lib.rs` and this work log.

**Acceptance checks:** integration fixtures cover normal, amend, sign-off, missing identity, and hook rejection; message files are removed after both success and failure.

## 2026-08-07 — Phase 3 / discard boundary

**Intent:** safely discard tracked modifications using Git while refusing untracked deletion until a platform trash implementation is available.

**Files:** `crates/git_cli/src/lib.rs` and this work log.

**Acceptance checks:** tracked paths restore from the index; unsafe, nested-repository, and untracked paths are rejected rather than deleted.

## 2026-08-07 — Phase 2 / polling refresh fallback

**Intent:** keep Working Copy current without UI-thread filesystem work while a native watcher integration remains unavailable.

**Files:** `apps/desktop/src/main.rs` and this work log.

**Acceptance checks:** an open repository schedules bounded background refreshes; switching repositories drops the old loop; closing the model prevents further updates.

## 2026-08-07 — Phase 2 / native watcher

**Intent:** receive platform filesystem events for the active worktree and coalesce them into the existing status refresh lifecycle.

**Files:** workspace manifests and lockfile, `apps/desktop/src/main.rs`, and this work log.

**Acceptance checks:** one `notify` watcher is retained per active repository; events are coalesced through a channel, stale watchers are replaced on repository changes, and polling remains available when watcher creation fails.

## 2026-08-06 — Phase 0 / issue 1: initialize workspace and CI

**Intent:** create the reproducible Rust workspace and quality gates required before the GPUI window spike.

**Files:** root Cargo configuration, minimal boundary crates, policy documents, ADR template, and CI workflows.

**Acceptance checks:** workspace metadata resolves; exact GPUI dependencies resolve from `ui_kit`; formatting, Clippy, tests, and cargo-deny have an executable local/CI command.

**Deferred:** GPUI window, menus, components, resizable panels, packaging, and Git process behavior are separate checklist items.

**Environment blocker:** the local GPUI build currently stops in its Metal shader build step because Xcode's Metal Toolchain is absent. Install it with `xcodebuild -downloadComponent MetalToolchain` before running workspace-wide compile, Clippy, or tests. This is a machine prerequisite, not a dependency or source failure.

**Resolved environment prerequisite:** Metal Toolchain 17F109 was installed. The workspace now builds and its full Clippy/test gates pass. `cargo-deny` is installed by CI but is not available as a local Cargo subcommand yet, so its local result remains pending. The lockfile is generated and ready for the initial commit; its checklist item stays open until that commit is made.

## 2026-08-06 — Phase 0 / issue 2: minimal GPUI macOS window

**Intent:** prove the pinned GPUI release opens a native application window with a standard title bar and an enforced minimum size.

**Files:** `apps/desktop/src/main.rs` and its manifest.

**Acceptance checks:** the application opens a centered 1200×800 window titled “Gitronimo”, enforces an 800×560 minimum size, and a `#[gpui::test]` opens the view on GPUI's test platform.

**Deferred:** menus, actions, panels, themes, and background tasks are separate Phase 0 checklist items.

**Verification:** the `#[gpui::test]` passed, and `cargo run -p gitronimo-desktop` launched the native process successfully before it was stopped. No background task is created by this spike.

## 2026-08-06 — Phase 0 / issue 3: actions, keybindings, and application menu

**Intent:** establish typed global actions and prove macOS menus and keyboard shortcuts can dispatch them without coupling to repository behavior.

**Files:** desktop actions, menu, and application composition modules.

**Acceptance checks:** an application menu exposes File and View actions, Command-O and Command-R dispatch to the root view, and a GPUI test verifies the action event.

**Verification:** two GPUI tests pass: one creates the native-window view and one dispatches Command-O and Command-R through the keymap to the root view. The File and View menus use those same typed actions.

## 2026-08-06 — Phase 0 / issue 4: design tokens and UI boundary

**Intent:** create the project-owned semantic color vocabulary and remove raw desktop-view colors.

**Files:** `ui_kit` theme module and desktop root view.

**Acceptance checks:** every required semantic Git-client color is exposed by `ThemeColors`; the desktop imports it from `ui_kit`; and a unit test proves the window and panel backgrounds remain distinct.

**Deferred:** component wrappers, typography scale, spacing, radii, shadows, and appearance switching wait for their active checklist items.

**Verification:** `Theme::dark()` supplies the original semantic palette, the desktop no longer contains raw color values, and the focused `ui_kit`/desktop test suite passes.

## 2026-08-06 — Phase 0 / issue 5: component-library evaluation

**Intent:** evaluate the exact `gpui-component 0.5.1` release without letting it leak into product views.

**Files:** workspace dependency configuration and ADR 0001.

**Acceptance checks:** the exact `gpui 0.2.2`/`gpui-component 0.5.1` pair compiles; no desktop or domain code imports the component crate; an explicit, reversible decision is recorded.

**Decision:** remove `gpui-component`. Its required global initializer owns theme and broad application state, which conflicts with Gitronimo's original token system and a narrow `ui_kit` boundary. Build only the needed controls with GPUI primitives.

## 2026-08-06 — Phase 0 / issue 7: virtualized synthetic history

**Intent:** prove the GPUI workspace can display 100,000 history rows without constructing every row.

**Files:** desktop root view.

**Acceptance checks:** `uniform_list` receives 100,000 entries and its processor constructs only the visible range; the app renders an original sidebar, history, and inspector proof layout.

**Verification:** the focused GPUI tests and workspace Clippy gate pass with the 100,000-row `uniform_list` in place.

## 2026-08-06 — Phase 0 / issue 8: graph canvas

**Intent:** prove a custom GPUI canvas can paint graph lanes in virtualized history rows.

**Files:** desktop root view.

**Acceptance checks:** each visible synthetic-history row contains a canvas that paints a lane segment from the project graph palette; no paths are created for off-screen rows.

**Verification:** focused desktop tests and workspace Clippy pass with the custom canvas in the `uniform_list` processor.

## 2026-08-06 — Phase 0 / theme switch

**Intent:** prove project-owned light and dark appearances can switch without view-level raw colors.

**Verification:** `Theme::for_appearance` supplies both semantic palettes; Command-Shift-L and the View menu dispatch the toggle action; focused tests and Clippy pass.

## 2026-08-06 — Phase 0 / resizable panes

**Intent:** prove the three-pane layout can resize through keyboard-accessible actions.

**Verification:** Command-Option-Left and Command-Option-Right widen the sidebar and inspector within safe bounds; the bounds test and focused GPUI suite pass.

## 2026-08-06 — Phase 0 / Git executable and process spike

**Intent:** validate Finder-safe Git discovery, typed process invocation, byte-preserving status/history parsing, local fetch execution, and cancellation against real temporary repositories.

**Files:** `crates/git_cli/src/lib.rs`, its manifest, and this work log.

**Acceptance checks:** discovery considers a configured executable, GUI `PATH`, and standard macOS paths; every process uses `std::process::Command` argument APIs; `git --version`, NUL-delimited porcelain-v2 status, 500 explicit-separator history records, local fetch, and cancelling an active Git process are covered by tests.

**Deferred:** repository session state, mutation coordination, product status models, and UI integration remain later Phase 1+ work.

**Verification:** four real-repository tests pass with the installed Git executable: configured/GUI-safe/common-path discovery and `--version`; NUL-delimited porcelain-v2 status preserving a tab, newline, space, and Unicode filename; 500 NUL-separated log records; and a local bare-remote fetch whose stderr stream is read before its process completes. A piped `git hash-object --stdin` is cancelled through `Child::kill`. All application calls use `std::process::Command::args`, never a shell string.

## 2026-08-06 — Phase 0 / dependency-policy correction

**Intent:** make the committed `cargo-deny` policy accurately validate the pinned GPUI dependency graph and internal workspace dependencies.

**Files:** `deny.toml`, workspace manifests, and dependency policy documentation.

**Acceptance checks:** `cargo deny check` permits the explicitly reviewed OSI-compatible transitive licenses required by GPUI and accepts versioned internal path dependencies without weakening third-party wildcard checks.

## 2026-08-06 — Phase 0 / macOS packaging spike

**Intent:** package the desktop binary as an original, unsigned development `.app`, document the signing handoff, and make CI retain an unsigned artifact.

**Files:** `Packager.toml`, original icon assets, packaging documentation, CI workflow, and this work log.

**Acceptance checks:** pinned `cargo-packager 0.11.8` creates an app bundle with a unique bundle identifier and icon; the bundle launches outside Cargo; Apple Silicon and Intel builds compile; signing is documented without committing credentials; CI uploads the unsigned development artifact.

**Verification:** `cargo-packager 0.11.8` built `target/release/Gitronimo.app`. Its `Info.plist` declares `com.gitronimo.desktop`, `public.app-category.developer-tools`, and the original `gitronimo.icns`; `open target/release/Gitronimo.app` launched its embedded executable directly. The host is Apple Silicon (`arm64`); the installed `x86_64-apple-darwin` target also produces a Mach-O x86_64 release executable. CI installs the same exact packager version and uploads the unsigned bundle. The local signing handoff names no credentials.

## 2026-08-06 — Phase 0 / GPUI lifecycle check

**Intent:** verify that closing the prototype does not leave a desktop process or child task alive.

**Files:** `PLAN.md` and this work log only.

**Acceptance checks:** terminate the launched app, confirm no Gitronimo executable remains, and relaunch the packaged app. The prototype deliberately creates no GPUI background task or Git process.

**Verification:** after terminating the launched packaged executable, `pgrep -fl gitronimo-desktop` produced no output. The prototype has no spawned GPUI task, worker, or Git child process, so no background task remained to leak.

## 2026-08-06 — Phase 0 / decision-rule alignment

**Intent:** align `AGENTS.md` with ADR 0001 after the component-library evaluation.

**Files:** `AGENTS.md` and this work log.

**Acceptance checks:** future changes cannot treat the rejected component library as an adopted dependency without replacing the documented decision.

**Verification:** `git_domain`, `app_core`, `git_cli`, `test_support`, and `ui_kit` are separate workspace crates, with GPUI confined to the desktop/UI boundary.

## 2026-08-06 — Phase 0 / clean-source verification

**Intent:** prove the current source compiles outside the active, dirty worktree without copying the user-provided screenshots.

**Files:** `PLAN.md` and this work log only.

**Acceptance checks:** a fresh temporary source copy, excluding `.git`, build outputs, machine metadata, and `docs/screens`, passes the workspace build and test commands.

**Verification:** a fresh `/tmp` source copy compiled successfully, then passed all workspace unit and doc tests with Cargo, Rustc, and Rustdoc explicitly pinned to the repository's Rust 1.97.1 toolchain. The copy deliberately excluded `docs/screens`; it is source-isolation evidence, not a substitute for committing the current worktree and testing that exact checkout.

## 2026-08-06 — Phase 0 / commit-backed verification

**Intent:** commit the implementation files required by Phase 0 without including user-provided screenshots or unrelated machine metadata, then validate the exact commit in a detached clean worktree.

**Files:** all Phase 0 source, policy, packaging, and original icon files; explicitly excluding `docs/screens` and `.DS_Store` files.

**Acceptance checks:** the commit contains `Cargo.lock` and every required Phase 0 implementation file, and its detached worktree passes the workspace build and test suite.

**Verification:** commit `65998ae` contains the Phase 0 implementation and `Cargo.lock`, without the screenshots or `.DS_Store` files. A detached worktree at that exact commit passed `cargo build --workspace --all-targets` and the full workspace unit and doc test suite under the pinned Rust 1.97.1 Cargo/Rustc/Rustdoc executables.

## 2026-08-06 — Phase 0 / virtual-history interaction check

**Intent:** verify GPUI can render and jump through the 100,000-row virtual history without operating-system input permissions.

**Files:** desktop root view and this work log.

**Acceptance checks:** the root view retains a `UniformListScrollHandle`; a GPUI visual test draws the same 100,000-row uniform-list configuration, requests item 99,999, redraws, and observes only the final visible range.

**Verification:** the visual test jumps strictly to item 99,999 and confirms the rendered range ends at 100,000 while its length remains smaller than the full list. GPUI 0.2.2's `logical_scroll_top_index` test helper is not used because it tracks retained child bounds, which virtual lists intentionally omit for off-screen rows.

## 2026-08-06 — Phase 1 / repository opening boundary

**Intent:** define a UI-independent repository classification and versioned recent-repository store before wiring asynchronous opening into the desktop shell.

**Files:** `git_domain`, `app_core`, `git_cli`, their manifests, and this work log.

**Acceptance checks:** Git discovery resolves a selected nested directory to its worktree root and absolute Git directory, classifies bare repositories as unsupported, and distinguishes non-repositories; a schema-versioned JSON store preserves recent paths and safely rejects unknown schema versions.

**Verification:** focused real-repository and persistence tests will cover nested worktrees, bare repositories, invalid paths, deduplicated recents, and unsupported store versions.

## 2026-08-06 — Phase 1 / desktop shell

**Intent:** replace the Phase 0 synthetic proof layout with a keyboard-accessible welcome and repository shell that opens selected directories without blocking first-window rendering.

**Files:** desktop actions, menu/keymap, root view, and this work log.

**Acceptance checks:** Command-O invokes GPUI's native directory picker; opening runs Git discovery in GPUI's background executor; valid repositories render a dedicated shell, while invalid and bare selections render actionable errors; the welcome view exposes recent repositories, an activity area, and diagnostics.

**Verification:** focused GPUI state tests and a manual packaged-app smoke test will cover welcome, error, and repository states.

## 2026-08-06 — Phase 1 / window geometry

**Intent:** restore and preserve the main window's safe geometry alongside recents without changing the store schema compatibility policy.

**Files:** `app_core`, desktop startup/window lifecycle, and this work log.

**Acceptance checks:** the store preserves recents while recording optional bounds; invalid or newer data remains untouched; startup uses stored bounds only when they meet the minimum window size; resize observations save the next restore geometry.

**Verification:** the repository-opening boundary has five real-Git tests for nested working trees, bare repositories, invalid directories, `-z` porcelain paths, bounded history, and process cancellation. The app-core store tests verify recents survive restart, unknown schemas are left untouched, and geometry coexists with recents. The desktop tests render the welcome shell and verify core keyboard actions. Native folder selection and external folder drops use GPUI's platform paths; repository discovery and diagnostics execute on the background executor. The model is one window per selected repository, with the originating window reused for recent and dropped paths.

## 2026-08-07 — Phase 6 / action discoverability and narrow windows

**Intent:** add native hover tooltips to the existing text-labelled action controls and prevent diagnostic chrome from crowding the main content at the minimum supported window width.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** every reusable action and mutation control exposes its label on hover; the inspector is omitted below its viable layout width; desktop tests continue to pass.

## 2026-08-07 — Phase 6 / Git executable rediscovery

**Intent:** refresh the visible Git diagnostic through the same fresh discovery path already used by every operation, so an install, upgrade, or removal is reflected without restarting the app.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** Refresh re-runs Git discovery and the focused desktop checks remain green.

## 2026-08-07 — Phase 6 / theme contrast review

**Intent:** bring muted light-mode text above the WCAG AA 4.5:1 normal-text threshold on its raised background while preserving the existing semantic token system.

**Files:** `crates/ui_kit/src/theme.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** measured primary, secondary, accent, and muted foreground pairs meet their applicable contrast targets in both light and dark appearances; UI-kit tests remain green.

**Verification:** `cargo fmt --all -- --check`, `cargo test -p ui_kit -p gitronimo-desktop --all-features` (13 tests), `cargo clippy -p ui_kit -p gitronimo-desktop --all-targets --all-features -- -D warnings`, and `git diff --check` passed. The only output was Cargo's upstream future-incompatibility notice for `block` and `proc-macro-error2`; it was not a project warning or test failure.

## 2026-08-07 — Phase 6 / initial keyboard focus

**Intent:** focus the root application view immediately after each window is created so global shortcuts work before the user first clicks the window content.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** both welcome and repository windows focus the existing root focus handle on creation; the GPUI shortcut test dispatches without manually focusing it first.

## 2026-08-07 — Phase 6 / safe executable-discovery timeout

**Intent:** time out only the side-effect-free `git --version` probe used to discover an installed executable; mutations and network operations remain explicitly cancellable rather than being forcibly killed.

**Files:** `crates/git_cli/src/lib.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** a stalled version probe returns a timed-out I/O error; normal Git discovery still works; no mutating Git path receives a hard timeout.

**Verification:** `cargo fmt --all -- --check`, `cargo test -p git_cli` (20 tests), and `cargo clippy -p git_cli --all-targets --all-features -- -D warnings` passed.

## 2026-08-07 — Phase 6 / local release metadata verification

**Intent:** verify the configured application identifier, name, icon, and native Apple Silicon architecture from a freshly packaged local `.app` without claiming Developer ID signing or notarization.

**Files:** `PLAN.md` and this work log.

**Acceptance checks:** the generated Info.plist matches the package metadata, the app reports arm64, and the bundle launches locally.

**Verification:** a fresh `cargo packager --release --formats app --manifest-path apps/desktop/Cargo.toml --out-dir target/release` generated `Gitronimo.app` with `CFBundleIdentifier=com.gitronimo.desktop`, display name `Gitronimo`, icon `gitronimo.icns`, and an arm64 executable. The freshly packaged app launched as PID 169. It remains ad-hoc signed, so Developer ID, notarization, Gatekeeper, and distributable artifact checks are deliberately still open.

**Intel verification:** `cargo build --release -p gitronimo-desktop --target x86_64-apple-darwin` completed successfully; `lipo -archs target/x86_64-apple-darwin/release/gitronimo-desktop` reported `x86_64`. The packaged local app remains arm64, so a separately packaged Intel or universal distributable artifact is still required.

**Universal verification:** the cross-compiled binary was packaged with `--target x86_64-apple-darwin --binaries-dir target/x86_64-apple-darwin/release`; `lipo -create` then produced a universal executable reporting `x86_64 arm64`. A local ZIP and `SHA256SUMS.txt` were produced under ignored `target/release-universal/`. They are intentionally unsigned and unnotarized local artifacts, not release uploads.

**Full gate:** `cargo fmt --all -- --check`, workspace Clippy with warnings denied, `cargo test --workspace --all-features` (39 unit tests), `cargo deny check`, and `git diff --check` passed. Cargo printed upstream future-incompatibility and duplicate-lock-entry notices, while `cargo deny` reported advisories, bans, licenses, and sources all OK.

## 2026-08-07 — Phase 6 / verified README screenshot

**Intent:** capture and publish a Gitronimo-owned welcome-state screenshot without including unrelated desktop content or repository data.

**Files:** `.gitignore`, `README.md`, `docs/screens/gitronimo-welcome.png`, `PLAN.md`, and this work log.

**Acceptance checks:** the screenshot is captured by Gitronimo's Core Graphics window ID, visually inspected, unignored for source control, and rendered from the README.

**Verification:** the running Gitronimo window was identified as Core Graphics window ID 57643 and captured with `screencapture -l`; visual inspection confirmed the image contains only the Gitronimo welcome state. The README link resolves, the image is no longer ignored, and `git diff --check` passed.

## 2026-08-07 — Phase 6 / accessibility-label audit

**Intent:** audit visible action names and the pinned GPUI accessibility surface, then document any framework limitation honestly with the available keyboard alternatives.

**Files:** `docs/troubleshooting.md`, `PLAN.md`, and this work log.

**Acceptance checks:** every shared action control retains a visible label and matching tooltip; the documented keyboard alternatives remain accurate; unsupported VoiceOver semantics are disclosed rather than claimed.

**Verification:** inspection found text labels on all reusable action and mutation controls, with native tooltips from the shared helpers. `Command-Shift-P` and `Command-/` match the keymap. A source search found no accessibility-role or programmatic-label API in pinned GPUI 0.2.2, so the documented limitation is accurate. Documentation target checks and `git diff --check` passed.

## 2026-08-07 — Phase 6 / release-note preparation

**Intent:** prepare concise, accurate 0.1.0 beta release notes for later publishing without claiming that an unsigned local artifact has been released.

**Files:** `CHANGELOG.md`, `README.md`, and this work log.

**Acceptance checks:** the notes state the shipped workflows, the safety/reliability behavior, and the known limitations consistently with the README and Phase 6 checklist.

**Verification:** the new changelog identifies version 0.1.0 beta, matches the README's scope and limitations, links to the documented accessibility limitation, and is linked from the README. Target checks and `git diff --check` passed.

## 2026-08-07 — Phase 6 / protected release workflow

**Intent:** make the credentialed release path reproducible in protected CI without adding a certificate, password, or notarization credential to the repository.

**Files:** `.github/workflows/release.yml`, `docs/packaging.md`, and this work log.

**Acceptance checks:** a `v*` tag triggers arm64 and x86_64 builds, creates a universal app, signs, notarizes, staples, Gatekeeper-assesses, checksums, and publishes the ZIP with the prepared changelog; all credentials enter only through named GitHub secrets.

**Verification:** Ruby's YAML parser accepted the workflow. Inspection confirms the workflow uses only the documented secret names, the same explicit Intel binary directory required by the local package build, universal `lipo` creation, `codesign`, `notarytool`, `stapler`, `spctl`, `shasum`, and `gh release create`. A local arm64 package simulation proved the required absolute output and binary paths create `target/release-arm/Gitronimo.app`; relative paths were corrected because cargo-packager resolves them from the desktop manifest. `actionlint` is not installed locally; a protected tag run remains required to prove the credentialed path.

## 2026-08-07 — Phase 6 / original workspace visual hierarchy

**Intent:** replace the sparse shell with a denser, original Gitronimo workspace hierarchy: product chrome, a purposeful welcome state, clearer sidebar grouping, and controls that expose the existing open and command-palette paths.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** the welcome state provides an obvious primary open action, quick keyboard discovery, recent-repository cards, and useful product guidance; shared chrome makes the repository state and primary actions legible without copying the reference product's assets, copy, or layout.

## 2026-08-07 — Phase 6 / working-copy action grouping

**Intent:** replace the linear working-copy action wall with compact, named branch and sync shelves, then give change groups and the commit composer a stronger shared surface hierarchy.

**Files:** `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** branch and network actions remain context-validated, working-copy mutations remain available, and status/commit/diff panels are easier to scan without adding a dependency or copying the reference application.

**Verification:** `cargo fmt --check`, `cargo test -p gitronimo-desktop --all-features` (11 tests), and `cargo clippy -p gitronimo-desktop --all-targets --all-features -- -D warnings` passed. A freshly built and packaged app was opened against the local Gitronimo checkout and visually inspected: the working copy has compact Branch, Sync, Changes, and Commit sections; unavailable upstream operations remain compact and explain themselves by tooltip. A fresh application-window capture was inspected and replaces Gitronimo's own welcome screenshot. The running packaged app was then reopened on the same local checkout.

**Full gate:** `cargo fmt --all -- --check`, workspace Clippy with warnings denied, `cargo test --workspace --all-features` (39 tests), `cargo deny check`, and `git diff --check` passed. Cargo and `cargo deny` reported upstream future-incompatibility and duplicate-lock-entry warnings only; dependency policy checks reported advisories, bans, licenses, and sources all OK.

## 2026-08-07 — Phase 7 / safe single-hunk staging foundation

**Intent:** add the smallest repository-scoped Git operation needed to stage one unstaged hunk, retaining Git's own patch validation and avoiding an interactive shell or a custom patch engine.

**Files:** `crates/git_cli/src/lib.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** the operation targets a repository-relative path with typed arguments, stages only the requested hunk in a temporary repository, and rejects unavailable/binary patch targets without altering the index.

**Verification:** `cargo fmt --check`, `cargo test -p git_cli` (21 tests), `cargo clippy -p git_cli --all-targets --all-features -- -D warnings`, and `git diff --check` passed. The integration test starts from two separated edits, stages only the first hunk, and proves that the other remains unstaged.

**UI verification:** the diff view provides one visible `Stage hunk N` control for each complete, unstaged text hunk. Controls are intentionally absent for staged, binary, truncated, or in-flight diffs, so unavailable operations cannot be triggered. `cargo test -p gitronimo-desktop --all-features` (11 tests) and desktop Clippy with warnings denied passed.

## 2026-08-07 — Phase 7 / safe single-hunk unstaging

**Intent:** mirror single-hunk staging with a staged-diff operation that reverses only the selected hunk in Git's index.

**Files:** `crates/git_cli/src/lib.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** one selected staged hunk becomes unstaged without modifying the working tree; the UI offers the control only for complete staged text diffs.

**Verification:** the shared temporary-repository integration test stages the first of two separated hunks, then reverses that first hunk from the index and proves both changes are still present only in the working-tree diff. `cargo test -p git_cli stages_only_the_requested_unstaged_hunk`, desktop Clippy with warnings denied, and formatting passed.

## 2026-08-07 — Phase 7 / explicit stash creation

**Intent:** provide a small reversible working-copy escape hatch using Git's native stash command, with a separate include-untracked action rather than silently changing what is preserved.

**Files:** `crates/git_cli/src/lib.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** creating a stash refreshes status; include-untracked is explicit; a temporary-repository test proves tracked and untracked behavior separately.

**Verification:** a temporary-repository integration test stashes tracked changes while retaining the untracked file, then explicitly stashes the remaining untracked file. `cargo test -p git_cli stashes_tracked_changes_and_optionally_untracked_files`, desktop Clippy with warnings denied, and formatting passed.

## 2026-08-07 — Phase 7 / apply latest stash

**Intent:** apply the latest stash with Git's native command while retaining the stash entry as the recovery path.

**Files:** `crates/git_cli/src/lib.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** applying restores the latest stash's working-copy change, retains the stash entry, refreshes status, and reports Git conflicts normally.

**Verification:** the stash integration test applies the latest stash, confirms its untracked file returns to the working copy, and confirms both stash entries remain. Formatting and the focused test passed; the packaged app was rebuilt and launched.

## 2026-08-08 — Phase 7 / confirmed latest-stash pop

**Intent:** require explicit confirmation before applying and removing the latest stash recovery entry.

**Files:** `crates/git_cli/src/lib.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** the first action only presents the consequence; confirmation runs Git's pop command and refreshes status.

**Verification:** the stash integration test confirms pop restores the tracked stash and removes its entry before testing the remaining include-untracked/apply path.

## 2026-08-08 — Phase 7 / confirmed latest-stash drop

**Intent:** require confirmation before permanently removing the latest stash recovery entry.

**Files:** `crates/git_cli/src/lib.rs`, `apps/desktop/src/main.rs`, `PLAN.md`, and this work log.

**Acceptance checks:** drop is not executed by the first click; confirmation removes only the latest stash and refreshes status.

**Verification:** the stash integration test creates a fresh latest stash and drops it, proving the final stash count is zero. Desktop tests, Clippy with warnings denied, and formatting passed.

## 2026-08-11 — UI-PLAN Phases 1–10 (UI parity)

**Intent:** complete the remaining `docs/UI-PLAN.md` phases on `feature/ui-improvements`: GPUI single-line inputs, toolbar quick open, working copy polish, welcome workflow hub, settings routing, Git progress streaming, secondary view two-pane consistency, and theme tokens.

**Files:** `apps/desktop/src/views/single_line_input.rs`, `toolbar.rs`, `sidebar.rs`, `commit_composer.rs`, `welcome.rs`, `workspace.rs`, `settings.rs`, `working_copy.rs`, `diff_viewer.rs`, secondary views (`reflog.rs`, `file_history.rs`, `blame.rs`, `compare.rs`, `worktrees.rs`, `submodules.rs`, `lfs.rs`, `tree.rs`), `apps/desktop/src/main.rs`, `apps/desktop/src/app_state.rs`, `crates/git_cli/src/lib.rs`, `crates/ui_kit/src/theme.rs`, `docs/UI-PLAN.md`, `docs/UI-IMPROVE.md`.

**Acceptance checks:**
- Toolbar, welcome sidebar, and commit composer use `single_line_input` (paste/caret; no osascript subject prompt).
- Quick Open overlay lists recents; Settings navigates to dedicated view; theme buttons call `apply_theme_mode`.
- Working copy shows +/- counts when numstat available; Description chip expands body row; diff lines highlight selection.
- `parse_git_progress_line` streams fetch/pull/push progress to `network_progress`.
- Reflog and other secondary views use `two_pane_view` / `view_panel_header`.
- Theme adds `toolbar_background`, `search_field_background`, `list_row_border`.

**Verification:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features` (102 tests), and `cargo deny check` all pass. `cargo run -p gitronimo-desktop` launches successfully.

## 2026-08-12 — Dialog buttons: unique element ids and accent primary style

**Intent:** fix the dead Pull button in the Pull dialog and give confirming actions an accent (blue) fill so they read as the primary choice next to Cancel/Close.

**Files:** `apps/desktop/src/views/components.rs`, `workspace.rs`, `working_copy.rs`, `toolbar.rs`, `commit_composer.rs`, `welcome.rs`, `commit_detail.rs`, `crates/ui_kit/src/theme.rs`, `apps/desktop/src/tests.rs`, `docs/UI-IMPROVE.md`, and this work log.

**Root cause:** button helpers used the bare label as the GPUI element id, so the dialog's `Pull` button and the toolbar's `Pull` button shared one interactive state. The toolbar hitbox is not hovered on mouse-up, so its handler cleared the shared pending mouse-down and the dialog button never produced a click. The same collision existed for `Fetch` (Remotes view), `Services` (Pull Requests view), and `Amend` (composer checkbox vs commit button).

**Acceptance checks:**
- Every button helper namespaces its element id (`action-button:`, `toolbar-button:`, `mutation-button:`, `context-menu-item:`, …).
- Clicking the rendered Pull button in the Pull dialog starts `git pull`; the toolbar Pull button still opens the dialog.
- Confirming actions (Pull, Push HEAD, Merge, prompt confirm, discard/stash/force-with-lease confirmations) render with `accent` fill and `accent_foreground` text; Cancel/Close stay neutral.
- Theme gains `accent_hover` and `accent_foreground` for both appearances.

**Verification:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass. Two new GPUI click tests (`the_toolbar_pull_button_opens_the_pull_dialog`, `confirming_the_pull_dialog_starts_the_network_command`) fail without the id fix.

## 2026-08-12 — Working Copy: drop the duplicated inline Back/Forward row

**Intent:** remove the stray `Back` button above the Working Copy panes and the empty band it created across the content area.

**Files:** `apps/desktop/src/views/working_copy.rs`, `apps/desktop/src/tests.rs`, `docs/UI-IMPROVE.md`, and this work log.

**Design:** `navigation_controls` rendered a full-width row above the two panes whenever the navigation stack was non-empty, duplicating the toolbar chevrons (which dispatch the same `NavigateBack`/`NavigateForward` actions) and reserving a row of empty space to the right of the button. Deleted the helper and its call site.

**Acceptance checks:** with navigation history present, the Working Copy content starts directly under the toolbar and no `Back` button renders; toolbar chevrons still navigate.

**Verification:** `navigation_history_does_not_add_an_inline_back_row` asserts the row is absent. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

## 2026-08-12 — Commit composer: flush section and an unmistakable primary action

**Intent:** drop the raised card fill behind the commit composer and make the Commit button read as active only when it can actually run.

**Files:** `apps/desktop/src/views/commit_composer.rs`, `apps/desktop/src/views/components.rs`, `apps/desktop/src/tests.rs`, `docs/UI-IMPROVE.md`, and this work log.

**Design:** the composer no longer paints its own background, margin, or rounded border; it is a flush section that shares the pane background and is separated from the file list by a single bottom border, so the subject/description fields are the only raised surfaces. `mutation_button` now mutes its label and drops hover/pointer feedback while disabled. `primary_window_action_button_with_reason` lets the composer state the missing precondition in the tooltip (`Stage at least one change to commit`, `Write a commit subject`, …) instead of one generic string; enabled it fills with `accent` and brightens on hover.

**Acceptance checks:** the composer shows no card fill; Commit stays a muted chip until a subject is written and something is staged (or amend is on), then turns accent blue; the disabled tooltip names the missing precondition.

**Verification:** `a_disabled_commit_button_names_what_is_missing` covers the reason matrix. `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, and `cargo deny check` pass.

## 2026-08-12 — Branch context menu

**Intent:** replace the six-item sidebar ref menu with branch context menu: same grouping, same wording (quoted ref names), cursor-anchored popup, submenus, disabled items that explain themselves, and the operations available on a branch.

**Groups:** pin · pull/push/force-push/sync · publish/push-to · track upstream · merge/rebase · archive · rename/delete · create branch/tag/PR · export/compare · reveal/copy.

**Files:** `crates/app_core/src/lib.rs` (per-repository branch organization), `crates/git_cli/src/lib.rs` (tag create/delete, upstream set/unset, archive export, remote-branch delete), `apps/desktop/src/app_state.rs`, `apps/desktop/src/main.rs`, `apps/desktop/src/views/working_copy.rs` (menu), `apps/desktop/src/views/workspace.rs` (anchored overlay + submenu), `apps/desktop/src/views/sidebar.rs` (pinned first, archived section), `apps/desktop/src/tests.rs`, `docs/UI-IMPROVE.md`, `docs/keyboard-shortcuts.md`.

**Acceptance checks:**
- The menu opens at the mouse position and stays inside the window.
- Local-branch menu offers pin/unpin, Pull…, Push…, Force Push with Lease…, Sync…, Publish, Push To ▸, Track Upstream Branch ▸, merge/rebase, archive/unarchive, rename, delete, create branch/tag/pull request, export files, compare, reveal in history, copy name — with quoted wording.
- Items that cannot run are disabled and say why (deleting HEAD, publishing a branch that already tracks, syncing a branch that is not checked out).
- Pinned branches sort first; archived branches move to their own sidebar section; both survive a restart.
- Tag delete uses `git tag --delete` and remote-branch delete uses `git push <remote> --delete`, instead of `git branch --delete`.

**Not implemented:** `Track Parent Branch` and `Create New Stacked Branch` belong to stacked-branch feature, and `Pin` ordering is app state rather than a Git concept.

**Verification:** `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features` (including `creates_and_deletes_tags_and_exports_an_archive`, `sets_and_unsets_a_branch_upstream`, `branch_organization_is_scoped_per_repository`), and `cargo deny check` all pass. Approach adapted from Working with Branches guide (UI structure) and local git_cli patterns; no AGPL code copied.
