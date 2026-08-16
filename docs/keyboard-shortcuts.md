# Keyboard shortcuts

Press **Command-/** in the app to toggle the in-app shortcut reference overlay. The overlay lists the most common shortcuts; this document is the complete reference.

## Global (repository window)

| Shortcut | Action |
|----------|--------|
| Command-Q | Quit GitRonimo |
| Command-H | Hide GitRonimo |
| Command-F | Focus toolbar search |
| Command-O | Open repository |
| Command-R | Refresh working copy |
| Command-Shift-C | Focus commit subject |
| Command-Shift-S | Save stash (message + include-untracked dialog) |
| Command-Shift-P | Command palette (Fetch/Pull/Push/Sync, staging, views, …) |
| Command-/ | Show or hide shortcut reference |
| Command-[ | Navigate back |
| Command-] | Navigate forward |
| Up / Down | Move selection in loaded History |
| Command-Shift-L | Toggle light/dark appearance |
| Command-Option-Left | Widen sidebar |
| Command-Option-Right | Widen inspector |

## Activity bar

| Control | Behavior |
|---------|----------|
| Status line | Latest activity message (success / error / progress / confirm coloring) |
| History button (left of status) | Opens **Message history** — scrollable log of recent statuses, errors, and confirmations (refresh chatter coalesced) |
| Network progress | Appears while fetch/pull/push/sync runs; **Cancel** aborts the in-flight Git child when possible |

Palette command **Message history** toggles the same popup. See [desktop-shell.md](desktop-shell.md).

## Command palette

`Command-Shift-P` opens a filterable, **scrollable** command list. Besides secondary views (Reflog, Blame, Compare, Workflow, …), it includes toolbar actions: Open repository, Fetch, Pull…, Push…, Sync, Stage/Unstage all, stash Save/Apply (dialogs), Save stash snapshot…, Apply selected stash files, Apply/Branch/Pop/Drop selected stash, Create branch…, Show settings, Show workflow, Quick open file, Message history, Toggle appearance, Navigate back/forward, **About GitRonimo**.

## Working Copy

| Shortcut | Action |
|----------|--------|
| Command-A | Select all files in the visible list (respects Modified/All Files tab and file search) |

When the commit subject or description field is focused, **Command-A** selects text inside that field instead.

### File list selection

- **Single click** — select one file and load its diff.
- **Command-click** — add or remove a file from the selection.
- **Shift-click** — range-select between the last clicked file and the current row.
- **Command-A** — select all visible changed files.
- **Drag a row** — drop the file (or the current multi-selection) on another macOS app to open those paths. Missing or deleted files are skipped.

When every visible file is already selected:

1. **Click any selected row** — clear the selection.
2. **Click that same row again** — select all visible files again.
3. **Click a different row** — select just that file, as usual.

The all/none toggle is therefore limited to repeated clicks on one row; single selection always stays reachable.

### Stage checkboxes

- **Single file selected** — checkbox toggles stage/unstage for that file.
- **Multiple files selected** (via Command-A, Shift range, or Command-click) — a checkbox click on any selected row applies to **all selected files**: the first click checks every box (stages the selection), and clicking again clears them (unstages). A selection that is only partly staged always stages first, whichever row you click. File list selection is kept after the operation.

Partial line/hunk staging remains in the diff viewer (Stage Chunk / Discard Chunk).

## Toolbar network actions

| Control | Behavior |
|---------|----------|
| **Pull** | Opens the Pull dialog (Remote Branch dropdown + Use Rebase Instead of Merge) |
| **Push** | Opens the Push HEAD dialog (Destination dropdown + Options) |
| **Sync** | Fetch + pull + push using the configured upstream (no dialog) |
| Sidebar **Pull…** | Same Pull dialog, prefilled when possible |
| Sidebar **Push…** | Same Push dialog, destination prefilled from the branch |

### Push dialog options

| Option | Git flag |
|--------|----------|
| Push All Tags | `--tags` |
| Force Push | `--force-with-lease` (never a bare `--force`) |
| Recurse Submodules | `--recurse-submodules=check` or `=on-demand` |
| Skip Hooks | `--no-verify` |

The destination is pushed as `<remote> HEAD:<branch>`; picking a remote branch that does not exist yet adds `--set-upstream`.

## Branches (sidebar)

| Interaction | Behavior |
|-------------|----------|
| Left-click local/remote/tag | Open History scoped to that ref |
| Double-click local/remote branch | Check out (`git switch` / `git switch --track`) |
| Right-click | Branch context menu (pin, pull/push, archive, rename, delete, …) |
| Pin | Branch stays flat at the top of BRANCHES across relaunch (persisted per repo) |
| Archive | Branch moves under **ARCHIVED** |
| Delete… | Cancel / Delete (`git branch -d`); if unmerged → **Could Not Delete Branch** Cancel / Delete force (`-D`) |

## History

| Interaction | Behavior |
|-------------|----------|
| Left-click commit | Select row and load changeset detail |
| Double-click commit | Open Commit Detail |
| Right-click commit | Commit context menu (copy, checkout, reset/revert/rebase, amend, branch/tag, patch, export, compare) |)

## Welcome window

Welcome search and repository actions use the command palette and toolbar; see in-app menus for repository-specific shortcuts.
