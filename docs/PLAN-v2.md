# PLAN-v2.md — GitRonimo 2.0.0

**Status:** Phase A prefer-gix complete; Phase E complete (LFS, stash partial apply, auto-stash, snapshots); Phase D complete (in-app updates, now on by default in GitRonimo Settings); Phase G complete (optional AI commit messages). Checkout/merge/rebase/push/hooks stay on system Git.  
**As of:** 2026-08-16  
**Product today:** **2.0.4** (`APP_VERSION` / packager). Architecture and crate rules stay in [`PLAN.md`](../PLAN.md) unless an item below supersedes them.

This is post-1.0.0 work. Do not start it while tagging or smoking 1.0.0. Hand steps for the 1.0.0 tag remain in [`todo-v1.md`](todo-v1.md). Remaining boxes below are `gix` fallbacks. Post-2.0.0 product work is [`PLAN-v3.md`](PLAN-v3.md).

---

## 1.0.0 already ships

Working Copy, History, branches, stashes, remotes, GitHub PRs (personal-access token), Workflow templates, command palette, Message history, merge/rebase/cherry-pick/revert/reset as operations plus the Working Copy continue/abort banner and Interactive rebase view, fetch/pull/push with cancel, notarized `v*` release workflow.

Git is the installed executable via `git_cli` (typed `Command` args). Git LFS **status** exists. [`PLAN.md`](../PLAN.md) forbade `gix` mutations in the MVP; Phase A below replaces that for 2.0.

---

## 2.0.0 goal

Make **[gitoxide](https://github.com/GitoxideLabs/gitoxide) `gix`** the main internal Git engine, keep **system Git as fallback**, then add updater, LFS/stash extras, and optional AI commit messages — without destabilizing the 1.0 daily loop (status, stage, commit, history, branch, fetch/pull/push).

Same product rules as 1.0 except the Git backend: no Git in `Render`; no GPUI in `git_domain`; no `gpui-component` without a new ADR; no third-party product names, icons, or layout copies in shipped UI.

Application code consumes the **`gix` library crate**, not the `gix` CLI and not `gitoxide-core`. Track capability against [crate-status.md (`gix`)](https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md#gix).

---

## In scope vs out of scope

| In 2.0.0 | Still not a product goal |
|----------|--------------------------|
| `gix` as primary Git backend | Dropping system Git (it stays as fallback) |
| System Git fallback for missing `gix` workflows | Custom Git protocol or replacing OpenSSH |
| In-app updater | Custom credential store (Keychain + Git helpers stay) |
| Git LFS fetch/pull UI | Windows / Linux packaging |
| Stash drag-and-drop / snapshots / auto-stash | Built-in terminal or text editor |
| Optional AI commit-message assistance | Cloud sync of settings |
| | Visual clone of another Git GUI |
| | Copying third-party assets or branding |
| | OAuth, other Git hosts, VoiceOver, localization, Graphite/git-flow CLI |

---

## Phase A — `gix` as main Git, system Git as fallback

Supersedes the 1.0 read-only `gix` note in [`PLAN.md`](../PLAN.md) (“introduce `gix` only after profiling”; “do not use `gix` for mutations in the MVP”). Write an ADR before the first `gix` dependency lands.

### Architecture

- [x] ADR: `gix` is the default backend; `git_cli` remains the fallback. `git_domain` stays types-only (no `gix`, no GPUI).
- [x] Port in `app_core` (existing mutation/query traits, or a thin `GitBackend`) so desktop code does not call `gix` or `Command` directly.
- [x] New crate `git_gix`: `gix` adapter only. Pin `gix` exactly; commit `Cargo.lock`; `cargo deny check` must pass (gitoxide is Apache-2.0 / MIT).
- [x] Keep `git_cli` for fallback. Never build Git commands with shell strings.
- [x] Settings: “Use system Git” override. Default is `gix`. Fallback is automatic when `gix` lacks the workflow or returns an unrecoverable error; log a redacted reason via `set_activity`.
- [x] Tests: temporary repositories; for each migrated operation, assert `gix` and `git_cli` agree on the `git_domain` model (or document a known deviation).

### What `gix` already has (prefer these first)

From [crate-status plumbing overview](https://github.com/GitoxideLabs/gitoxide/blob/main/crate-status.md): clone/fetch/ls-refs, commit and low-level ref/object/index mutation, status, blob/tree diff, merge-base / rev-parse / rev-walk, worktree streaming.

- [x] Repository discovery, refs, HEAD, ahead/behind (unborn `HEAD` follows porcelain: branch name when Git prints it, not `(initial)`)
- [x] Status (index vs worktree) and untracked listing
- [x] History / rev-walk and commit metadata
- [x] Tree and blob reads; unified diff of trees / worktree
- [x] Stage / unstage / commit (low-level index + commit)
- [x] Fetch and clone (HTTPS via `gix` remotes; SSH stays on system Git until `gix` push/SSH plumbing is usable)

### Fallback until `gix` orchestrates the workflow

crate-status still marks these incomplete. Keep them on `git_cli` until plumbing exists; do not reimplement porcelain in the desktop:

- [ ] checkout / switch / restore / reset
- [ ] merge / cherry-pick / revert (and continue/abort)
- [ ] rebase (including interactive todo)
- [ ] stash, apply, am
- [ ] push (and `file://` / `ssh://` self-contained clone/fetch)
- [ ] hooks, signed commits/tags, Git LFS smudge/fetch, mergetool, submodules, worktree add/remove

When a later `gix` release gains a workflow, migrate it in a dedicated work-log unit with the same dual-backend tests.

### Product constraints

- Cancellation and progress must still work for fetch/clone (gix interruptible operations).
- Credentials: reuse Git credential helpers / Keychain; do not store passwords in app JSON.
- Do not log credentials, environment dumps, or unredacted Git/`gix` output.
- No `unsafe` in `git_gix` without an ADR.

---

## Phase D — In-app updates

- [x] Check GitHub Releases for a newer notarized zip
- [x] Prompt, download, verify SHA-256, replace the `.app` safely
- [x] Never run unsigned bits; respect Gatekeeper
- [x] Off-by-default or explicit Settings toggle until the path is audited
- [x] No telemetry

---

## Phase E — LFS and stash extras

LFS fetch/pull and stash mutations stay on **system Git** until Phase A has a `gix` path (stash plumbing is still open upstream).

- [x] Git LFS fetch/pull UI (status view already ships; CLI still works)
- [x] Stash drag-and-drop (partial apply)
- [x] Auto-stash around switch/pull when the user opts in
- [x] Stash snapshots (named, non-destructive)

Keep stash save/apply/pop/drop/branch from 1.0. Do not put Git in stash-row `Render`.

---

## Phase G — AI commit messages (optional)

- [x] Opt-in Settings; no default network
- [x] Prompt uses only staged diff the user can see; never send secrets, PAT, or full repo
- [x] User must edit/accept before commit (gix or system Git, whichever backend is active)
- [x] Failure is a composer no-op with a redacted error

Shipped without a new HTTP/AI crate: typed `curl` in `apps/desktop/src/ai_commit.rs`, prompt/JSON/parse in `app_core`, Keychain `com.gitronimo.ai-commit`. HTTPS (including the empty OpenAI default) requires an API key; HTTP is allowlisted only on `127.0.0.1` / `localhost` / `[::1]`.

Requires a current checklist item before adding any HTTP/AI crate ([`dependency-policy.md`](dependency-policy.md)).

---

## Execution rules

Same as 1.0 [`PLAN.md`](../PLAN.md) §23 and [`AGENTS.md`](../AGENTS.md):

1. One checkbox group at a time.
2. Work-log entry before coding.
3. XERJ reference search for GPUI/Git UX; GitComet is approach-only. For `gix` APIs, prefer gitoxide docs and crate-status over guesswork.
4. Gates: `fmt --check`, clippy `-D warnings`, `cargo test --workspace --all-features`, `cargo deny check`.
5. Bump `APP_VERSION` and packager version together when cutting a later release.

Suggested order: **A → E → D → G** (all checkbox groups done). Remaining boxes are `gix` fallbacks only; 3.0 product work is [`PLAN-v3.md`](PLAN-v3.md).

---

## Non-goals that stay non-goals

Do not add these under a “2.0” label without a new ADR:

- dropping the system Git fallback
- application-owned password vault
- Windows/Linux `.app` equivalents
- in-app editor or terminal
- cloud-synced preferences
- copying another product’s icons, screenshots, names, or layout measurements
