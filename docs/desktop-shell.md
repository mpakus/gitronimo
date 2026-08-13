# Desktop shell chrome

Functional reference for the repository window chrome that sits outside individual views (toolbar, sidebar BRANCHES, activity bar, overlays). Updated for the 2026-08-12 shell pass.

## Layout

```
┌─ Toolbar (Fetch / Pull / Push / Sync · stash · Refresh · search · palette) ─┐
│ Sidebar │ Content (+ optional list/detail panes)                            │
│ BRANCHES│                                                                   │
│ (pins…) │                                                                   │
├─ Activity bar: [history] [network progress…] status line ───────────────────┤
└─ Overlays: command palette, text/choice prompts, Pull/Push, confirms ───────┘
```

Preferences live at `~/Library/Application Support/Gitronimo/recent-repositories.json` (schema v1).

## Activity bar and message history

| Piece | Behavior |
|-------|----------|
| Status line | Current `activity` string; colored via `ActivityKind` (success / error / progress / confirm / info) |
| Clock / history button | Toggles **Message history** popup (bottom-left) |
| Popup | Newest-first, up to 100 entries; scrollable; colored dots + relative ages |
| Network strip | Indeterminate progress + Cancel while a `NetworkOperation` runs |

All user-visible status text should go through `GitronimoApp::set_activity` so the history log stays complete. Consecutive identical lines are skipped. Consecutive working-copy refresh chatter (`Refreshing working copy…` / `Working copy refreshed…`) is **coalesced** so push completions, errors, and confirmations are not drowned out.

Palette entry: **Message history**.

## Confirm dialogs

Destructive or blocked Git outcomes use modal overlays (not only the activity flash):

| Flow | First step | Second step (if needed) |
|------|------------|-------------------------|
| Delete local branch | **Delete Branch** — Cancel / Delete (`git branch -d`) | If not fully merged → **Could Not Delete Branch** — Cancel / Delete (`git branch -D`) via `AppConfirmDialog::ForceDeleteBranch` |
| Force push | Existing force-with-lease confirmation | — |

`AppConfirmDialog` is the shared enum for blocked-action confirmations; render through `workspace` modal helpers. Domain logic stays in `main.rs`, not in `Render`.

## Pinned and archived branches

- Pin / Unpin and Archive / Unarchive live on the local-branch context menu.
- State is per repository under `branch_organization` in preferences (`BranchOrganization { pinned, archived }`).
- **Pinned** branches render **flat at the top of BRANCHES** (bookmark icon, pin order). No separate “PINNED” section label.
- **Archived** branches move under an **ARCHIVED** section.
- Preference load-modify-save is serialized with a **path-keyed lock** so window-geometry / width writers cannot wipe pins (multiple `RecentRepositoryStore::new` handles share one lock per prefs path).

## Command palette (`Command-Shift-P`)

Searchable list (`PALETTE_COMMANDS` in `app_state.rs`); list viewport scrolls. Includes:

- Open repository, Fetch / Pull… / Push… / Sync, Refresh
- Stage all / Unstage all, Focus commit composer, Save stash / Apply latest stash, Create branch…
- Show working copy / history / stashes / remotes / settings / reflog / LFS / services / worktrees / submodules / rebase / conflicts…
- History tools (file history, blame, compare, browse tree, squash/fixup/drop/reword, merge tool, signature check)
- Quick open file, Message history, Toggle appearance, Keyboard shortcuts, Navigate back/forward

Dispatch is `run_palette_command` in `main.rs`. Adding a user-facing action that already has a handler should usually add a `PaletteCommand` variant + label + match arm.

## Related docs

- Shortcuts: [keyboard-shortcuts.md](keyboard-shortcuts.md)
- UI patterns: [UI-IMPROVE.md](UI-IMPROVE.md)
- Architecture / prefs: [architecture.md](architecture.md)
- Recovery: [troubleshooting.md](troubleshooting.md)
