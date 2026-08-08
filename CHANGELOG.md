# Changelog

## 0.1.0 beta

### Included

- Open local Git repositories and inspect working-copy status and diffs.
- Stage, unstage, discard tracked changes, and create commits.
- Browse bounded history and manage local branches.
- Fetch, pull, publish, and push with cancellation for network work.
- Recover safely from missing repositories, stale index locks, malformed preferences, and application panics.

### Safety and reliability

- Gitronimo uses your installed Git executable, credential helper, SSH configuration, hooks, signing, and filters.
- Force pushes use force-with-lease and require an explicit confirmation.
- Crash reports stay on your Mac and are never uploaded automatically.

### Known limitations

- Partial staging, stash, merge and rebase workflows, and hosting-service integration are not included.
- The public package is not signed or notarized yet.
- VoiceOver parity is limited by the pinned GPUI framework; see the [accessibility note](docs/troubleshooting.md#keyboard-and-assistive-technology).
