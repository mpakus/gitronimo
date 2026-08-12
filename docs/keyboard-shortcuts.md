# Keyboard shortcuts

Press **Command-/** in the app to toggle the in-app shortcut reference overlay. The overlay lists the most common shortcuts; this document is the complete reference.

## Global (repository window)

| Shortcut | Action |
|----------|--------|
| Command-Q | Quit Gitronimo |
| Command-O | Open repository |
| Command-R | Refresh working copy |
| Command-Shift-C | Focus commit subject |
| Command-Shift-P | Command palette |
| Command-/ | Show or hide shortcut reference |
| Command-[ | Navigate back |
| Command-] | Navigate forward |
| Up / Down | Move selection in loaded History |
| Command-Shift-L | Toggle light/dark appearance |
| Command-Option-Left | Widen sidebar |
| Command-Option-Right | Widen inspector |

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

## Welcome window

Welcome search and repository actions use the command palette and toolbar; see in-app menus for repository-specific shortcuts.
