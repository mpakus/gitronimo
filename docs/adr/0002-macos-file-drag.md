# ADR 0002: AppKit file-URL drag-out from Working Copy

- Status: Accepted
- Date: 2026-08-14

## Context

Working Copy rows need to drag onto other macOS apps (editors, Finder) as real files. GPUI 0.2.2 `on_drag` is an in-window payload and never writes `NSFilenamesPboardType` / `public.file-url`. AppKit’s dragging session API is the supported way to export file URLs. The workspace lint `unsafe_code = deny` exists because Gitronimo otherwise has no `unsafe`. objc2 `define_class!` and `NSDraggingItem::setDraggingFrame_contents` require `unsafe`.

## Decision

Confine macOS drag-out to `platform_macos`:

- A small objc2 `NSDraggingSource` class that always advertises **copy** (never move/delete).
- `NSDraggingItem` writers that are `NSURL` file URLs for paths the desktop layer already validated.
- `unsafe_code` allowed only in that crate, documented here.

The desktop crate resolves Git paths to existing worktree files and must not call AppKit from `Render`.

## Alternatives

- GPUI `on_drag` / `ExternalPaths`: rejected; those types receive drops into GitRonimo, they do not start an OS drag.
- AppleScript / `NSWorkspace.open`: rejected; that opens a file in place, it is not a drag to an arbitrary drop target.
- Patching vendored GPUI: rejected; drag-out is product behavior, not a framework fork.

## Consequences

Dragging a status row past a short movement threshold starts a nested AppKit drag. Click-to-select is unchanged. Destinations that accept file URLs (Zed, RubyMine, Finder, Mail) can open or attach the files. Missing or escaped paths never reach AppKit.

## Rollback path

Remove the Working Copy mouse-move hook and delete `platform_macos` file-drag code plus this ADR. Restore `unsafe_code = deny` as the only crate lint for that package.
