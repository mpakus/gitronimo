# UI/UX patterns

Status: **complete** — UI-PLAN Phases 1–10 implemented; manual screenshot QA remains optional follow-up.

**Companion docs:** [`UI-PLAN.md`](UI-PLAN.md), [`README.md`](../README.md)

GitRonimo’s UI should look professional and use a familiar Git-client information architecture, with original branding and copy.

Current window layout: toolbar (top), sidebar + content + optional inspector (middle), activity bar (bottom).

```
┌──────────────────────────────────────────────┐
│ Toolbar                                      │
├──────┬───────────────────────────┬───────────┤
│ Side │  Content                  │ Inspector │
│ bar  │                           │           │
├──────┴───────────────────────────┴───────────┤
│ Activity bar                                  │
└──────────────────────────────────────────────┘
```

---

## 1. Surfaces

### 1.1 Hosting accounts

No Services tab. GitHub personal-access-token connect/sign-out lives in **Settings**. Pull Requests remain a palette destination. Hosted-repo browse/clone from a Services pane is out of product.

### 1.2 Repositories (welcome)

- recents and saved repositories with last-opened time and current branch;
- folder grouping (and a grouping toggle);
- Add existing…, Create new…, and Clone… in one place;
- a badge/line showing each repo’s open state or upstream.

### 1.3 Repository window

Single window per repository. Sidebar destinations: Working Copy, History, Stashes, Settings. Pull Requests, Branches Review, and Reflog remain reachable from the command palette.

### 1.4 Back / Forward

`NavigateBack` / `NavigateForward` are toolbar chevrons only — disabled when the stack is empty. The content area must not add its own Back/Forward row.

### 1.5 Sidebar

- **left-click** a local/remote branch or tag opens History scoped to that ref;
- **double-click** a local or remote branch checks it out (`git switch` / `git switch --track`);
- **right-click** opens the branch context menu, not an inline panel in Working Copy;
- remote-activity footer (or activity bar) shows in-flight fetch/pull/push progress and last result.

### 1.6 Working Copy

- commit composer sits above the file list;
- **Modified only / All files** list-mode toggle;
- per-file **stage checkbox**;
- **multi-select** with Command-A, Shift-click, and Command-click; checkbox on a multi-selected row stages/unstages all selected files (see [`keyboard-shortcuts.md`](keyboard-shortcuts.md)).

### 1.7 Pull Requests

- left: list of open PRs (title, number, author, updated);
- right: detail with description, changed files, comments;
- actions: comment, merge with explicit method, create PR, checkout branch.

### 1.8 History

- Changeset / Tree mode toggle on the detail pane;
- full-width flat commit rows, multi-lane graph, plain typographic ref labels;
- **double-click → Commit Detail**.

### 1.9 Commit Detail

File list (with per-file add/del counts) on the left, full diff on the right. First-class destination from History.

---

## 2. Basic workflow

- Welcome offers open, add, create (`git init`), and clone.
- File editing happens in an external editor; the watcher refreshes status.
- File list + diff; per-file checkbox; left = staged / right = unstaged affordance; hunk and line staging in the diff viewer.
- Composer: subject + optional body, then Commit. Optional **Suggest** (Settings **AI commit messages**) fills those fields from the staged diff and does not commit. Stay on Working Copy after commit; remember the new OID for History reveal.
- History graph + detail confirms the new commit.

---

## 3. Implementation status

**Done (functional):**

- [x] Working Copy composer above file list, Modified/All Files toggle, per-file stage checkbox, staged/unstaged badges. **Suggest** appears on the composer when Settings **AI commit messages** is on.
- [x] Working Copy multi-select: Command-A select all visible files; toggle deselect/reselect on row click when all selected; batch stage/unstage via checkbox on multi-selection
- [x] Visible Back/Forward (Prev/Next) toolbar navigation with disabled state
- [x] Remember new commit OID for History reveal; stay on Working Copy after commit (no auto-redirect)
- [x] Commit Detail view with Changeset/Tree modes (double-click from History)
- [x] Stashes, Remotes, and Git LFS sidebar destinations
- [x] Repositories welcome view: grouped/flat recents, detail panel, Add/Create/Clone actions
- [x] Pull Requests list/detail workflow (Phase 9 baseline)
- [x] Welcome toolbar Bookmarks / Workflow tabs (templates, Start / Finish / Sync)
- [x] Services tab and view removed; GitHub connect lives in Settings. Settings also has Git engine (gix / System Git), auto-stash, and AI commit messages (extra toggles default off). In-app updates live in **GitRonimo → Settings…** and default on.
- [x] Always-visible inline toolbar/sidebar search filtering repos and files
- [x] Sidebar remote-activity progress bar during fetch/pull/push + last-result footer when idle
- [x] Visual polish pass: welcome detail headers, commit focus border, diff tabs/hunk headers, HEAD badge, activity bar

**Done (shell chrome):**

- [x] Message history popup on the activity bar
- [x] Branch delete Cancel/Delete + unmerged **Could Not Delete Branch** force confirm
- [x] Pinned branches persist across relaunch; flat at top of BRANCHES
- [x] Command palette expanded and scrollable
- [x] Network progress strip in the bottom activity bar during fetch/pull/push/sync

**Done (stashes core):**

- [x] Save stash dialog (message + include untracked); path-limited stash from WC
- [x] Apply stash dialog (delete after = pop; restore staging area = `--index`)
- [x] Stashes list shows date; selection loads paths + diff; Branch… from stash
- [x] Mutations refresh Working Copy and stash list; stash files can be applied without dropping (drag onto Working Copy). Auto-stash is a Settings opt-in. Named snapshots keep the working copy.
- [x] Sidebar HEAD pill includes `↑N` / `↓N`

**Done (interior):**

- [x] In-repo sidebar trimmed to Working Copy / History / Stashes / Settings
- [x] Branch/ref: left-click scoped History; double-click checkout; right-click context menu; selection ribbon vs muted HEAD badge
- [x] Pull / Push HEAD dialogs with options
- [x] History: full-width flat rows, multi-lane graph, plain ref labels, scope header + month groups
- [x] Shared layout constants and helpers in `components.rs`

### Remaining gaps

| Gap | Notes |
| --- | ----- |
| OAuth / enterprise GitHub | Deferred per `PLAN.md` |
| Deterministic remote progress | Strip + Cancel exist; finer Git stderr parsing still partial |
| History ref label density | Long ref names still truncate on narrow panes |

See [`keyboard-shortcuts.md`](keyboard-shortcuts.md) and [`desktop-shell.md`](desktop-shell.md).

### Delivery notes

- GitHub Cloud uses a personal access token entered through an obscured macOS dialog and stored only in Keychain (Settings, service `com.gitronimo.github`).
- Optional AI commit API keys use a separate Keychain item (`com.gitronimo.ai-commit`). Suggest never auto-commits.
- Enterprise/self-hosted GitHub and OAuth device flow remain separate hardening work in `PLAN.md`.
- Pull Request mutations require the selected hosted repository and use explicit merge-method confirmation.

## 4. Visual design notes

- **Density:** compact rows; file lists show status badge + path + inline line-change counts.
- **Color:** status signals via semantic tokens; do not overload with color.
- **Diff:** hunk highlighting, subtle added/removed backgrounds, visible current-line indicator.
- **Commit:** subject vs body, staged file count, and Commit affordance must be unambiguous.
- **Empty states:** every view needs a meaningful empty state.
- **Button hierarchy:** confirming actions use `primary_action_button`; Cancel stays on `raised_background`.
- **Surfaces:** panels are flat and separated by borders. The commit composer shares the pane background.
- **Branch context menu:** cursor-anchored (`ref_context_menu.rs`); items quote the branch name; Pin/Archive persist; unmerged delete offers force confirm.
- **History commit context menu:** grouped menu with quoted short OIDs (`commit_context_menu.rs`).
- **Element ids:** button helpers namespace GPUI ids so two controls never share hover/click state.
