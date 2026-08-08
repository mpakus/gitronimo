# Troubleshooting

## Gitronimo cannot open a repository

Choose the working-tree folder, not a bare repository. Confirm that Git is installed and that the folder still exists. Gitronimo reports repository disappearance without deleting or repairing files automatically.

## Git reports `index.lock`

Check that no Git process is still running. If none is running, inspect `.git/index.lock` and remove it manually only after confirming it is stale. Gitronimo never deletes index locks automatically.

## Authentication or push rejection

Gitronimo delegates credentials to your installed Git credential helper or SSH configuration. For a non-fast-forward rejection, fetch or pull the newer remote commits, resolve any changes, then push again. Force-with-lease is available only through an explicit advanced confirmation.

## Crash report

On a panic, Gitronimo writes a local report under `~/Library/Application Support/Gitronimo/crash-reports/`. Reports are never uploaded automatically and contain only timestamp and source-location metadata.

## Keyboard and assistive technology

All Gitronimo actions use visible text labels, and shared action controls repeat those labels in hover tooltips. Use the native menus or `Command-Shift-P` command palette; `Command-/` opens the complete shortcut reference.

GPUI 0.2.2 does not expose macOS accessibility roles or programmatic labels for its custom elements, so VoiceOver parity is a known beta limitation. It will need framework support before Gitronimo can provide a complete assistive-technology experience.
