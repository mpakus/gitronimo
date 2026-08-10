# Implementation work log

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

**Design:** the user relaxed the Tower-asset rule; `AGENTS.md` now forbids copying third-party icons/glyphs/design assets into shipped code but permits attributed screenshots under `docs/` for UI/UX reference (never shipped). `PLAN.md` §28 risk updated to match. `PLAN.md` was also edited by the user to drop GitLab/Bitbucket/Azure DevOps auth checkboxes (GitHub only) and to mention OpenCode alongside Codex in the execution protocol.

**Files:**
- `.opencode/opencode.json` — registers `./skills` (11 Apache-2.0 GPUI skills from Zed) as opencode project skills.
- `AGENTS.md`, `PLAN.md` — asset/attribution policy update.
- `docs/UI-IMPROVE.md` — UI/UX improvement plan mapped from the Tower "Getting Started", "Interface Overview", and "Basic Workflow" guides.
- `docs/screens/tower-guides/` — 15 attributed screenshots downloaded from the Tower help guides (reference only).

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

**Why:** The current single-file desktop shell is the single largest blocker for the subsequent Tower-quality UI work-streams (inline commit composer, real toolbar, sidebar tree with icons, polished history, in-app dialogs, command palette). Each of those improvements needs a stable home in its own module; today every change collides in `main.rs`. `PLAN.md §7` already prescribes `apps/desktop/src/views/` and §7.1 lists `ui_kit` primitives that this split enables per-module work on.

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

**Intent:** prove the current source compiles outside the active, dirty worktree without copying the user-provided Tower screenshots.

**Files:** `PLAN.md` and this work log only.

**Acceptance checks:** a fresh temporary source copy, excluding `.git`, build outputs, machine metadata, and `docs/screens`, passes the workspace build and test commands.

**Verification:** a fresh `/tmp` source copy compiled successfully, then passed all workspace unit and doc tests with Cargo, Rustc, and Rustdoc explicitly pinned to the repository's Rust 1.97.1 toolchain. The copy deliberately excluded `docs/screens`; it is source-isolation evidence, not a substitute for committing the current worktree and testing that exact checkout.

## 2026-08-06 — Phase 0 / commit-backed verification

**Intent:** commit the implementation files required by Phase 0 without including user-provided Tower screenshots or unrelated machine metadata, then validate the exact commit in a detached clean worktree.

**Files:** all Phase 0 source, policy, packaging, and original icon files; explicitly excluding `docs/screens` and `.DS_Store` files.

**Acceptance checks:** the commit contains `Cargo.lock` and every required Phase 0 implementation file, and its detached worktree passes the workspace build and test suite.

**Verification:** commit `65998ae` contains the Phase 0 implementation and `Cargo.lock`, without the Tower screenshots or `.DS_Store` files. A detached worktree at that exact commit passed `cargo build --workspace --all-targets` and the full workspace unit and doc test suite under the pinned Rust 1.97.1 Cargo/Rustc/Rustdoc executables.

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
