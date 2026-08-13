# Troubleshooting

## Gitronimo cannot open a repository

Choose the working-tree folder, not a bare repository. Confirm that Git is installed and that the folder still exists. Gitronimo reports repository disappearance without deleting or repairing files automatically.

## Git reports `index.lock`

Check that no Git process is still running. If none is running, inspect `.git/index.lock` and remove it manually only after confirming it is stale. Gitronimo never deletes index locks automatically.

## Authentication or push rejection

Gitronimo delegates credentials to your installed Git credential helper or SSH configuration. For a non-fast-forward rejection, fetch or pull the newer remote commits, resolve any changes, then push again. Force-with-lease is available only through an explicit advanced confirmation.

## Branch delete refused (not fully merged)

Safe delete (`git branch -d`) opens **Could Not Delete Branch** when the tip is not fully merged. Choose **Delete** only if you intend to discard those commits from that branch tip (`git branch -D`). Cancel leaves the branch intact. The refusal is also recorded in Message history (activity bar clock button).

## Pinned branches missing after relaunch

Pins are stored in `~/Library/Application Support/Gitronimo/recent-repositories.json` under `branch_organization`. They appear flat at the top of BRANCHES (no separate section title). If pins vanish after a crash mid-write, check for a `.corrupt` backup beside that file; Gitronimo recovers malformed preferences without overwriting a newer schema.

## Message history looks empty of important events

Working-copy refresh lines are coalesced so they do not fill the log. Successes (e.g. push complete), errors, and confirmations remain. Open the clock button on the activity bar, or run **Message history** from the command palette, and scroll.

## Crash report

On a panic, Gitronimo writes a local report under `~/Library/Application Support/Gitronimo/crash-reports/`. Reports are never uploaded automatically and contain only timestamp and source-location metadata. macOS may also keep a matching `.ips` under `~/Library/Logs/DiagnosticReports/`.

## Keyboard and assistive technology

All Gitronimo actions use visible text labels, and shared action controls repeat those labels in hover tooltips. Use the native menus or `Command-Shift-P` command palette (scrollable; includes Fetch/Pull/Push/Sync and staging); `Command-/` opens the complete shortcut reference. Shell chrome details: [`desktop-shell.md`](desktop-shell.md).

Working Copy file selection:

- `Command-A` selects all files in the visible list (Modified/All Files tab and search filter apply).
- When all visible files are selected, click a row to clear selection; click again to select all.
- With multiple files selected, clicking a row's stage checkbox stages or unstages all selected files.

Full shortcut and selection rules: [`keyboard-shortcuts.md`](keyboard-shortcuts.md). Screenshot inventory: [`screens/README.md`](screens/README.md).

GPUI 0.2.2 does not expose macOS accessibility roles or programmatic labels for its custom elements, so VoiceOver parity is a known beta limitation. It will need framework support before Gitronimo can provide a complete assistive-technology experience.

## Build toolchain

Gitronimo requires **Rust 1.97+** (`edition2024`). If `cargo build` fails with an edition error, install the pinned toolchain:

```bash
rustup toolchain install 1.97.1-aarch64-apple-darwin
export PATH="$HOME/.rustup/toolchains/1.97.1-aarch64-apple-darwin/bin:$PATH"
```
