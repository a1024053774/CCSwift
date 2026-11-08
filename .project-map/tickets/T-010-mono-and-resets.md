---
id: T-010
title: Monochrome look, reset times, own app identity
type: build
status: open
blocked_by: [T-009]
claimed_by:
supersedes:
---

## What to build

User feedback, 2026-11-08:

1. Colors: plain monochrome. Light = white background, black text; dark = black background, white text. Grays only for borders and muted text; no warm tint, no orange accent, no background wash/gradient. Accent (active dot, bars, focus ring) uses the foreground color. Edit the theme file and remove the wash, not add a theme switcher.
2. Usage rows: each account shows both 5h and 7d bars with percent, and for each window **when it resets** (e.g. "18:00 重置", "11月13日 重置"), plus when the reading was taken ("更新于 3 分钟前"). The CLI OAuth usage response carries `resets_at` per window: keep it. Prefer the CLI reading when the account has one (it is fresh and has reset times); otherwise Desktop's reading, whose reset time is unknown, shown as "—". Keep the row compact; drop the "CLI refreshes at most once a minute" footer.
3. Own identity so the release does not pose as upstream: app name "CC Same Plus", bundle id `io.github.a1024053774.cc-same-plus`, DMG/archive names to match in `scripts/bundle-macos.sh`. Keep the cc-same state directory (T-002). README top section: one paragraph saying what this fork adds and crediting songkeys/cc-same and realiti4/claude-swap; keep the license.
4. Bump version to 0.2.0.

## Acceptance

- [ ] Light and dark screenshots of Desktop, CLI and Usage pages (computer use, real accounts): pure white / pure black backgrounds, no orange
- [ ] Usage screenshot shows 5h + 7d, reset times and reading age for each account; CLI reset times match `cswap list` output
- [ ] `scripts/bundle-macos.sh` produces "CC Same Plus.app" and its DMG/ZIP; the app launches from dist/ and its CLI switch still works once (2 -> start slot), checked with `cswap status`
- [ ] cargo test + clippy pass for both workspaces

## Resolution

