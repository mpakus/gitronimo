# Screenshot reference

## Gitronimo product captures (committed)

These files are safe to embed in README and public docs. They show Gitronimo's own UI.

| File | View | Notes |
|------|------|-------|
| [gitronimo-welcome.png](gitronimo-welcome.png) | Welcome / empty state | Choose repository, quick start, recents |
| [gitronimo-repositories.png](gitronimo-repositories.png) | Welcome / Repositories | Bookmark folders, drop zone, toolbar search |
| [gitronimo-working-copy.png](gitronimo-working-copy.png) | Working Copy | Composer, file list, staged diff, hunk actions |
| [gitronimo-history.png](gitronimo-history.png) | History | Graph, month groups, Changeset detail |
| [gitronimo-branch-menu.png](gitronimo-branch-menu.png) | Sidebar context menu | Right-click branch actions over History |

Retake captures after major UI changes. Prefer clean shots without debug annotations (red boxes or arrows).

## Tower reference captures (local study only)

The following paths are **gitignored** and kept for internal UX comparison only, per [AGENTS.md](../../AGENTS.md):

- `tower-guides/` — screenshots from [Tower help guides](https://www.git-tower.com/help/guides/first-steps/tower-overview/mac) (© fournova / git-tower.com)
- `tower-*.png`, `t-vs-g.png` — comparison captures
- `00*.png`, `0*.png` — dated Gitronimo dev captures (may duplicate committed shots)

Never ship Tower screenshots, icons, copy, or branding in the application bundle. Gitronimo uses them only to study information architecture and density.

## Adding a new committed screenshot

1. Capture at native resolution on macOS (dark mode is the default product look).
2. Save as `docs/screens/gitronimo-<view>.png`.
3. Update [README.md](../../README.md) and this table.
4. Confirm `.gitignore` still whitelists `gitronimo-*.png`.
