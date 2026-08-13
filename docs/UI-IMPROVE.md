# UI/UX Improvement Plan

Status: **complete** — Tower 99% parity roadmap Phases 1–10 implemented; manual screenshot QA remains optional follow-up.

**Companion docs:** [`UI-PLAN.md`](UI-PLAN.md), [`README.md`](README.md), [`screens/README.md`](screens/README.md)

**Sources**

- Tower "Getting Started": <https://www.git-tower.com/help/guides/first-steps/get-started-with-tower/mac>
- Tower "Interface Overview": <https://www.git-tower.com/help/guides/first-steps/tower-overview/mac>
- Tower "A Basic Workflow": <https://www.git-tower.com/help/guides/first-steps/basic-workflow/mac>

**Copyright notice:** All screenshots in `docs/screens/tower-guides/` are © their respective owner (Tower, fournova / git-tower.com) and are kept under `docs/` for internal UI/UX reference and study only, per `AGENTS.md`. They are never shipped in the app bundle and never claimed as Gitronimo's own. "Tower" is a trademark of fournova.

---

## Goal

Gitronimo's UI must look professional and closely match the information architecture of a modern Git client so users can transfer muscle memory from tools like Tower. This document captures the UX patterns to adopt, mapped to Gitronimo's current views.

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

## 1. Interface overview (from "Tower Interface Overview")

### 1.1 Services — hosting accounts belong in the app

![Tower Services view](screens/tower-guides/overview-01-services.png)

**Guide text:** the Services view manages GitHub/Bitbucket/Beanstalk accounts right from within Tower; you can create and clone remote repositories or manage SSH public keys without leaving Tower.

**Adopt in Gitronimo:** not shipped. Gitronimo has no Services tab. GitHub personal-access-token connect/sign-out lives in **Settings**; Pull Requests remain a palette destination. Hosted-repo browse/clone from a Services pane is out of product.

### 1.2 Repositories — bookmarks and organization

![Tower Repositories view](screens/tower-guides/overview-02-repositories.png)

**Guide text:** bookmark local repositories, organize them in folders, and access general info; this is also where you create, add, or clone repositories.

**Adopt in Gitronimo:** upgrade the welcome screen into a Repositories view:

- recents and saved repositories with last-opened time and current branch;
- folder grouping (and a grouping toggle) for organization;
- "Add existing…", "Create new…", and a clearly labeled deferred "Clone…" entry point in one place;
- a badge/line showing each repo's open state or upstream.

### 1.3 Repository window — a hub for everything

![Tower repository window](screens/tower-guides/overview-03-repo.png)

**Guide text:** after opening a repository you have access to all commands and information around it: overview of modified files, commit changes, inspect previous commits, stashes, remote repositories, and more.

**Adopt in Gitronimo:** keep the single-window-per-repository model, but make every destination reachable from the sidebar (already true) and add the missing destinations the guide lists: a stash surface and a remotes surface.

### 1.4 Back / Forward — visible navigation controls

![Tower Back and Forward buttons](screens/tower-guides/overview-04-back-forward.png)

**Guide text:** the first toolbar items are Back and Forward buttons to step back or replay your path.

**Adopt in Gitronimo:** Gitronimo already tracks `navigation_back`/`navigation_forward` (actions `NavigateBack`/`NavigateForward`). The toolbar chevrons are the only navigation affordance — disabled when the stack is empty. The content area must not add its own Back/Forward row; that duplicated the toolbar and pushed both panes down by a row.

### 1.5 Sidebar — single source of navigation + remote activity

![Tower sidebar](screens/tower-guides/overview-05-sidebar.png)

**Guide text:** the sidebar controls which aspect of the repository you look at; selecting an item shows details on the right. At the bottom you are informed about current remote activities — e.g. a Push operation shows a progress indicator and status.

**Adopt in Gitronimo:**

- sidebar stays the primary navigation (current design already does this);
- workspace sidebar lists **Working Copy, History, Stashes, Settings** (Pull Requests, Branches Review, and Reflog remain reachable from the command palette);
- **left-click** a local/remote branch or tag opens History scoped to that ref (commit list + changeset detail); **double-click** a local or remote branch checks it out (`git switch` / `git switch --track`); **right-click** opens the branch context menu (Tower pattern), not an inline panel in Working Copy;
- add a **remote-activity footer** in the sidebar (or activity bar) showing in-flight fetch/pull/push progress and last result — wire it to the existing `NetworkOperation` state.

### 1.6 Working Copy — the commit hub

![Tower Working Copy](screens/tower-guides/overview-06-working-copy.png)

**Guide text:** (a) the left side lists currently modified files (with an option to see all of the project's files); selecting a file shows its diff on the right. (b) The top-left commit area wraps up changes into a new commit.

**Adopt in Gitronimo:**

- move the commit composer to sit directly above the file list in the Working Copy view (it is currently reachable via the composer action);
- add a **"Modified only / All files" list-mode toggle**;
- add a compact per-file **stage checkbox** (see 2.3);
- **multi-select** with Command-A, Shift-click, and Command-click; checkbox on a multi-selected row stages/unstages all selected files (see [`keyboard-shortcuts.md`](keyboard-shortcuts.md)).

### 1.7 Pull Requests — collaboration surface

![Tower Pull Requests](screens/tower-guides/overview-07-pull-requests.png)

**Guide text:** open pull requests are listed on the left; choosing one inspects the proposed change or lets you comment and manage it. A button starts a new pull request.

**Adopt in Gitronimo:** the Phase 9 PR view:

- left: list of open PRs (title, number, author, updated);
- right: detail with description, changed files, comments;
- actions: comment, merge with explicit method, create PR, checkout branch.

### 1.8 Commit History — changeset and tree modes

![Tower History](screens/tower-guides/overview-08-history.png)

**Guide text:** the History view shows the commit log; selecting a commit shows details on the right in two modes: (a) _Changeset_ — meta data (author, date, message) plus detailed changes; (b) _Tree_ — the project's complete file structure at that moment. Double-click opens the Commit Detail view.

**Adopt in Gitronimo:**

- Gitronimo's History already shows commit graph + per-commit diff; add a **Changeset / Tree mode toggle** on the detail pane;
- central commit list uses **full-width flat rows** (accent selection only on the active row), **multi-lane graph** with node dots, two-line author/date + hash/subject layout, and **plain typographic ref labels** (no pill backgrounds);
- add **double-click → Commit Detail view** (1.9) navigation.

### 1.9 Commit Detail — granular inspection

![Tower Commit Detail view](screens/tower-guides/overview-09-commit-details.png)

**Guide text:** the Commit Detail view lists the files touched by the commit; selecting a file shows its full changes.

**Adopt in Gitronimo:** formalize a Commit Detail view driven from History: file list (with per-file add/del line counts) on the left, full diff on the right. This is largely the current history detail pane; make it a first-class destination with its own navigation entry.

---

## 2. Basic workflow (from "A Basic Workflow")

### 2.1 Opening a repository

![Tower repositories list](screens/tower-guides/workflow-01-repositories.png)

**Guide text:** (a) open a repository already added, (b) add an existing repository from disk, (c) create a new repository (existing project or blank folder), (d) clone a remote repository.

**Adopt in Gitronimo:** the welcome/Repositories view must offer all four paths clearly. Gitronimo has open/add/clone; add "Create new repository" (init in a blank folder).

### 2.2 Editing files happens outside the app

![Tower edit files step](screens/tower-guides/workflow-02-edit-files.png)

**Guide text:** file editing happens in an external editor; Tower reacts to changes.

**Adopt in Gitronimo:** already handled by the filesystem watcher + debounce; keep status refresh snappy.

### 2.3 Checking the status and staging

![Tower working copy with diff](screens/tower-guides/workflow-03-working-copy.png)

![Tower staged changes](screens/tower-guides/workflow-04-staged.png)

**Guide text:** select a modified file to inspect its diff. Stage all changes in a file by ticking its **Status checkbox**, or stage individual parts in the diff view. The status-column icon moves right-to-left: an icon on the right means unstaged changes; an icon on the left means the file is staged.

**Adopt in Gitronimo:**

- keep the file list + diff panes (current design);
- add a **per-file checkbox** in the status list that toggles stage/unstage (single click), in addition to the current multi-select + stage button;
- when multiple files are selected (Command-A, Shift-click, or Command-click), a checkbox click stages or unstages **all selected files** and keeps the list selection;
- add a **staged/unstaged icon position** affordance (left = staged) or clear left/right badges so users instantly see the stage state;
- preserve partial staging of lines/hunks in the diff viewer (already implemented).

### 2.4 Committing

![Tower commit area](screens/tower-guides/workflow-05-commit.png)

**Guide text:** enter a short subject and optionally more text in the description, then click Commit.

**Adopt in Gitronimo:** Gitronimo's commit composer already covers subject + body + amend + sign-off. Keep the composer compact and permanently visible in the Working Copy view (see 1.6).

### 2.5 History confirms the commit

![Tower history after commit](screens/tower-guides/workflow-06-history.png)

**Guide text:** the new commit appears in the History view; select it to see details (date, author, exact changes).

**Adopt in Gitronimo:** already implemented (graph + detail). Ensure the newly created commit is selected/revealed after a commit.

---

## 3. Priority order

| #   | Improvement                                               | View         | Phase   |
| --- | --------------------------------------------------------- | ------------ | ------- |
| 1   | Commit composer visible above Working Copy file list      | Working Copy | now     |
| 2   | Modified-only / All-files list-mode toggle                | Working Copy | now     |
| 3   | Per-file stage checkbox + staged/unstaged badge           | Working Copy | now     |
| 4   | Visible Back/Forward toolbar buttons                      | Toolbar      | now     |
| 5   | Remember new commit OID for History (stay on Working Copy after commit) | History / WC | now     |
| 6   | Changeset / Tree mode toggle on commit detail             | History      | now     |
| 7   | Commit Detail destination (double-click)                  | History      | now     |
| 8   | Remote-activity progress in sidebar footer                | Sidebar      | now     |
| 9   | Stashes and Remotes sidebar destinations                  | Sidebar      | now     |
| 10  | Repositories view: folders, create-new, grouping          | Welcome      | Phase 9 |
| 11  | Services view (accounts + clone-from-service)             | Services     | Phase 9 |
| 12  | Pull Requests view (list, detail, comment, merge, create) | PRs          | Phase 9 |

### Implementation status

**Done (functional):**

- [x] Working Copy composer above file list, Modified/All Files toggle, per-file stage checkbox, staged/unstaged badges
- [x] Working Copy multi-select: Command-A select all visible files; toggle deselect/reselect on row click when all selected; batch stage/unstage via checkbox on multi-selection
- [x] Visible Back/Forward (Prev/Next) toolbar navigation with disabled state
- [x] Remember new commit OID for History reveal; stay on Working Copy after commit (no auto-redirect)
- [x] Commit Detail view with Changeset/Tree modes (double-click from History)
- [x] Stashes, Remotes, and Git LFS sidebar destinations
- [x] Repositories welcome view: grouped/flat recents, detail panel, Add/Create/Clone actions
- [x] Pull Requests list/detail workflow (Phase 9 baseline)
- [x] Welcome toolbar Bookmarks / Workflow tabs (Workflow placeholder)
- [x] Services tab and view removed; GitHub connect lives in Settings
- [x] Always-visible inline toolbar/sidebar search filtering repos and files
- [x] Sidebar remote-activity progress bar during fetch/pull/push + last-result footer when idle
- [x] Visual polish pass: welcome detail headers, commit focus border, diff tabs/hunk headers, HEAD badge, activity bar

**Done (2026-08-12 — shell chrome pass):**

- [x] Message history popup on the activity bar (successes / errors / confirms; refresh coalesced; scrollable)
- [x] Branch delete Cancel/Delete + unmerged **Could Not Delete Branch** force confirm (`AppConfirmDialog`)
- [x] Pinned branches persist across relaunch; flat at top of BRANCHES (no PINNED label); prefs RMW path-locked
- [x] Command palette expanded (Fetch/Pull/Push/Sync, staging, stash, settings, …) and scrollable
- [x] Network progress strip in the bottom activity bar during fetch/pull/push/sync

**Done (2026-08-12 — Tower-style Stashes core):**

- [x] Save stash dialog (message + include untracked); path-limited stash from WC **Stash selected…**
- [x] Apply stash dialog (delete after = pop; restore staging area = `--index`)
- [x] Stashes list shows date; selection loads paths + diff; Branch… from stash
- [x] Toolbar / palette / `Command-Shift-S` open save dialog; Apply opens apply dialog
- [x] Mutations refresh Working Copy and stash list (auto-stash / Snapshots / DnD deferred)
- [x] Sidebar HEAD pill includes `↑N` / `↓N` (toolbar tracking uses the same arrow-then-count order)

**Done (2026-08-11 — Tower interior pass):**

- [x] In-repo sidebar trimmed to Working Copy / History / Stashes / Settings; PRs, Branches Review, Reflog via palette
- [x] Branch/ref: left-click opens scoped History (list + detail); double-click checks out local/remote branches; right-click context menu (Checkout, History, Merge, …); sidebar selection ribbon vs muted HEAD badge (Tower)
- [x] Pull dialog: Remote Branch dropdown + Use Rebase Instead of Merge checkbox
- [x] Push HEAD dialog: Destination dropdown + Options (all tags, force-with-lease, recurse submodules, skip hooks)
- [x] History: full-width flat rows, multi-lane graph, plain ref labels, scope header + month groups
- [x] Working Copy file multi-select + batch checkbox staging

**Done (consistency pass — 2026-08-10):**

- [x] Shared layout constants (`LIST_ROW_HEIGHT`, `NAV_ROW_HEIGHT`, `PANEL_HEADER_HEIGHT`, etc.) in `components.rs`
- [x] Shared helpers: `section_header`, `detail_section`, `detail_row`, `view_panel_header`, `two_pane_view`, `head_badge`, `count_badge`
- [x] Full-width accent selection in sidebar nav (no rounded inset) and welcome repo list
- [x] Unified empty states (`centered_empty_state`) across Welcome, WC, History, PRs
- [x] Pull Requests two-pane layouts matching Stashes/Remotes
- [x] Panel headers standardized to 28px across Stashes, Remotes, Settings, PRs

**Partially done (works but visually or structurally short of Tower):**

- [~] §1.1 Welcome tabs — Bookmarks + Workflow; Services surface removed (GitHub in Settings); Workflow still placeholder
- [~] §1.4 Back/Forward — Prev/Next labels added; toolbar groups Fetch/Pull/Push/Sync, stash Apply/Save, Refresh, and inline search fields Tower-style
- [x] §1.5 Remote activity — in-flight progress bar in the bottom activity strip (left) and sidebar footer; Cancel available while running; last-result text when idle; Message history for past statuses/errors

- [~] §1.8 History inline detail — Changeset/Tree segmented toggle in History inspector; commit rows use Tower density (author/date/subject)
- [~] §4 Visual polish — interior pass: in-repo sidebar IA, WC composer rows, accent file selection, diff line backgrounds, stashes/remotes two-pane; OAuth/enterprise still deferred

**Done (interior views — 2026-08-10 pass):**

- [x] In-repo sidebar: WORKSPACE/BRANCHES/TAGS/REMOTES sections, nav icons, scrollable ref tree, no welcome clutter
- [x] Working Copy: Tower commit row order (subject + count, Stage All | Commit), square status badges, accent file-row selection
- [x] Toolbar subtitle: `View (branch - N Changed Files)`; Quick Open label
- [x] History: structured commit rows + segmented Changeset/Tree detail header
- [x] Diff viewer: added/removed line row backgrounds
- [x] Stashes / Remotes: list + detail two-pane layouts with empty states
- [x] Commit Detail / History changeset: file list left, diff right

### Remaining gaps (priority order)

| Gap                           | Section        | Notes                                                                               |
| ----------------------------- | -------------- | ----------------------------------------------------------------------------------- |
| Workflow view content         | §1.1           | Rail tab exists; full workflow surface still placeholder                            |
| OAuth / enterprise GitHub     | Delivery notes | Still deferred per `PLAN.md`                                                        |
| Deterministic remote progress | §1.5           | Strip + Cancel exist; finer Git stderr byte/object parsing still partial |

| IME-rich search/composer      | §4             | Inline search uses key-down capture; full EntityInputHandler fields deferred        |
| Settings dedicated view       | §1.3           | Settings view exists (appearance, identity, GitHub account, shortcuts)              |
| History ref label density     | §1.8           | Plain text labels; long ref names still truncate on narrow panes                    |

See also [`keyboard-shortcuts.md`](keyboard-shortcuts.md) for Working Copy selection behavior and [`desktop-shell.md`](desktop-shell.md) for activity bar, palette, pins, and confirms.

### Delivery notes

- GitHub Cloud uses a personal access token entered through an obscured macOS dialog and stored only in Keychain (Settings).
- Enterprise/self-hosted GitHub endpoint configuration and OAuth device flow remain separate product hardening work in `PLAN.md`.
- Pull Request mutations require the selected hosted repository and use explicit merge-method confirmation.
- The `t-vs-g.png` reference informed density, grouping, and two-pane hierarchy only; Gitronimo does not ship its screenshots, icons, copy, or branding.

## 4. Visual design notes

Professional Git-client polish targets for Gitronimo's own design system (`ui_kit`):

- **Density:** compact rows with clear separators; file lists show icons (status badge) + path + inline size/line-change counts.
- **Color use:** status signals (modified/staged/conflict/untracked) should be instantly distinguishable via the existing semantic tokens; do not overload with color.
- **Diff legibility:** preserve syntax-highlighted hunks, subtle background for added/removed lines, and a visible current-line indicator.
- **Commit clarity:** the commit area should make "subject vs body", staged file count, and the Commit affordance unambiguous.
- **Empty states:** every view needs a meaningful empty state (no changes, no history, no PRs, no accounts) rather than a blank panel.
- **Button hierarchy:** the confirming action of a dialog or confirmation panel (Pull, Push HEAD, Merge, Commit, Confirm discard) is accent-filled via `primary_action_button`; Cancel, Close, and secondary navigation stay on `raised_background`. Both variants brighten on hover. Disabled buttons drop hover and pointer feedback, mute their label, and explain the missing precondition in their tooltip.
- **Surfaces:** panels are flat and separated by borders, not by stacked fills. The commit composer shares the pane background so the subject and description fields are the only raised surfaces in the pane; a card fill there competed with the fields and the file list.
- **Branch context menu:** right-clicking a sidebar branch opens a Tower-style menu at the cursor (`apps/desktop/src/views/ref_context_menu.rs`). Items quote the branch name, group with separators, disable with reasons when they cannot run, and open ▸ flyouts for Push To and Track Upstream. Pin and Archive are persisted per repository: pinned branches sort flat at the top of BRANCHES (pin order) and stay there across relaunch; archived branches use their own ARCHIVED section. Delete uses Cancel/Delete (`git branch -d`); if Git refuses an unmerged tip, a second modal (“Could Not Delete Branch”) offers Cancel/Delete for force (`git branch -D`) instead of only flashing the activity bar.
- **History commit context menu:** right-clicking a History commit opens a Tower-grouped menu (`apps/desktop/src/views/commit_context_menu.rs`) with quoted short OIDs. Reset / Revert / Rebase / Delete require History scoped to the HEAD branch; Amend and Edit Message require the selected commit to be HEAD; Hard reset, Revert, and Delete use confirms.
- **Element ids:** button helpers namespace their GPUI ids (`action-button:{label}`, `toolbar-button:{label}`, …). Two elements sharing an id share interactive state, and the one that is not hovered swallows the other's click.
