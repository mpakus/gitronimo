# Keyboard shortcuts

Press **Command-/** in the app to toggle the in-app shortcut reference overlay.

## Global (repository window)

| Shortcut | Action |
|----------|--------|
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
2. **Click any row again** — select all visible files again.

### Stage checkboxes

- **Single file selected** — checkbox toggles stage/unstage for that file.
- **Multiple files selected** (via Command-A, Shift range, or Command-click) — checkbox on any selected row stages or unstages **all selected files** based on that row's current staged state. File list selection is kept after the operation.

Partial line/hunk staging remains in the diff viewer (Stage Chunk / Discard Chunk).

## Welcome window

Welcome search and repository actions use the command palette and toolbar; see in-app menus for repository-specific shortcuts.
