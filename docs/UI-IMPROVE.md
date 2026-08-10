# UI/UX Improvement Plan

Status: **implemented** — screenshots below are third-party product captures used to study professional Git-client UI/UX. Gitronimo keeps its own original branding, icons, palette, and typography; none of these assets are copied into shipped code.

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

**Adopt in Gitronimo:** this is the Phase 9 hosting-services surface. A Services view (or welcome-screen section) that:

- lists connected hosting accounts with provider name and login;
- sign-in and sign-out per provider;
- shows the auth state (connected / expired token / rate-limited);
- seeds "Clone repository…" with the account's repositories.

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

**Adopt in Gitronimo:** Gitronimo already tracks `navigation_back`/`navigation_forward` (actions `NavigateBack`/`NavigateForward`). Render them as visible toolbar buttons (disabled when empty), not only keyboard shortcuts.

### 1.5 Sidebar — single source of navigation + remote activity

![Tower sidebar](screens/tower-guides/overview-05-sidebar.png)

**Guide text:** the sidebar controls which aspect of the repository you look at; selecting an item shows details on the right. At the bottom you are informed about current remote activities — e.g. a Push operation shows a progress indicator and status.

**Adopt in Gitronimo:**

- sidebar stays the primary navigation (current design already does this);
- add a **remote-activity footer** in the sidebar (or activity bar) showing in-flight fetch/pull/push progress and last result — wire it to the existing `NetworkOperation` state.

### 1.6 Working Copy — the commit hub

![Tower Working Copy](screens/tower-guides/overview-06-working-copy.png)

**Guide text:** (a) the left side lists currently modified files (with an option to see all of the project's files); selecting a file shows its diff on the right. (b) The top-left commit area wraps up changes into a new commit.

**Adopt in Gitronimo:**

- move the commit composer to sit directly above the file list in the Working Copy view (it is currently reachable via the composer action);
- add a **"Modified only / All files" list-mode toggle**;
- add a compact per-file **stage checkbox** (see 2.3).

### 1.7 Pull Requests — collaboration surface

![Tower Pull Requests](screens/tower-guides/overview-07-pull-requests.png)

**Guide text:** open pull requests are listed on the left; choosing one inspects the proposed change or lets you comment and manage it. A button starts a new pull request.

**Adopt in Gitronimo:** the Phase 9 PR view:

- left: list of open PRs (title, number, author, updated);
- right: detail with description, changed files, comments;
- actions: comment, merge with explicit method, create PR, checkout branch.

### 1.8 Commit History — changeset and tree modes

![Tower History](screens/tower-guides/overview-08-history.png)

**Guide text:** the History view shows the commit log; selecting a commit shows details on the right in two modes: (a) *Changeset* — meta data (author, date, message) plus detailed changes; (b) *Tree* — the project's complete file structure at that moment. Double-click opens the Commit Detail view.

**Adopt in Gitronimo:**

- Gitronimo's History already shows commit graph + per-commit diff; add a **Changeset / Tree mode toggle** on the detail pane;
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

| # | Improvement | View | Phase |
|---|-------------|------|-------|
| 1 | Commit composer visible above Working Copy file list | Working Copy | now |
| 2 | Modified-only / All-files list-mode toggle | Working Copy | now |
| 3 | Per-file stage checkbox + staged/unstaged badge | Working Copy | now |
| 4 | Visible Back/Forward toolbar buttons | Toolbar | now |
| 5 | Reveal new commit in History after committing | History | now |
| 6 | Changeset / Tree mode toggle on commit detail | History | now |
| 7 | Commit Detail destination (double-click) | History | now |
| 8 | Remote-activity progress in sidebar footer | Sidebar | now |
| 9 | Stashes and Remotes sidebar destinations | Sidebar | now |
| 10 | Repositories view: folders, create-new, grouping | Welcome | Phase 9 |
| 11 | Services view (accounts + clone-from-service) | Services | Phase 9 |
| 12 | Pull Requests view (list, detail, comment, merge, create) | PRs | Phase 9 |

### Implementation status

- [x] Working Copy composer, list mode, per-file staging, navigation, commit reveal, and remote activity.
- [x] Commit Detail Changeset/Tree modes and double-click navigation.
- [x] Stashes and Remotes destinations.
- [x] Git LFS status destination.
- [x] Repositories view with grouped/flat recents, Add existing, Create new, and local/service clone entry points.
- [x] Services view with GitHub token validation, Keychain storage, repository listing, rate-limit state, sign-out, and clone handoff.
- [x] Pull Requests list, detail, comment, merge, create, and checkout workflow.

### Delivery notes

- Services currently targets GitHub Cloud with a personal access token entered through an obscured macOS dialog and stored only in Keychain.
- Hosted repository cloning uses the provider's clone URL and the user's configured Git credential helper or SSH setup.
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
