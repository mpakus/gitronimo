# Changelog

## 0.9 — 2026-08-13

Product version **0.9** (About GitRonimo / `APP_VERSION`). Unsigned Apple Silicon and Intel `.app` bundles; signed notarized distribution still requires the protected tag workflow.

### Shell and workflow

- Message history on the activity bar (scrollable; successes, errors, confirmations; working-copy refresh lines coalesced).
- Command palette expanded and scrollable: Fetch/Pull/Push/Sync, staging, stash, create branch/tag, amend, history selection actions (checkout/reset/revert/patch/export/compare), rebase onto, merge revision, settings, and secondary views.
- Branch delete Cancel/Delete; unmerged tips open **Could Not Delete Branch** for force delete (`git branch -D`).
- Pinned branches persist per repository and sort flat at the top of BRANCHES; preference writes are serialized so geometry saves cannot wipe pins.
- Network progress strip in the bottom activity bar during fetch/pull/push/sync.
- History commit context menu (right-click): copy, detached checkout, reset/revert/rebase with confirms, amend/reword on HEAD, branch/tag/patch/export/compare.
- Stashes Tower-style core: save dialog (message + untracked + path-limited from WC), apply dialog (pop / restore index), stash changeset detail, branch-from-stash; `Command-Shift-S` opens Save stash.
- Current branch sidebar pill shows `HEAD ↑N` / `↓N` (same arrow order as the toolbar tracking line).
- Removed the Services welcome tab, in-repo destination, and palette command. GitHub account connect/sign-out lives in Settings.
- Workflow tab (welcome + in-repo): GitHub Flow / GitLab Flow / git-flow templates, auto-detect from local branches, Start / Finish / Sync topic branches; config persists per repository. Approach-only from Tower Workflows.
- Command-H hides GitRonimo (`Hide GitRonimo` in the application menu), matching Command-Q quit.
- Command-F focuses the toolbar search field (welcome repository filter or in-repo file search).
- macOS application menu is **GitRonimo** (binary / bundle name). **About GitRonimo** opens a black two-column overlay with `assets/gitronimo-icon.png`, product version **0.9** (`APP_VERSION` in `apps/desktop/src/views/about.rs`), “Made in Austin ✩ Texas”, and https://aomega.co.
- Crash-report panics: branch context-menu render no longer re-reads `GitronimoApp` (GPUI double-lease); pane resize / bookmark drag no longer copy a cursor style onto `on_drag` (GPUI `set_window_cursor_style` debug assert).

### Known limitations

- Merge and rebase wizards, OAuth/enterprise GitHub, and notarized distribution are not complete.
- Remote progress is partly indeterminate (no full Git stderr byte/object parsing everywhere).
- VoiceOver parity is limited by the pinned GPUI framework; see the [accessibility note](docs/troubleshooting.md#keyboard-and-assistive-technology).
- Secondary views (Reflog, Blame, Compare, Branches Review, etc.) remain reachable from the command palette rather than the primary sidebar.

## 0.1.0 beta

### Included

- Open local Git repositories and inspect working-copy status and diffs.
- Stage, unstage, discard tracked changes, and create commits (including amend and sign-off).
- Partial line and hunk staging in the diff viewer.
- Working Copy multi-select: Command-A, Shift-click, toggle deselect/reselect; batch stage/unstage via checkboxes on multi-selection.
- Browse bounded history with graph, scope filter, Changeset/Tree detail, and Commit Detail navigation.
- Stashes, remotes, pull requests, and settings surfaces (two-pane layouts).
- Welcome/Repositories: bookmarks, grouped recents, add/create/clone.
- Branch/ref context menu on sidebar right-click; trimmed workspace sidebar (Working Copy, History, Stashes, Settings).
- Fetch, pull, publish, and push with cancellation for network work.
- Pull and Push HEAD dialogs: remote branch/destination pickers, rebase option, push all tags, force-with-lease, recurse submodules, skip hooks.
- Recover safely from missing repositories, stale index locks, malformed preferences, and application panics.

### Safety and reliability

- Gitronimo uses your installed Git executable, credential helper, SSH configuration, hooks, signing, and filters.
- Force pushes use force-with-lease and require an explicit confirmation.
- Crash reports stay on your Mac and are never uploaded automatically.

### Known limitations

- Merge and rebase wizards, OAuth/enterprise GitHub, and notarized distribution are not complete.
- Remote progress is partly indeterminate (no full Git stderr byte/object parsing everywhere).
- VoiceOver parity is limited by the pinned GPUI framework; see the [accessibility note](docs/troubleshooting.md#keyboard-and-assistive-technology).
- Secondary views (Reflog, Blame, Compare, Branches Review, etc.) remain reachable from the command palette rather than the primary sidebar.
