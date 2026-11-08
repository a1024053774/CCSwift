# Spec: Desktop and CLI switching in one app (2026-11-08)

## Problem

The user switches work/personal Claude accounts with two tools: CC Same for Desktop (with shared conversations) and claude-swap (`cswap`) for the CLI. Two tools, two mental models.

## Solution

Extend this fork of CC Same with a CLI page and a usage view. Desktop switching and session sync stay exactly as upstream.

- Window: a segmented control `Desktop | CLI | Usage` at the top. Desktop = the current cc-same view. CLI = account list with active marker; click to switch. Usage = one list across both.
- Tray: existing Desktop items plus a `CLI` submenu (accounts, click to switch).
- Core: a new module in `crates/core` reads/writes claude-swap's store (T-003) and fetches CLI usage (T-004). The app crate only renders.

## Decisions

[T-001](../tickets/T-001-scope.md), [T-002](../tickets/T-002-fork-and-state.md), [T-003](../tickets/T-003-cli-store.md), [T-004](../tickets/T-004-usage-source.md).

## Test seam

End to end: the built `.app` driven by Codex computer use against the user's real accounts, checked with `cswap status`/`cswap list` and Desktop's signed-in account. Unit tests only for the store read/write against fixtures in claude-swap's format.

## Out of scope

See MAP.md *Out of scope*. No new dependency unless gpui-kit / existing deps cannot do it (Keychain via `/usr/bin/security`, as claude-swap does; HTTP via the existing `reqwest`).
