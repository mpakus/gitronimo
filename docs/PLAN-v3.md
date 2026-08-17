# PLAN-v3.md — GitRonimo 3.0.0

**Status:** not started. Record only; do not mix with remaining PLAN-v2 `gix` fallbacks.  
**As of:** 2026-08-16  
**Product today:** **2.0.2** (`APP_VERSION` / packager). Architecture and crate rules stay in [`PLAN.md`](../PLAN.md) unless an item below supersedes them.

This is post-2.0.0 work. Remaining checkout / merge / rebase / stash / push / hooks migrations stay in [`PLAN-v2.md`](PLAN-v2.md) until `gix` orchestrates them. Do not start 3.0 while tagging or smoking 2.0.0.

---

## 2.0.0 already ships

Working Copy, History, branches, stashes, remotes, GitHub PRs (personal-access token), Workflow, command palette, Message history, merge/rebase/cherry-pick/revert/reset plus continue/abort and Interactive rebase, fetch/pull/push with cancel, in-app updater (GitRonimo Settings, on by default), LFS fetch/pull, stash extras, optional AI commit messages, default `gix` engine with system Git fallback.

WORKSPACE sidebar destinations are Working Copy / History / Stashes / Settings. Pull Requests, LFS, Worktrees, Submodules, Reflog, Blame, Compare, Branches Review, and Conflicts remain palette-only.

---

## 3.0.0 goal

**Findability + GitHub completeness + daily-loop polish** — without becoming another Git GUI clone or an IDE.

Same product rules as 2.0: no Git in `Render`; no GPUI in `git_domain`; no `gpui-component` without a new ADR; no third-party product names, icons, or layout copies in shipped UI. Prefer promoting views that already exist over new destinations.

---

## In scope vs out of scope

| In 3.0.0 | Still not a product goal |
|----------|--------------------------|
| Sidebar More for palette-only views that already exist | Dropping system Git |
| Conflicts as a first-class pause state (jump, not an in-app editor) | Built-in terminal or text editor |
| GitHub OAuth device flow + enterprise/self-hosted URL | GitLab / Bitbucket / Azure (3.1 if OAuth is solid) |
| Welcome clone list after GitHub connect | Windows / Linux packaging |
| Branches Review merge / rebase / publish | Graphite CLI, git-flow CLI |
| Working Copy Blame / File history on the file menu | Custom credential vault |
| History inline search; diff whitespace + hunk keys | Cloud sync of settings |
| Commit signing Settings; update-available reminder after a user-initiated check | Visual clone of another Git GUI |
| | Copying third-party assets or branding |
| | VoiceOver roles (GPUI 0.2.2), localization |

---

## Keep in 2.x (not 3.0)

- Finish `gix` fallbacks as they land upstream ([PLAN-v2 Phase A](PLAN-v2.md) unchecked list). Dual-backend tests stay required.
- Finer Git `%` progress parsing ([UI-IMPROVE.md](UI-IMPROVE.md)).
- Hand steps for the 2.0.0 tag.

Multi-window (`open_windows` in [`PLAN.md`](../PLAN.md) §9.1) only if the single-repo window becomes a real pain.

---

## Phase F — Findability

Views already exist under `apps/desktop/src/views/` (`pull_requests.rs`, `lfs.rs`, `worktrees.rs`, `submodules.rs`, `reflog.rs`, `blame.rs`, `compare.rs`, `branches_review.rs`, `conflicts.rs`). WORKSPACE nav in `sidebar.rs` is only Working Copy / History / Stashes / Settings.

- [ ] WORKSPACE **More** (or a compact second section): Pull Requests, Git LFS, Worktrees, Submodules, Branches Review. Use GitRonimo section labels already in the sidebar — do not copy another product’s activity-bar icon strip.
- [ ] Show **Conflicts** in that section only while merge / rebase / cherry-pick / revert is paused.
- [ ] Working Copy file context menu: **File history** and **Blame** for the selected path (palette commands already exist; menu today is copy / Reveal / Open).
- [ ] History inline author / path / message field (same parser as palette **History filter…**; no Git in `Render`).
- [ ] Auto-open Conflicts (or a sticky Working Copy banner that jumps there) when an operation pauses. Keep resolution as Take ours/theirs + mergetool — not an in-app three-way editor.

Suggested files: `views/sidebar.rs`, `working_copy.rs`, `history.rs`, `conflicts.rs`, `app_state.rs`, `docs/desktop-shell.md`, `docs/keyboard-shortcuts.md`.

---

## Phase H — Hosting (GitHub only)

Closes the open [`PLAN.md`](../PLAN.md) Phase 9 box (enterprise/self-hosted) and the README OAuth limitation. PAT-in-Settings stays as a fallback. Do not add a second Git host in 3.0.

- [ ] GitHub OAuth device flow; store the token in Keychain `com.gitronimo.github` (same item as today’s PAT, or document a migration). No token in prefs JSON or logs.
- [ ] Enterprise / self-hosted GitHub base URL in Settings (HTTPS). Refuse non-allowlisted hosts the same way AI-commit HTTP does for loopback.
- [ ] Welcome: clone from the connected account after GitHub connect (not only Settings + a clone URL).
- [ ] Optional extra, only after the three boxes above: **Suggest PR title/body** from `main...HEAD` using the existing AI Keychain and allowlist. Still opt-in; never auto-create the PR. Staged-diff Suggest stays unchanged.

No new HTTP crate without a current checklist item ([`dependency-policy.md`](dependency-policy.md)). Prefer typed `curl` like `hosting_github` / in-app updates.

---

## Phase P — Polish

- [ ] Branches Review: Merge / Rebase / Publish using the same confirms as the branch context menu ([`UI-PLAN.md`](UI-PLAN.md) deferred this).
- [ ] Settings: commit signing toggle + a line that explains why the last commit is unsigned (signed-*status* already ships). No new crates; do not put Git in Settings `Render`.
- [ ] Diff viewer: ignore whitespace, line wrap, keyboard jump to next / prev hunk. Skip word-level highlight until whitespace + hunk nav ship.
- [ ] Optional Working Copy folder grouping (collapse directories) on All Files; staging semantics stay path-based.
- [ ] History ref labels: wrap to a second line or tooltip with the full name — not smaller type that fails contrast ([UI-IMPROVE.md](UI-IMPROVE.md)).
- [ ] Updater follow-through: after a *user-initiated* check, persist “update available” on About / GitRonimo menu until installed. Optional Settings **Remind weekly**. Still **no check on launch** unless the user opts into the reminder.
- [ ] Focus / tab-order / 26px target audit on overlays (palette, About, confirms) and list rows ([`PLAN.md`](../PLAN.md) §8.4). Polish, not a new destination.

Keep AI commit messages as-is (opt-in, staged diff only, never auto-commit) except the optional PR Suggest under Phase H.

---

## Execution rules

Same as [`PLAN.md`](../PLAN.md) §23 and [`AGENTS.md`](../AGENTS.md):

1. One checkbox group at a time.
2. Work-log entry before coding.
3. XERJ reference search for GPUI/Git UX; GitComet is approach-only.
4. Gates: `fmt --check`, clippy `-D warnings`, `cargo test --workspace --all-features`, `cargo deny check`.
5. Bump `APP_VERSION` and packager version together when cutting 3.0.0.

Suggested order: **F → H → P**.

---

## Non-goals that stay non-goals

Do not add these under a “3.0” label without a new ADR:

- dropping the system Git fallback
- application-owned password vault
- Windows/Linux `.app` equivalents
- in-app editor or terminal
- cloud-synced preferences
- copying another product’s icons, screenshots, names, or layout measurements
- GitLab / Bitbucket / Azure (track as 3.1 after Phase H)
- VoiceOver roles until GPUI exposes them
- localization
- Graphite CLI / git-flow CLI
