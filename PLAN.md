# PLAN.md — Native Rust + GPUI Git Client for macOS

> **Document status:** Implementation blueprint  
> **Prepared for:** Codex-driven development  
> **Target platform:** macOS desktop  
> **Primary language:** Rust  
> **UI framework:** GPUI  
> **Plan version:** 1.0  
> **Research date:** 2026-08-06

Post-1.0.0 work (`gix` as primary Git, updater, LFS/stash extras, optional AI commits) is in [`docs/PLAN-v2.md`](docs/PLAN-v2.md). Do not mix it into 1.0 tagging. Post-2.0.0 product work (findability, GitHub OAuth/enterprise, polish) is in [`docs/PLAN-v3.md`](docs/PLAN-v3.md). Remaining `gix` fallbacks stay in PLAN-v2.

---

## 1. Mission

Build a fast, polished, free, open-source Git desktop client for macOS, with original visual identity and implementation.

The application must be:

- a native compiled macOS desktop application;
- written primarily in Rust;
- rendered with GPUI, without HTML, JavaScript, Electron, or WebView;
- useful for daily Git work before advanced hosting-service integrations are added;
- safe around destructive Git operations;
- responsive on large repositories;
- keyboard-friendly and accessible;
- structured so Codex can implement it incrementally without turning the codebase into a monolith.

This project uses original branding, assets, copy, and interaction details. Do not copy another product’s icons, screenshots, names, or layout measurements into shipped code.

---

## 2. Product statement

### One-sentence description

A native macOS Git client that makes working-copy management, history navigation, staging, commits, branches, and remotes fast and understandable.

### Initial target user

A developer who already understands basic Git concepts but wants a visual tool for:

- reviewing repository state;
- staging files and hunks;
- composing commits;
- reading commit history;
- switching and managing branches;
- fetching, pulling, and pushing;
- understanding errors without opening Terminal.

### Product principles

1. **Git remains the source of truth.**
2. **Never hide the command that changed a repository.**
3. **Read operations should feel instant.**
4. **Destructive operations require explicit intent.**
5. **Keyboard and mouse workflows are equally important.**
6. **The UI may be inspired by established patterns, but the visual design must be original.**
7. **Do not block the UI thread with repository or filesystem work.**
8. **Every Git parser must be tested against real repositories and unusual filenames.**
9. **Advanced features must not destabilize core staging, commit, history, and branch workflows.**
10. **Codex changes must be small, reviewable, and tied to a checklist item.**

---

## 3. Scope

## 3.1 MVP scope

The first usable release must support:

- launching as a normal `.app`;
- opening an existing local Git repository;
- remembering recently opened repositories;
- repository sidebar and main workspace;
- working-copy status;
- staged and unstaged file lists;
- text diff display;
- stage and unstage by file;
- discard file changes with confirmation;
- commit subject and body;
- normal commit and amend commit;
- commit history;
- basic commit graph;
- commit details and changed files;
- local branches, remote branches, tags, and remotes;
- checkout existing branch;
- create and delete local branch;
- fetch, pull, and push;
- operation progress and cancellation where Git allows it;
- command/activity log;
- light and dark appearance;
- macOS menus and keyboard shortcuts;
- clear error reporting;
- release `.app` bundle for Apple Silicon and Intel;
- open-source license and contributor documentation.

## 3.2 Post-MVP scope

Add only after the MVP is stable:

- stage or discard individual hunks;
- stage or discard individual lines;
- stash list, create, apply, pop, drop;
- merge;
- rebase;
- cherry-pick;
- revert;
- reset with safe mode selection;
- reflog;
- file history;
- blame;
- worktrees;
- submodule support;
- conflict-resolution workflow;
- interactive rebase;
- Git LFS status and operations;
- commit signing UI;
- GitHub, GitLab, Bitbucket, and Azure DevOps integrations;
- pull-request UI;
- update system;
- localization;
- optional AI commit-message assistance.

## 3.3 Explicit non-goals for MVP

Do not implement these during MVP work:

- a custom Git transport protocol;
- a custom credential store;
- a replacement for OpenSSH;
- a complete pure-Rust implementation of Git;
- Windows or Linux packaging;
- a built-in terminal;
- a full text editor or IDE;
- cloud synchronization of settings;
- repository hosting;
- team collaboration features;
- visual parity with another commercial Git client;
- copying third-party icons, assets, wording, layout measurements, or branding.

---

## 4. Success criteria

The MVP is considered successful when all of the following are true:

- [x] A new user can open a repository and understand its current state without Terminal.
- [x] Status parsing handles spaces, Unicode, tabs, newlines, renames, untracked files, ignored files, conflicts, and submodule status safely.
- [x] The application never constructs a Git command through shell string concatenation.
- [x] A repository mutation cannot run concurrently with another mutation in the same repository.
- [x] The UI stays responsive while loading at least 50,000 commits.
- [x] The initial history page appears without loading the complete history.
- [x] A 10,000-file repository does not render all rows at once.
- [x] Staging, unstaging, committing, fetching, pulling, and pushing have integration tests.
- [x] Destructive actions display the affected repository and paths.
- [x] Every failure shows the Git command category, exit status, and useful stderr without exposing secrets.
- [x] The application can be built as a universal or separate `arm64` and `x86_64` macOS app.
- [x] A signed and notarized release workflow is documented.
- [x] No third-party copyrighted assets or trademarks are packaged with the application.

---

## 5. Technical decisions

## 5.1 UI framework

Use **GPUI** for the native desktop UI.

Pinned baseline:

```toml
gpui = "=0.2.2"
```

`gpui-component` was evaluated in Phase 0 and **removed** (ADR 0001). Build controls in `ui_kit` with GPUI primitives. Do not add `gpui-component` without a new ADR.

Rules:

- Commit `Cargo.lock`.
- Pin exact GPUI versions.
- Do not use `*`, an unpinned Git branch, or an unpinned Git revision.
- A GPUI upgrade must be its own pull request.
- Run the complete test suite and manual UI smoke checklist on every GPUI upgrade.
- Use the current stable Rust toolchain required by the pinned GPUI release.
- Commit `rust-toolchain.toml`.

## 5.2 Git implementation

Use a hybrid architecture:

### Compatibility path

Use the installed `git` executable for:

- status;
- staging and unstaging;
- commit;
- checkout and switch;
- branch mutation;
- fetch, pull, and push;
- merge and rebase later;
- hooks;
- credential helpers;
- SSH behavior;
- signing;
- filters;
- LFS;
- user configuration.

### Read-optimization path

Introduce `gix` only behind a trait and only after profiling demonstrates a benefit. Candidate uses:

- object reads;
- references;
- commit traversal;
- commit-graph reading;
- repository discovery;
- fast metadata and cache generation.

Do not use `gix` for repository mutations in the MVP. 2.0 uses `gix` as the default engine for discovery, HEAD, refs, status, history, trees/diffs, stage/unstage/commit, and HTTPS fetch/clone ([`docs/PLAN-v2.md`](docs/PLAN-v2.md), ADR 0003); checkout, merge, rebase, stash, push, hooks, and LFS still fall back to system Git.

### Git installation policy

MVP requires an installed Git executable.

At startup:

1. Check configured Git path.
2. Check `git` from the GUI-safe environment.
3. Check common macOS locations.
4. Run `git --version`.
5. Detect required command capabilities.
6. Show a setup screen when Git is unavailable.

Do not assume that a Finder-launched application receives the same `PATH` as Terminal.

## 5.3 Async and background work

Use GPUI’s task and executor facilities for UI-owned async work.

Rules:

- No filesystem scan, Git process, diff parsing, object traversal, or syntax highlighting on the UI thread.
- Use a bounded worker strategy for CPU-heavy parsing.
- Store spawned `Task` values when cancellation or lifetime matters.
- Use generation IDs so stale results cannot overwrite newer repository state.
- A closed window or repository session must cancel or detach from its pending work safely.
- Do not add a second async runtime until a demonstrated need exists.
- If Tokio is introduced for a specific dependency, isolate the runtime in the infrastructure layer and document why.

## 5.4 Filesystem watching

Use `notify` behind a repository watcher abstraction.

Watch:

- working tree;
- `.git/index`;
- `.git/HEAD`;
- `.git/refs`;
- `.git/packed-refs`;
- `.git/config`;
- operation state files such as merge, rebase, cherry-pick, and bisect markers.

Rules:

- Debounce bursts.
- Coalesce related events.
- Never refresh the complete repository for every individual event.
- Suspend redundant refreshes while the app itself performs a known Git mutation.
- Always perform a validating refresh after a mutation completes.
- Provide a polling fallback when native notifications are unreliable.

## 5.5 Packaging

Use `cargo-packager` behind project scripts to create the macOS app bundle.

Release outputs:

- `aarch64-apple-darwin`;
- `x86_64-apple-darwin`;
- optionally a universal binary;
- signed `.app`;
- notarized `.dmg` or `.zip`;
- checksums;
- release notes.

Do not put signing certificates or notarization credentials in the repository.

---

## 6. High-level architecture

```text
┌───────────────────────────────────────────────────────────────────┐
│                          macOS application                         │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │                         GPUI views                          │  │
│  │ Toolbar · Sidebar · Working Copy · History · Diff · Dialogs│  │
│  └──────────────────────────────┬──────────────────────────────┘  │
│                                 │ actions/events                   │
│  ┌──────────────────────────────▼──────────────────────────────┐  │
│  │                  Application state / use cases              │  │
│  │ RepositorySession · Navigation · Operations · Preferences  │  │
│  └───────────────┬───────────────────────────────┬─────────────┘  │
│                  │                               │                │
│  ┌───────────────▼──────────────┐  ┌─────────────▼─────────────┐  │
│  │       Git service trait      │  │ Repository watch service │  │
│  │ read · mutate · network      │  │ debounce · invalidate    │  │
│  └──────────┬───────────┬───────┘  └─────────────┬─────────────┘  │
│             │           │                         │                │
│  ┌──────────▼───────┐ ┌─▼───────────────┐ ┌──────▼────────────┐  │
│  │ Git CLI adapter  │ │ gix read adapter│ │ notify adapter    │  │
│  │ canonical path   │ │ later/optional  │ │ FSEvents/kqueue   │  │
│  └──────────────────┘ └─────────────────┘ └───────────────────┘  │
└───────────────────────────────────────────────────────────────────┘
```

### Dependency rule

Dependencies point inward:

```text
desktop_ui
    ↓
app_core
    ↓
git_domain
```

Infrastructure implements interfaces defined by inner crates:

```text
git_cli ───────► app_core traits
repo_watch ────► app_core traits
platform_macos ► app_core traits
```

The domain layer must not import GPUI, process APIs, macOS APIs, or filesystem-watcher types.

---

## 7. Proposed Cargo workspace

```text
/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── PLAN.md
├── AGENTS.md
├── README.md
├── CONTRIBUTING.md
├── LICENSE
├── SECURITY.md
├── CHANGELOG.md
├── deny.toml
├── rustfmt.toml
├── .github/
│   ├── workflows/
│   │   ├── ci.yml
│   │   ├── macos-build.yml
│   │   └── release.yml
│   └── ISSUE_TEMPLATE/
├── assets/
│   ├── icons/
│   ├── app-icon/
│   └── fonts/
├── packaging/
│   ├── packager.toml
│   ├── entitlements.plist
│   └── scripts/
├── apps/
│   └── desktop/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── app.rs
│           ├── actions.rs
│           ├── menus.rs
│           ├── keymap.rs
│           ├── windows/
│           └── views/
├── crates/
│   ├── git_domain/
│   ├── app_core/
│   ├── git_cli/
│   ├── git_graph/
│   ├── git_diff/
│   ├── repo_watch/
│   ├── persistence/
│   ├── platform_macos/
│   ├── ui_kit/
│   └── test_support/
└── tests/
    ├── fixtures/
    └── scenarios/
```

## 7.1 Crate responsibilities

### `git_domain`

Pure domain types:

- `RepositoryId`
- `RepositoryPath`
- `Oid`
- `Commit`
- `CommitSummary`
- `Signature`
- `Branch`
- `RemoteBranch`
- `Tag`
- `Remote`
- `WorktreeStatus`
- `FileChange`
- `ConflictState`
- `Diff`
- `DiffFile`
- `DiffHunk`
- `DiffLine`
- `GitOperation`
- `GitError`

No GPUI and no process execution.

### `app_core`

Application use cases and ports:

- open repository;
- close repository;
- refresh status;
- load history;
- load diff;
- stage;
- unstage;
- discard;
- commit;
- branch operations;
- remote operations;
- activity log;
- settings;
- cancellation;
- operation coordination.

Defines traits such as:

```rust
pub trait GitReadService {
    fn discover(&self, path: &Path) -> Result<RepositoryInfo, GitError>;
    fn status(&self, repo: &RepositoryPath) -> Result<WorktreeStatus, GitError>;
    fn refs(&self, repo: &RepositoryPath) -> Result<RefSnapshot, GitError>;
    fn history(
        &self,
        repo: &RepositoryPath,
        request: HistoryRequest,
    ) -> Result<HistoryPage, GitError>;
    fn diff(
        &self,
        repo: &RepositoryPath,
        request: DiffRequest,
    ) -> Result<Diff, GitError>;
}

pub trait GitMutationService {
    fn execute(
        &self,
        repo: &RepositoryPath,
        operation: MutationRequest,
        cancellation: CancellationToken,
    ) -> Result<MutationOutcome, GitError>;
}
```

Use async trait signatures only where the selected runtime and GPUI integration make them necessary. Do not make the domain model async by default.

### `git_cli`

Owns:

- Git executable discovery;
- environment construction;
- safe `Command` invocation;
- stdout and stderr capture;
- cancellation;
- progress parsing;
- parsers for machine-readable formats;
- capability detection;
- redaction;
- command audit records.

### `git_graph`

Owns:

- commit lane assignment;
- parent edges;
- merge edge routing;
- visible graph rows;
- graph color tokens;
- incremental graph layout;
- graph tests.

It does not render GPUI elements.

### `git_diff`

Owns:

- unified diff parsing;
- binary-file detection;
- rename and copy metadata;
- hunk models;
- patch construction;
- partial-stage patch validation;
- line-number mapping;
- syntax-token mapping later.

### `repo_watch`

Owns:

- watcher lifecycle;
- event normalization;
- debounce;
- invalidation categories;
- polling fallback;
- testable event coalescing.

### `persistence`

Owns:

- recent repositories;
- window geometry;
- sidebar expansion state;
- theme choice;
- selected Git executable;
- preferences;
- cached metadata;
- schema versioning and migrations.

Use a simple versioned JSON or SQLite store. Start with JSON unless queryable cache requirements justify SQLite.

### `platform_macos`

Owns macOS-only integrations that GPUI does not provide sufficiently:

- app activation behavior;
- Finder reveal;
- opening paths in external applications;
- Keychain integration only if later required;
- Dock menu;
- secure bookmark handling if sandboxing is introduced;
- platform appearance and accessibility hooks;
- app bundle metadata helpers.

### `ui_kit`

Wraps:

- buttons;
- icon buttons;
- split panes;
- list rows;
- tree rows;
- toolbar controls;
- dialogs;
- menus;
- context menus;
- text inputs;
- command palette;
- empty states;
- banners;
- progress indicators;
- colors, spacing, typography, and focus rings.

No domain-specific Git behavior belongs here.

### `test_support`

Owns:

- temporary repository builder;
- deterministic commit clock;
- author configuration;
- branch and merge fixture creation;
- unusual filename fixtures;
- fake Git services;
- process recording;
- snapshot sanitization.

---

## 8. UI information architecture

## 8.1 Window structure

```text
┌───────────────────────────────────────────────────────────────────────┐
│ macOS title bar / toolbar                                             │
│ Back · Forward · Repo · Fetch · Pull · Push · Search / Quick Actions │
├────────────────┬────────────────────────────┬─────────────────────────┤
│ Sidebar        │ Primary content            │ Inspector / details     │
│                │                            │                         │
│ Working Copy   │ File list or history       │ Diff / commit metadata  │
│ History        │ Commit graph               │                         │
│ Stashes        │                            │                         │
│                │                            │                         │
│ Branches       │                            │                         │
│ Tags           │                            │                         │
│ Remotes        │                            │                         │
├────────────────┴────────────────────────────┴─────────────────────────┤
│ Activity / operation status                                           │
└───────────────────────────────────────────────────────────────────────┘
```

The layout may borrow conventional Git-client patterns, but visual proportions, icons, typography, colors, and details must be original.

## 8.2 Primary views

### Repository welcome

- recent repositories;
- open existing repository;
- clone repository, post-MVP or late MVP;
- initialize repository, post-MVP;
- Git setup warning;
- drag-and-drop folder opening.

### Working Copy

- commit composer;
- staged group;
- unstaged group;
- untracked group;
- conflict group;
- selected file diff;
- stage/unstage controls;
- context menu;
- refresh state;
- current branch and upstream state.

### History

- virtualized commit rows;
- commit graph canvas;
- decorations for branch, tag, and remote refs;
- author;
- subject;
- relative or absolute time;
- search/filter;
- selected commit inspector;
- paginated loading.

### Branches

- local branches;
- remote branches;
- current branch;
- upstream;
- ahead/behind;
- checkout;
- create;
- rename;
- delete with confirmation;
- publish;
- merge/rebase later.

### Activity

- current operation;
- operation history;
- start/end timestamps;
- duration;
- exit status;
- safe command display;
- stdout/stderr summary;
- cancel button where supported;
- copy diagnostic information.

## 8.3 Command palette

Provide a keyboard-first palette.

Initial actions:

- open repository;
- switch repository;
- working copy;
- history;
- checkout branch;
- create branch;
- fetch;
- pull;
- push;
- refresh;
- focus commit composer;
- toggle sidebar;
- toggle inspector;
- open preferences;
- show activity log.

Use GPUI Actions for keyboard commands. Do not bind behavior directly to raw key events when an action can represent the intent.

## 8.4 Focus and accessibility

Every interactive control must have:

- visible keyboard focus;
- accessible label;
- tooltip where the icon alone is ambiguous;
- logical tab order;
- minimum target size;
- selected, disabled, loading, and error states;
- sufficient contrast in light and dark themes.

Do not encode file state or graph lanes using color alone.

---

## 9. Application state model

## 9.1 Global state

```text
ApplicationState
├── preferences
├── recent_repositories
├── open_windows
├── git_installation
├── theme
└── application_activity
```

## 9.2 Repository session

Each open repository gets an isolated session:

```text
RepositorySession
├── identity
├── path
├── repository_generation
├── status_snapshot
├── refs_snapshot
├── history_state
├── diff_cache
├── navigation_state
├── watcher
├── operation_queue
├── active_operation
└── errors
```

Rules:

- Do not keep one giant global mutable model.
- Each repository session owns its tasks and subscriptions.
- Increment `repository_generation` after a mutation or invalidating watcher event.
- Every async read records the generation at start.
- Ignore a result when its generation is stale.
- Read requests may run concurrently when safe.
- Mutation requests are serialized per repository.
- A mutation invalidates related caches when it completes.

## 9.3 Event categories

Normalize watcher and operation events into:

```rust
enum RepositoryInvalidation {
    Worktree,
    Index,
    Head,
    Refs,
    Config,
    OperationState,
    Everything,
}
```

This allows targeted refreshes instead of repeatedly running every Git query.

---

## 10. Git CLI contract

## 10.1 Process safety rules

- Never invoke `/bin/sh -c`.
- Never construct a single command string.
- Use `std::process::Command` or a safe async equivalent with individual arguments.
- Pass paths after `--` where supported.
- Set current directory explicitly.
- Set locale to stable values when parsing human-adjacent output.
- Prefer machine-readable output over localized text.
- Disable color for parsed output.
- Bound stdout and stderr collection for runaway processes.
- Stream network progress separately.
- Redact tokens, credentials, authorization headers, and credential-bearing URLs.
- Record command arguments as structured values.
- Treat repository paths and ref names as untrusted input.
- Do not parse shell-quoted text when a NUL-delimited format exists.

## 10.2 Initial command map

### Repository discovery

```text
git -C <path> rev-parse --show-toplevel
git -C <path> rev-parse --absolute-git-dir
git -C <path> rev-parse --is-bare-repository
```

### Status

```text
git -C <repo> -c color.ui=false status \
  --porcelain=v2 \
  --branch \
  --show-stash \
  -z
```

The parser must ignore unknown porcelain-v2 headers for forward compatibility.

### References

Use `git for-each-ref` with explicit field and record separators. Capture:

- full ref name;
- short ref name;
- object ID;
- object type;
- upstream;
- ahead/behind when separately calculated;
- HEAD marker;
- subject and date where useful.

### History

Start with a NUL/record-separated pretty format, topological ordering, and a page limit.

Capture:

- commit OID;
- parent OIDs;
- author name;
- author email;
- author timestamp and timezone;
- committer name;
- committer email;
- committer timestamp and timezone;
- subject;
- body;
- decorations, preferably loaded separately from refs.

Do not parse the default human-readable `git log` output.

### Diff

Use stable flags:

```text
git -C <repo> diff --no-ext-diff --no-color --binary -- <paths...>
git -C <repo> diff --cached --no-ext-diff --no-color --binary -- <paths...>
git -C <repo> show --format=... --no-ext-diff --no-color <oid>
```

External diff tools must not be invoked during internal parsing.

### Stage

```text
git -C <repo> add -- <paths...>
```

### Unstage

Preferred when supported:

```text
git -C <repo> restore --staged -- <paths...>
```

Fallback:

```text
git -C <repo> reset -q HEAD -- <paths...>
```

Handle unborn repositories separately.

### Commit

Use a temporary message file rather than putting the complete message in command arguments:

```text
git -C <repo> commit --file <temp-message-file>
```

Optional flags:

- `--amend`;
- `--signoff`;
- signing only when configured and explicitly requested.

Allow hooks to run. Present hook failures clearly.

### Branch checkout

Prefer capability-detected modern commands:

```text
git -C <repo> switch <branch>
git -C <repo> switch -c <new-branch> <start-point>
```

Provide a compatibility fallback to `checkout`.

### Network

```text
git -C <repo> fetch --progress <remote>
git -C <repo> pull --progress ...
git -C <repo> push --progress ...
```

Network commands must:

- stream progress;
- support cancellation;
- avoid freezing the UI;
- use existing credential helpers and SSH configuration;
- provide actionable authentication errors.

## 10.3 Capability detection

At application startup, build a `GitCapabilities` value rather than relying only on version numbers.

Examples:

- supports `switch`;
- supports `restore`;
- supports porcelain v2;
- supports `--show-stash`;
- supports force-with-lease;
- supports pathspec-from-file;
- supports specific progress formats.

Use behavior or help-output probing only where it is reliable and cached.

---

## 11. Mutation safety model

Classify every operation:

```rust
enum RiskLevel {
    ReadOnly,
    Reversible,
    Destructive,
}
```

Examples:

### Read-only

- status;
- log;
- diff;
- refs;
- fetch, although it changes remote-tracking refs, it does not alter the working tree.

### Reversible or normally recoverable

- stage;
- unstage;
- commit;
- branch create;
- checkout with clean worktree;
- push without force.

### Destructive or history-rewriting

- discard changes;
- delete unmerged branch;
- reset hard;
- clean;
- force push;
- rebase;
- amend a published commit;
- drop stash;
- delete tag or remote branch.

Requirements:

- Destructive actions use an explicit confirmation dialog.
- Confirmation text names the repository, ref, and paths.
- The primary destructive button uses a destructive style.
- The default focused button is cancel for irreversible operations.
- Never offer raw `--force`; use `--force-with-lease` where applicable.
- Before history-rewriting operations, create an internal recovery record containing old HEAD and relevant refs.
- Keep an operation journal without credentials.
- Do not claim an operation is undoable unless a tested recovery path exists.

---

## 12. Commit graph design

## 12.1 Data input

Each history row needs:

```rust
struct GraphCommit {
    oid: Oid,
    parents: SmallVec<[Oid; 2]>,
}
```

The graph layout module returns:

```rust
struct GraphRow {
    commit: Oid,
    commit_lane: usize,
    segments: Vec<GraphSegment>,
    lane_count: usize,
}
```

## 12.2 Initial algorithm

Use a deterministic lane algorithm:

1. Read commits in topological order.
2. Maintain active lanes, each associated with an expected OID.
3. Find or create the lane containing the current commit.
4. Replace the current lane with the first parent.
5. Add additional parents as new lanes.
6. Collapse lanes whose expected commits have been consumed.
7. Emit vertical, diagonal, merge, and commit-node segments.
8. Keep lane identity stable across paginated batches when possible.

Requirements:

- deterministic output;
- octopus merges supported even if visually simplified;
- no crossing minimization required in the first version;
- graph layout independent from pixel coordinates;
- snapshot tests for linear, branch, merge, nested merge, and octopus histories.

## 12.3 Rendering

Use:

- virtualized commit rows;
- a low-level GPUI canvas or custom element for graph segments;
- theme tokens for graph colors;
- selection and hover states independent of graph color;
- cached row geometry;
- no one-view-per-edge object explosion.

The graph must render only visible rows plus a small overscan range.

---

## 13. Diff model and viewer

## 13.1 Supported MVP cases

- text additions and deletions;
- multiple hunks;
- renamed files;
- copied files;
- new files;
- deleted files;
- mode changes;
- binary files;
- missing final newline;
- Unicode content;
- very long lines;
- large diffs with truncation controls;
- submodule summary.

## 13.2 Diff representation

```rust
struct DiffFile {
    old_path: Option<RepoPath>,
    new_path: Option<RepoPath>,
    status: DiffFileStatus,
    old_mode: Option<FileMode>,
    new_mode: Option<FileMode>,
    is_binary: bool,
    hunks: Vec<DiffHunk>,
}

struct DiffHunk {
    header: String,
    old_range: LineRange,
    new_range: LineRange,
    lines: Vec<DiffLine>,
}

enum DiffLineKind {
    Context,
    Addition,
    Deletion,
    NoNewlineMarker,
}
```

## 13.3 Viewer behavior

MVP:

- unified diff;
- line numbers;
- sticky file header;
- collapse file;
- collapse hunk;
- copy path;
- reveal file in Finder;
- open file in configured editor;
- whitespace toggle;
- loading and large-diff states.

Later:

- side-by-side view;
- syntax highlighting;
- word-level highlighting;
- hunk and line staging;
- image diff;
- merge conflict editor.

## 13.4 Partial staging

Do not implement partial staging until file-level staging is stable.

When implemented:

1. Build a minimal patch from selected hunks or lines.
2. Validate the patch.
3. Apply with `git apply --cached`.
4. Re-read index and worktree status.
5. If application fails, leave repository unchanged and show diagnostic output.
6. Include tests for shifted line numbers, no-newline markers, renames, and mixed selections.

---

## 14. Repository watcher strategy

Refresh matrix:

| Invalidation   | Refresh                           |
| -------------- | --------------------------------- |
| Worktree       | status, visible working-copy diff |
| Index          | status, staged diff               |
| Head           | status, refs, history head        |
| Refs           | refs, decorations, ahead/behind   |
| Config         | remotes, identity, settings       |
| OperationState | merge/rebase/conflict state       |
| Everything     | all repository snapshots          |

Debounce target:

- coalesce rapid events into one refresh window;
- immediately reflect app-initiated operation completion;
- prevent an endless watcher-refresh loop.

Watcher tests must use normalized synthetic events. Do not rely only on timing-sensitive real FSEvents in CI.

---

## 15. Persistence and cache

## 15.1 User settings

Persist:

- theme: system/light/dark;
- Git executable path;
- default pull strategy;
- fetch behavior;
- commit-title guideline;
- confirm-destructive-actions;
- diff whitespace preference;
- preferred external editor;
- window size and position;
- panel widths;
- sidebar expansion;
- recent repository list.

## 15.2 Cache policy

Caches are disposable.

Possible cache entries:

- commit summaries;
- graph rows;
- syntax tokens;
- avatar metadata later;
- repository display metadata.

Every cache key must include enough repository state to avoid stale results, such as:

- repository identity;
- HEAD OID;
- refs generation;
- query/filter;
- page cursor;
- app cache schema version.

The application must continue functioning when the cache is deleted or corrupted.

---

## 16. Error model

Use structured errors.

```rust
enum GitErrorKind {
    GitNotFound,
    UnsupportedGit,
    NotARepository,
    BareRepositoryUnsupported,
    Authentication,
    Authorization,
    Network,
    Conflict,
    DirtyWorktree,
    HookRejected,
    InvalidRef,
    ProcessFailed,
    ParseFailed,
    Cancelled,
    Io,
}
```

A user-visible error includes:

- short title;
- clear explanation;
- suggested next step;
- repository path;
- operation name;
- safe diagnostic details;
- copy-diagnostics button.

A diagnostic record may include:

- app version;
- macOS version;
- architecture;
- Git version;
- operation ID;
- command program;
- redacted arguments;
- exit code;
- bounded stderr;
- timing.

Never include:

- access tokens;
- passwords;
- private keys;
- authorization headers;
- credential-bearing URLs;
- complete environment dumps.

---

## 17. Logging and observability

Use `tracing`.

Log fields:

- operation ID;
- repository ID, not necessarily full path in telemetry;
- command category;
- duration;
- exit status;
- cancellation;
- parser record counts;
- cache hit or miss;
- refresh reason;
- UI generation ID.

Development builds may log more details locally.

The open-source default must not send telemetry. Any future telemetry must be opt-in, documented, and privacy-preserving.

---

## 18. Security checklist

- [x] No shell command construction.
- [x] No credentials stored by default.
- [x] Existing Git credential helpers are used.
- [x] SSH private keys are never read by the application.
- [x] URLs are redacted before logging.
- [x] Environment variables are allowlisted for diagnostic export.
- [x] Temporary commit-message and patch files use secure permissions.
- [x] Temporary files are removed on success and best-effort removed on failure.
- [x] Repository paths are canonicalized carefully without breaking symlinked worktrees.
- [x] Path arguments are separated with `--`.
- [x] Ref names are validated with Git, not a hand-written incomplete regex alone.
- [x] External diff and textconv execution is disabled for internal parsing unless explicitly supported.
- [x] Destructive operations require confirmation.
- [x] Force push defaults to force-with-lease.
- [x] Dependencies are audited with `cargo-deny`.
- [x] Rust advisories are checked in CI.
- [x] Release artifacts are signed and checksummed.
- [x] Notarization credentials exist only in protected CI secrets.

---

## 19. Testing strategy

## 19.1 Unit tests

Required for:

- status porcelain-v2 parser;
- refs parser;
- log parser;
- diff parser;
- path handling;
- redaction;
- capability detection;
- graph layout;
- watcher event coalescing;
- state reducers;
- risk classification;
- cache keys.

## 19.2 Parser fixtures

Include fixtures for:

- spaces in names;
- tabs in names;
- newlines in names;
- Unicode;
- invalid UTF-8 paths on Unix;
- rename;
- copy;
- deleted file;
- submodule;
- merge conflict;
- unborn repository;
- detached HEAD;
- no upstream;
- ahead and behind;
- SHA-1 repository;
- SHA-256 repository when supported;
- empty repository;
- bare repository detection.

Prefer generating binary fixtures from real Git commands rather than hand-authoring all outputs.

## 19.3 Integration tests

Use temporary repositories and the actual Git executable.

Scenarios:

- initialize and discover repository;
- initial commit;
- modify, stage, unstage;
- commit with subject and body;
- amend;
- rename;
- delete;
- conflict;
- branch create and checkout;
- branch delete;
- local bare remote;
- fetch;
- pull;
- push;
- rejected push;
- hook rejection;
- cancellation;
- watcher refresh.

Network tests must use local bare repositories, not public internet services.

## 19.4 GPUI tests

Use `#[gpui::test]` for:

- action dispatch;
- keyboard navigation;
- focus transitions;
- selection changes;
- opening dialogs;
- command palette;
- stale async result rejection;
- repository session closure;
- error banners.

Avoid screenshot-only tests as the primary correctness mechanism.

## 19.5 Visual regression tests

After core views stabilize:

- render deterministic component states;
- capture light and dark images;
- compare with tolerances;
- review intentional changes.

Use them for layout regressions, not domain behavior.

## 19.6 Performance tests

Benchmarks:

- parse status for 10,000 entries;
- parse 100 MB diff;
- layout 100,000 commit graph rows incrementally;
- render history viewport while scrolling;
- refresh after one changed file;
- open repository with many refs;
- cache cold versus warm.

Performance tests must report data, not merely pass without thresholds.

---

## 20. Performance budgets

Initial budgets are engineering targets, not marketing claims.

- Application shows its first window without waiting for repository loading.
- Repository shell appears immediately after selection.
- Status query and parsing for a normal repository should normally complete well below one second.
- History first page should load before complete history.
- Never fetch more than the first bounded history page for initial display.
- Lists with more than a few hundred rows must be virtualized.
- Diff rendering must be incremental or virtualized for large files.
- Watcher events should refresh only invalidated data.
- UI rendering must not allocate proportional to complete repository history on each frame.
- Commit graph layout should be reusable between frames.
- Cache and background work must have explicit memory limits.
- Large stdout and stderr must be bounded or streamed.

Add measured thresholds to CI only after repeatable baseline measurements exist.

---

## 21. Design system

Create original design tokens.

```rust
struct Theme {
    colors: ThemeColors,
    typography: Typography,
    spacing: Spacing,
    radii: Radii,
    shadows: Shadows,
    metrics: ComponentMetrics,
}
```

Required semantic color tokens:

- window background;
- panel background;
- raised background;
- sidebar background;
- border;
- separator;
- primary text;
- secondary text;
- muted text;
- accent;
- selection;
- focus ring;
- success;
- warning;
- danger;
- added line;
- removed line;
- modified line;
- conflict;
- graph lane palette.

Rules:

- Do not scatter raw color values throughout views.
- Do not copy another product’s exact palette.
- Support system appearance changes.
- Use macOS-appropriate typography and spacing while preserving an original identity.
- Use SVG icons with a compatible open-source license.
- Record icon attribution when required.
- Do not package extracted Apple or third-party product assets.

---

## 22. Phased implementation plan

# Phase 0 — Technical validation and project rules

## Goal

Prove that GPUI, packaging, virtualization, Git process execution, and the proposed architecture are viable before implementing product features.

## Checklist

### Repository setup

- [x] Create Cargo workspace.
- [x] Add `rust-toolchain.toml`.
- [x] Pin `gpui = "=0.2.2"`.
- [x] Pin `gpui-component = "=0.5.1"` for evaluation.
- [x] Commit `Cargo.lock`.
- [x] Add formatting, Clippy, tests, and `cargo-deny` CI.
- [x] Add Apache-2.0 or MIT/Apache-2.0 project license decision.
- [x] Add `AGENTS.md` with Codex rules.
- [x] Add architecture decision record directory.
- [x] Add dependency-update policy.

### GPUI spike

- [x] Open a macOS window.
- [x] Configure title bar and minimum size.
- [x] Add native application menu.
- [x] Dispatch GPUI actions from menu and keyboard.
- [x] Render three resizable panels.
- [x] Render a virtualized list with 100,000 synthetic rows.
- [x] Render a custom graph canvas in visible rows.
- [x] Switch light and dark themes.
- [x] Write one `#[gpui::test]`.
- [x] Verify app exits without leaked background tasks.

### Component-library gate

- [x] Do not wrap `gpui-component` controls; remove the library per ADR 0001.
- [x] Verify exact pinned versions compile together.
- [x] Verify keyboard focus and project-owned theme customization through the GPUI prototype.
- [x] Verify no need to import `gpui-component` outside `ui_kit`.
- [x] Decide: adopt, partially adopt, or remove.
- [x] Record decision in `docs/adr/0001-ui-components.md`.

### Git spike

- [x] Find Git from a Finder-launched app.
- [x] Run `git --version`.
- [x] Open a temporary repository.
- [x] Parse porcelain-v2 status with `-z`.
- [x] Load 500 commit records with explicit separators.
- [x] Stream fetch progress from a local remote.
- [x] Cancel a long-running process.
- [x] Confirm no shell is used.

### Packaging spike

- [x] Build `.app`.
- [x] Include app icon and metadata.
- [x] Run on Apple Silicon.
- [x] Build Intel target.
- [x] Document local signing.
- [x] Create unsigned development artifact in CI.

## Exit criteria

- [x] All spikes compile from a clean checkout.
- [x] GPUI version and component decision are recorded.
- [x] 100,000-row synthetic list remains responsive.
- [x] Git status fixture with unusual filenames parses correctly.
- [x] `.app` launches outside `cargo run`.
- [x] Architecture boundaries are represented by actual crates.

---

# Phase 1 — Application shell and repository opening

## Goal

Create a stable macOS application shell that can open and remember repositories.

## Checklist

- [x] Implement application startup.
- [x] Implement main window.
- [x] Implement app menu.
- [x] Implement global actions.
- [x] Implement system/light/dark theme.
- [x] Implement welcome view.
- [x] Implement folder picker.
- [x] Implement drag-and-drop folder opening.
- [x] Implement repository discovery.
- [x] Detect worktree root and Git directory.
- [x] Detect bare repository and show unsupported state.
- [x] Implement recent repository persistence.
- [x] Implement one window per repository or a clearly documented single-window model.
- [x] Restore window geometry.
- [x] Add empty, loading, and error states.
- [x] Add activity status area.
- [x] Add application diagnostics view.

## Exit criteria

- [x] User can launch `.app`, choose a repository, close, relaunch, and reopen it.
- [x] Invalid and non-repository folders show actionable messages.
- [x] Repository opening never blocks first window rendering.
- [x] Recent repository data survives schema-safe restart.
- [x] Core actions are keyboard accessible.

---

# Phase 2 — Working-copy read model

## Goal

Display an accurate, automatically refreshing repository status and file diff.

## Checklist

### Status

- [x] Implement porcelain-v2 parser.
- [x] Parse branch OID and name.
- [x] Parse detached HEAD.
- [x] Parse upstream.
- [x] Parse ahead/behind.
- [x] Parse stash count.
- [x] Parse ordinary changes.
- [x] Parse renames and copies.
- [x] Parse unmerged entries.
- [x] Parse untracked files.
- [x] Parse ignored files only on demand.
- [x] Parse submodule state.
- [x] Preserve non-UTF-8 paths safely.

### UI

- [x] Add sidebar shell.
- [x] Add Working Copy navigation item.
- [x] Add staged group.
- [x] Add unstaged group.
- [x] Add untracked group.
- [x] Add conflicts group.
- [x] Add status badges.
- [x] Add file selection.
- [x] Add multi-selection.
- [x] Add context menus.
- [x] Add refresh action.

### Diff

- [x] Implement unified diff parser.
- [x] Load unstaged file diff.
- [x] Load staged file diff.
- [x] Display text hunks.
- [x] Display binary state.
- [x] Handle rename metadata.
- [x] Handle missing final newline.
- [x] Add large-diff truncation and explicit load-more action.
- [x] Add copy path.
- [x] Add reveal in Finder.
- [x] Add open in external editor.
- [x] Drag Working Copy files to other macOS apps as file URLs.

### Watcher

- [x] Start watcher per open repository.
- [x] Normalize events.
- [x] Debounce bursts.
- [x] Refresh status after external file edits.
- [x] Refresh after external Git commands.
- [x] Stop watcher when repository closes.
- [x] Add polling fallback.

## Exit criteria

- [x] Working Copy accurately matches command-line Git for fixture repositories.
- [x] External file changes appear automatically.
- [x] Selecting a file shows the correct staged or unstaged diff.
- [x] Large lists and diffs do not freeze the UI.
- [x] Parser and watcher test suites pass.

---

# Phase 3 — Staging and committing

## Goal

Make the client useful for the most common daily local workflow.

## Checklist

### Staging

- [x] Stage one file.
- [x] Stage multiple files.
- [x] Stage all.
- [x] Unstage one file.
- [x] Unstage multiple files.
- [x] Unstage all.
- [x] Handle unborn repository.
- [x] Refresh status and diff after mutation.
- [x] Prevent concurrent mutations.
- [x] Show operation progress.
- [x] Show hook and index-lock failures.

### Discard

- [x] Discard tracked-file changes.
- [x] Delete untracked file through a safe OS trash strategy where possible.
- [x] Confirm affected paths.
- [x] Distinguish reversible trash from permanent deletion.
- [x] Refuse ambiguous or unsupported destructive cases.
- [x] Test symlinks and nested repositories.

### Commit composer

- [x] Commit subject input.
- [x] Commit body input.
- [x] Subject/body keyboard navigation.
- [x] Character guidance, not hard-coded policy.
- [x] Commit button enabled only with staged changes and valid message.
- [x] Commit using secure temporary message file.
- [x] Show author identity.
- [x] Detect missing user name/email.
- [x] Normal commit.
- [x] Amend commit.
- [x] Sign-off option.
- [x] Preserve draft when commit fails.
- [x] Clear draft only after successful commit.
- [x] Focus shortcut for composer.

## Exit criteria

- [x] Complete edit → stage → commit workflow works without Terminal.
- [x] Failed commit does not lose message text.
- [x] Repository cannot execute two mutations concurrently.
- [x] Discard paths and consequences are explicit.
- [x] Integration tests cover staging, unstaging, commit, amend, and hook rejection.

---

# Phase 4 — Commit history and graph

## Goal

Provide a fast, understandable history browser.

## Checklist

### History data

- [x] Define history request and page model.
- [x] Load bounded first page.
- [x] Load additional pages.
- [x] Load parent OIDs.
- [x] Load author and committer metadata.
- [x] Load commit subject and body.
- [x] Load ref decorations separately.
- [x] Support current branch history.
- [x] Support all refs history.
- [x] Support selected branch/tag history.
- [x] Cancel stale searches.

### Graph

- [x] Implement linear history layout.
- [x] Implement branch lanes.
- [x] Implement two-parent merges.
- [x] Implement octopus merge fallback.
- [x] Preserve lane continuity across pages.
- [x] Add graph snapshot tests.
- [x] Render graph with a custom GPUI element or canvas.
- [x] Virtualize rows.
- [x] Add overscan.
- [x] Cache graph rows.

### History UI

- [x] Add History navigation item.
- [x] Add commit row.
- [x] Add author, subject, time, and decorations.
- [x] Add selection.
- [x] Add keyboard navigation.
- [x] Add commit inspector.
- [x] Add changed-file list.
- [x] Add selected-commit diff.
- [x] Add copy OID.
- [x] Add search by subject, author, and OID.
- [x] Add reveal current HEAD.

## Exit criteria

- [x] First history page appears without full-history loading.
- [x] Scrolling tens of thousands of commits remains responsive.
- [x] Graph tests cover representative branch and merge shapes.
- [x] Selecting a commit shows correct metadata and changes.
- [x] Search cancellation prevents stale result replacement.

---

# Phase 5 — Branches, refs, and remotes

## Goal

Support normal branch and synchronization workflows safely.

## Checklist

### Ref browser

- [x] Load local branches.
- [x] Load remote branches.
- [x] Load tags.
- [x] Load remotes.
- [x] Display current branch.
- [x] Display upstream.
- [x] Display ahead/behind.
- [x] Group hierarchical ref names.
- [x] Persist expanded groups.
- [x] Add ref context menus.

### Local branch operations

- [x] Checkout branch.
- [x] Create branch from HEAD.
- [x] Create branch from selected commit/ref.
- [x] Rename local branch.
- [x] Delete merged local branch.
- [x] Confirm deleting unmerged branch.
- [x] Handle dirty-worktree failures.
- [x] Handle detached HEAD.

### Remote operations

- [x] Fetch default remote.
- [x] Fetch selected remote.
- [x] Pull current branch.
- [x] Push current branch.
- [x] Publish branch and set upstream.
- [x] Show progress.
- [x] Support cancellation.
- [x] Present authentication failures.
- [x] Present non-fast-forward rejection.
- [x] Add force-with-lease only behind explicit advanced action.
- [x] Refresh refs and status after completion.

## Exit criteria

- [x] User can create, switch, fetch, pull, publish, and push a branch.
- [x] Network work never blocks UI rendering.
- [x] Non-fast-forward and authentication failures are understandable.
- [x] Force push is not the default or easy accidental action.
- [x] Local-bare-remote integration suite passes.

---

# Phase 6 — MVP polish and public beta

## Goal

Turn the functional application into a reliable open-source beta.

## Checklist

### UX

- [x] Refine loading states.
- [x] Refine empty states.
- [x] Refine error states.
- [x] Add tooltips.
- [x] Add context-sensitive menu validation.
- [x] Add command palette.
- [x] Add back/forward navigation.
- [x] Add window-title repository state.
- [x] Add unsaved commit-draft handling.
- [x] Add keyboard shortcut reference.
- [x] Review focus behavior.
- [x] Review accessibility labels.
- [x] Review light/dark contrast.
- [x] Review small-window behavior.
- [x] Rework visual hierarchy and toolbar chrome.
- [x] Rework welcome and recent-repository surface.
- [x] Refine working-copy density and action grouping.

### Reliability

- [x] Add crash/panic report file without automatic upload.
- [x] Recover from corrupted preferences.
- [x] Recover from missing repository.
- [x] Recover from stale index lock with instructions, not automatic deletion.
- [x] Handle repository deletion while open.
- [x] Handle Git executable change.
- [x] Add operation timeout policy only where safe.
- [x] Bound all process output.
- [x] Verify cancellation cleanup.
- [x] Verify temp-file cleanup.

### Documentation

- [x] README with screenshots and scope.
- [x] Installation instructions.
- [x] Build instructions.
- [x] Architecture overview.
- [x] Contribution guide.
- [x] Security policy.
- [x] Code of conduct.
- [x] Issue templates.
- [x] Feature request template.
- [x] Troubleshooting guide.
- [x] Third-party notices.
- [x] Trademark statement.

### Release

- [x] Set application bundle identifier.
- [x] Finalize original app name and icon.
- [x] Build Apple Silicon release.
- [x] Build Intel release.
- [x] Build universal release if supported.
- [x] Sign with Developer ID.
- [x] Notarize with `notarytool`.
- [x] Staple notarization ticket.
- [x] Package `.dmg` or `.zip`.
- [x] Generate SHA-256 checksums.
- [ ] Run clean-machine smoke test.
- [ ] Publish release notes.
- [ ] Tag version.

## Exit criteria

- [x] All MVP success criteria pass.
- [ ] Release artifact opens normally under Gatekeeper.
- [ ] Core workflows pass on a clean macOS user account.
- [x] Known limitations are documented.
- [x] No critical or high-severity dependency advisories remain unaddressed.

---

# Phase 7 — Partial staging, stash, and safe history operations

## Goal

Expand local Git capability without compromising repository safety.

## Checklist

- [x] Stage hunk.
- [x] Unstage hunk.
- [x] Stage selected lines.
- [x] Discard hunk.
- [x] Discard selected lines.
- [x] Create stash.
- [x] Include untracked option.
- [x] Apply stash.
- [x] Pop stash.
- [x] Drop stash with confirmation.
- [x] Cherry-pick commit.
- [x] Revert commit.
- [x] Merge branch.
- [x] Abort merge.
- [x] Rebase branch.
- [x] Abort rebase.
- [x] Continue operation after conflicts.
- [x] Add recovery journal.
- [x] Add operation-state banner.
- [x] Add conflict overview.

## Exit criteria

- [x] Patch operations have extensive integration tests.
- [x] Every history-changing operation records pre-operation refs.
- [x] Abort and recovery behavior is tested.
- [x] The application accurately reflects in-progress Git operation state.

---

# Phase 8 — Advanced history and repository tools

## Checklist

- [x] File history.
- [x] Blame.
- [x] Reflog.
- [x] Restore lost branch from reflog.
- [x] Compare refs.
- [x] Browse tree at commit.
- [x] Export file at revision.
- [x] Worktree list/create/remove.
- [x] Submodule status/update/open.
- [x] Interactive rebase plan editor.
- [x] Squash/fixup/reword/drop.
- [x] Conflict-resolution UI.
- [x] External merge-tool integration.
- [x] Signed-commit status.
- [x] Git LFS status.

---

# Phase 9 — Hosting services and pull requests

## Checklist

- [x] Define provider-neutral service trait.
- [x] GitHub authentication.
- [x] Store tokens in macOS Keychain.
- [x] List repositories.
- [x] Clone from service.
- [x] List pull requests.
- [x] View pull-request details.
- [x] Create pull request.
- [x] Comment.
- [x] Checkout pull-request branch.
- [x] Merge with explicit method.
- [ ] Handle enterprise/self-hosted instances.
- [x] Add provider API rate-limit handling.
- [x] Add privacy documentation.

---

## 23. Codex execution protocol

Codex or OpenCode must follow this protocol for every implementation session.

## 23.1 Before coding

1. Read `PLAN.md`.
2. Read `AGENTS.md`.
3. Identify exactly one unchecked task or one tightly connected task group.
4. Inspect existing architecture before adding new modules.
5. State the intended files and acceptance tests in the work log or pull request.
6. Do not upgrade dependencies unless the task explicitly requests it.
7. Do not introduce a framework abstraction without a current use case.

## 23.2 During coding

- Keep changes small.
- Prefer adding tests before or with parser and mutation logic.
- Keep Git logic out of GPUI views.
- Keep GPUI types out of domain crates.
- Do not use shell execution.
- Do not add `unsafe` outside platform integration without written justification.
- Do not use `unwrap()` or `expect()` in normal runtime paths.
- Do not silently ignore Git stderr.
- Do not store secrets in logs, fixtures, or snapshots.
- Do not complete unrelated checklist items opportunistically.
- Preserve public interfaces unless the current task requires a migration.
- Add comments for invariants, not obvious syntax.
- Use typed domain models instead of passing unstructured strings.
- Use test repository builders instead of large checked-in `.git` directories.

## 23.3 Before marking a task complete

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
```

Also run task-specific integration or UI tests.

Then:

- update the corresponding checkbox;
- summarize changed behavior;
- list tests added;
- list known limitations;
- avoid claiming the entire phase is complete unless every exit criterion passes.

## 23.4 Codex stop conditions

Codex must stop and leave the checklist item incomplete when:

- a destructive behavior is not understood;
- a Git output format is being guessed;
- a GPUI API appears incompatible with the pinned version;
- a change requires an unplanned dependency upgrade;
- credentials would need insecure storage;
- a test cannot represent the claimed behavior;
- an operation has no safe error or cancellation path;
- the proposed implementation crosses architecture boundaries without an explicit ADR.

When stopped, create a focused note describing the blocker and the smallest research spike needed.

---

## 24. Suggested `AGENTS.md` rules

Create a companion `AGENTS.md` containing at least:

```markdown
# Agent Rules

- Read PLAN.md before editing code.
- Work on one PLAN.md checkbox group at a time.
- Never use shell command strings for Git.
- Never place Git/domain logic in GPUI Render implementations.
- Never import GPUI in git_domain.
- Do not add `gpui-component` (ADR 0001); use `ui_kit` primitives.
- Pin all framework versions and commit Cargo.lock.
- Add tests for every Git parser and mutation.
- Use local temporary repositories for integration tests.
- Do not log credentials or full environment variables.
- Do not add unsafe code without an ADR.
- Do not copy third-party product assets, text, icons, or proprietary design details.
- Run fmt, clippy, tests, and cargo-deny before completion.
```

---

## 25. First implementation issues, in order

These issues are intentionally small enough for Codex-assisted development.

1. **Initialize workspace and CI**
2. **Create minimal GPUI macOS window**
3. **Add actions, keybindings, and app menu**
4. **Create local design tokens and `ui_kit` boundary**
5. **Evaluate pinned `gpui-component` controls**
6. **Render three resizable panels**
7. **Render 100,000-row virtualized synthetic history**
8. **Render synthetic commit graph canvas**
9. **Package development `.app`**
10. **Implement Git executable discovery**
11. **Implement safe process runner**
12. **Create temporary repository test builder**
13. **Implement repository discovery**
14. **Implement porcelain-v2 status parser**
15. **Implement status integration tests**
16. **Create welcome and repository-open workflow**
17. **Create repository session state**
18. **Create Working Copy file groups**
19. **Implement unified diff parser**
20. **Display selected-file diff**
21. **Implement repository watcher and debounce**
22. **Implement stage/unstage mutation queue**
23. **Create commit composer**
24. **Implement commit and amend**
25. **Implement history record parser**
26. **Implement paged history state**
27. **Implement commit graph layout**
28. **Render virtualized history and graph**
29. **Implement ref parser and sidebar**
30. **Implement branch checkout/create/delete**
31. **Implement fetch progress**
32. **Implement pull and push**
33. **Add activity log and diagnostics**
34. **Complete MVP UX and accessibility audit**
35. **Create signed/notarized beta workflow**

Do not begin issue 22 before status, watcher, and mutation serialization are tested. Do not begin partial staging before file-level staging and diff parsing are stable.

---

## 26. Definition of done

A checklist item is done only when:

- [ ] Behavior is implemented.
- [ ] Public API and error behavior are clear.
- [ ] Unit tests exist where applicable.
- [ ] Integration tests exist for Git mutations.
- [ ] UI loading, empty, success, and failure states exist.
- [ ] Keyboard interaction works where applicable.
- [ ] Logging is structured and redacted.
- [ ] Documentation is updated.
- [ ] Formatting passes.
- [ ] Clippy passes with warnings denied.
- [ ] Workspace tests pass.
- [ ] Dependency policy passes.
- [ ] No unrelated refactor is bundled into the change.
- [ ] The relevant `PLAN.md` checkbox is updated.

A phase is done only when all checklist items and exit criteria in that phase pass.

---

## 27. Release checklist

Product version is `APP_VERSION` in `apps/desktop/src/views/about.rs` and `[package.metadata.packager] version` in `apps/desktop/Cargo.toml` (currently **2.0.1**). The Cargo workspace crate version is independent — do not bump it for a product release.

### Code quality

- [x] Clean release build.
- [x] No Clippy warnings.
- [x] All tests pass.
- [x] Dependency audit passes.
- [x] License audit passes.
- [x] Version updated.
- [x] Changelog updated.

### Functional smoke test

- [ ] Launch.
- [ ] Open repository.
- [ ] Detect dirty files.
- [ ] View staged and unstaged diff.
- [ ] Stage and unstage.
- [ ] Commit.
- [ ] View commit in history.
- [ ] Create branch.
- [ ] Checkout branch.
- [ ] Fetch.
- [ ] Pull.
- [ ] Push.
- [ ] Handle a failed command.
- [ ] Relaunch and restore repository.
- [ ] Test dark and light appearance.

### macOS distribution

- [ ] Bundle identifier correct.
- [ ] Version metadata correct.
- [ ] App icon correct at all sizes.
- [ ] Apple Silicon build tested.
- [ ] Intel build tested.
- [ ] Universal build tested if distributed.
- [ ] Developer ID signature valid.
- [ ] Hardened Runtime configured as needed.
- [ ] Entitlements minimized.
- [ ] Notarization accepted.
- [ ] Ticket stapled.
- [ ] Gatekeeper assessment passes.
- [ ] Archive checksum published.
- [ ] Clean-user-account launch tested.

### Open source

- [x] LICENSE present.
- [x] Third-party notices current.
- [ ] Source tag matches binary.
- [x] Build instructions reproduce release.
- [x] Known limitations published.
- [x] Security contact published.

---

## 28. Risks and mitigations

## Risk: GPUI breaking changes

**Mitigation:**

- exact version pins;
- committed lockfile;
- framework wrapper crate;
- dedicated upgrade pull requests;
- upgrade smoke checklist;
- avoid following Git `main` in normal development.

## Risk: GPUI documentation gaps

**Mitigation:**

- keep proof-of-concept examples in `examples/`;
- cite the exact GPUI version in internal docs;
- derive patterns from pinned source, not latest examples;
- add local documentation for adopted patterns;
- avoid clever framework-specific abstractions.

## Risk: `gpui-component` incompatibility or churn

**Mitigation:**

- library removed after Phase 0 (ADR 0001);
- project-owned primitives live in `ui_kit`;
- re-evaluate only through a new ADR and dedicated spike.

## Risk: Git edge cases

**Mitigation:**

- use installed Git as compatibility authority;
- machine-readable formats;
- NUL delimiters;
- real-repository integration tests;
- capability detection;
- preserve raw bytes for paths on Unix;
- never infer success from empty stderr.

## Risk: UI freezes on large repositories

**Mitigation:**

- background tasks;
- bounded queries;
- pagination;
- virtualization;
- incremental graph layout;
- cancellation;
- generation checks;
- profiling before optimization.

## Risk: destructive operation causes data loss

**Mitigation:**

- risk classification;
- mutation serialization;
- confirmations;
- recovery refs/journal;
- safe defaults;
- no raw force push;
- integration tests for abort and recovery.

## Risk: authentication complexity

**Mitigation:**

- initially defer to Git credential helpers and SSH;
- do not build custom token storage in MVP;
- provide clear environment and helper diagnostics;
- add GUI askpass only as a dedicated, security-reviewed feature.

## Risk: macOS GUI app has different environment from Terminal

**Mitigation:**

- explicit Git discovery;
- configurable Git path;
- known-path probing;
- diagnostics screen;
- do not depend blindly on shell initialization files.

## Risk: visual imitation creates legal or product confusion

**Mitigation:**

- original name, icon, palette, typography, and component design;
- no third-party product screenshots shipped in the app bundle;
- describe the product as “a Git client,” not an unofficial version of another app;
- include a trademark disclaimer where relevant.

---

## 29. Architecture decision records to create

- `0001-ui-component-library.md`
- `0002-git-cli-as-canonical-backend.md`
- `0003-gix-default-git-engine.md`
- `0004-gpui-task-model.md`
- `0005-repository-mutation-serialization.md`
- `0006-path-byte-handling.md`
- `0007-persistence-format.md`
- `0008-macos-packaging-and-signing.md`
- `0009-destructive-operation-recovery.md`
- `0010-telemetry-default-off.md`

Every ADR must state:

- context;
- decision;
- alternatives;
- consequences;
- rollback path.

---

## 30. Research notes and dependency baseline

The plan is based on the following current project facts as of 2026-08-06:

- GPUI is a hybrid immediate/retained GPU-accelerated Rust UI framework.
- GPUI remains pre-1.0 and warns that breaking changes can occur.
- GPUI uses Metal on macOS.
- GPUI provides actions, an event-loop-integrated async executor, test support, and virtualized uniform lists.
- GPUI `0.2.2` is published under Apache-2.0.
- `gpui-component 0.5.1` was evaluated against GPUI `0.2.2` in Phase 0 and removed (ADR 0001).
- Git porcelain-v2 status offers structured, extensible output, and `-z` supports safe machine parsing of filenames.
- `git rev-list` and related log commands support commit-ancestry traversal and bounded history queries.
- `gix` is useful as a Rust Git library but still has varying feature/stability levels across its crates; it should not replace system Git semantics in the MVP.
- `notify` supports macOS FSEvents or kqueue.
- `cargo-packager` supports macOS app bundles and signing/notarization configuration.
- Direct macOS distribution should use Developer ID signing and Apple notarization.

Primary references:

- GPUI: <https://gpui.rs/>
- GPUI source and README: <https://github.com/zed-industries/zed/tree/main/crates/gpui>
- GPUI API docs: <https://docs.rs/gpui/>
- Git status format: <https://git-scm.com/docs/git-status>
- Git diff: <https://git-scm.com/docs/git-diff>
- Git revision traversal: <https://git-scm.com/docs/git-rev-list>
- Gitoxide: <https://github.com/GitoxideLabs/gitoxide>
- Notify: <https://docs.rs/notify/>
- Cargo Packager: <https://docs.rs/cargo-packager/>
- Apple notarization: <https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution>

---

## 31. Final implementation directive

Build the product vertically, not by creating every abstraction first.

The first complete user journey is:

```text
Launch
→ Open repository
→ See changed files
→ Inspect diff
→ Stage file
→ Write commit message
→ Commit
→ See new commit in history
```

The second complete user journey is:

```text
Open repository
→ See branches and upstream
→ Create or checkout branch
→ Fetch
→ Pull
→ Commit
→ Push
```

Do not begin advanced history rewriting, hosting-service integrations, or AI features until both journeys are reliable, tested, and packaged as a normal macOS application.
