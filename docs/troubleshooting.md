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

Full shortcut and selection rules: [`keyboard-shortcuts.md`](keyboard-shortcuts.md).

GPUI 0.2.2 does not expose macOS accessibility roles or programmatic labels for its custom elements, so VoiceOver parity is a known beta limitation. It will need framework support before Gitronimo can provide a complete assistive-technology experience.

## Git engine (gix vs System Git)

Settings **Git engine** defaults to **gix**. Discovery, status, history, stage/commit, and HTTPS fetch/clone use `gix` unless you choose **System Git** or `gix` fails (then GitRonimo falls back and shows a redacted status line). Checkout, merge, rebase, stash, push, hooks, signing, LFS, and SSH remotes always use the installed Git executable.

## Git LFS fetch or pull failed

The Git LFS view runs the installed `git lfs` client (`fetch` downloads objects; `pull` also checks them out). Install Git LFS if the command is missing. Operations use the first configured remote and can be cancelled from the activity bar. Credentials follow your Git helper, same as remotes.

## Stash files did not apply

**Apply selected files** (or dragging stash files onto Working Copy) restores those paths from the stash without dropping it. Select files in the stash changeset first. Deleted paths and some rename cases may fail because Git `restore` needs a path that exists in the stash tree. Use **Apply…** for the full stash. Parent (`..`) and absolute paths are refused.

## Snapshot vs stash

**Save snapshot…** keeps your working copy and adds a named entry to the stash list (subject starts with `snapshot`). **Save stash** resets the working copy. A clean tree cannot snapshot. Messages starting with `-` are refused.

## Auto-stash left conflicts or a leftover stash

Settings **Auto-stash before switch and pull** is off until you turn it on. Switch stashes including untracked files, then pops; pull passes Git `--autostash`. If reapplying conflicts, resolve them on Working Copy. The stash entry (subject `gitronimo autostash before switch`, or Git's autostash for pull) stays until you drop it.

## Build toolchain

GitRonimo requires **Rust 1.97+** (`edition2024`). If `cargo build` fails with an edition error, install the pinned toolchain:

```bash
rustup toolchain install 1.97.1-aarch64-apple-darwin
rustup target add aarch64-apple-darwin x86_64-apple-darwin
export PATH="$HOME/.rustup/toolchains/1.97.1-aarch64-apple-darwin/bin:$PATH"
```

The debug/release binary is `target/debug/GitRonimo` (crate `gitronimo-desktop`). Packaging both architectures: [packaging.md](packaging.md).

## Unsigned app / Gatekeeper

Local and CI development bundles are unsigned `GitRonimo.app`. macOS Gatekeeper may block first launch. That is expected until a Developer ID signed, notarized tag release. Do not disable Gatekeeper globally; use the signing handoff in packaging.md for distribution.

## In-app update failed or refused

**GitRonimo → Settings…** (Command-comma) **In-app updates** is on by default. **Check now** / About **Check for updates** / **GitRonimo → Check for Updates…** only talk to GitHub when that toggle is on; GitRonimo does not poll on launch. Install requires a real `GitRonimo.app` (not `cargo run`). The zip must match `SHA256SUMS.txt`, and the extracted app must pass Gatekeeper. Unsigned local zips are refused. After a successful install, quit and open `GitRonimo.app` again.

## AI commit suggestion failed or did nothing

Settings **AI commit messages** stays off until you turn it on. **Suggest** (Working Copy composer, or palette **Suggest commit message**) sends only the staged diff, with tokens/URL passwords redacted, to the endpoint you configured. It never commits. Stage at least one path first. HTTPS endpoints (including the empty default `https://api.openai.com/v1`) need an API key in Keychain (**API key…** in Settings); that key is not the GitHub PAT. Loopback HTTP (`127.0.0.1`, `localhost`, `[::1]`) is allowed for a local model without a key. A failed request leaves the composer as it was. Turn the toggle off, or an empty staged list, does not call the network.

## About GitRonimo

**GitRonimo → About GitRonimo** (or palette **About GitRonimo**) shows product version **2.0.2**. After a release, bump `APP_VERSION` in `apps/desktop/src/views/about.rs` and the packager version together. **Check for updates** on About (and **GitRonimo → Check for Updates…**) uses the same path as App Settings. Click outside the overlay to dismiss it.