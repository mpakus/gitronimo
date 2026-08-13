# Changelog

## Unreleased

### Shell and workflow

- Message history on the activity bar (scrollable; successes, errors, confirmations; working-copy refresh lines coalesced).
- Command palette expanded and scrollable: Fetch/Pull/Push/Sync, staging, stash, create branch, settings, and secondary views.
- Branch delete Cancel/Delete; unmerged tips open **Could Not Delete Branch** for force delete (`git branch -D`).
- Pinned branches persist per repository and sort flat at the top of BRANCHES; preference writes are serialized so geometry saves cannot wipe pins.
- Network progress strip in the bottom activity bar during fetch/pull/push/sync.

## 0.1.0 beta

### Included

- Open local Git repositories and inspect working-copy status and diffs.
- Stage, unstage, discard tracked changes, and create commits (including amend and sign-off).
- Partial line and hunk staging in the diff viewer.
- Working Copy multi-select: Command-A, Shift-click, toggle deselect/reselect; batch stage/unstage via checkboxes on multi-selection.
- Browse bounded history with graph, scope filter, Changeset/Tree detail, and Commit Detail navigation.
- Stashes, remotes, services (GitHub token), pull requests, and settings surfaces (two-pane layouts).
- Welcome/Repositories: bookmarks, grouped recents, add/create/clone, Services rail.
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
