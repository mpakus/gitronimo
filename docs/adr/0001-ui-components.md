# ADR 0001: Use project-owned GPUI primitives instead of gpui-component

- Status: Accepted
- Date: 2026-08-06

## Context

Phase 0 evaluated `gpui-component 0.5.1`, which is compatible with the pinned `gpui 0.2.2`. Its required `init` function installs a library-owned theme and initializes broad global state for dock, input, list, dialog, menu, and related controls. Gitronimo needs an original, project-owned visual system and must keep framework coupling narrow.

## Decision

Remove `gpui-component` after the compatibility evaluation. Build the small set of controls required by active checklist items with GPUI primitives in `ui_kit`.

## Alternatives

- Adopt all of `gpui-component`: rejected because its global initialization and visual system would leak beyond the intended wrapper boundary.
- Partially adopt it: rejected because the initializer still installs the same global state and theme.
- Use GPUI primitives: selected because they preserve original design tokens and minimize framework-specific coupling.

## Consequences

The project owns buttons, inputs, lists, dialogs, and layout primitives as they become necessary. This costs targeted implementation work but avoids a second design system and makes GPUI upgrades easier to contain. Future work may re-evaluate a pinned component release only through a new ADR and dedicated spike.

## Rollback path

Restore the exact dependency in `Cargo.toml`, implement a `ui_kit` adapter for a specific needed component, prove it respects project tokens and focus behavior, and replace this ADR with a new decision.

