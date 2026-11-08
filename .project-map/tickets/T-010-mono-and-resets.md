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

- [x] Light and dark screenshots of Desktop, CLI and Usage pages (computer use, real accounts): pure white / pure black backgrounds, no orange
- [ ] Usage screenshot shows 5h + 7d, reset times and reading age for each account; CLI reset times match `cswap list` output
- [x] `scripts/bundle-macos.sh` produces "CC Same Plus.app" and its DMG/ZIP; the app launches from dist/ and its CLI switch still works once (2 -> start slot), checked with `cswap status`
- [x] cargo test + clippy pass for both workspaces

## Resolution

INCOMPLETE — implemented on 2026-11-08; the usage comparison remains unchecked because the real OAuth endpoint rate-limited slot 3. Commit: the commit containing this resolution (`T-010: add monochrome UI, reset times, and CC Same Plus identity`). T-009's unrelated tray acceptance remains unchanged.

### Changes

- Pure white/black theme backgrounds and foreground accents; removed the warm wash. Replaced GPUI's independently colored initials with monochrome initials and kept the selected tab visible with an underline. No new theme setting.
- Retain each CLI window's raw `resets_at`, render local reset times (unknown/null/malformed = `—`), reading age and source; prefer successful CLI readings over Desktop. Removed the refresh footer. Scoped the active Keychain lookup to the OS user after reproducing the ambiguous service-only lookup. No new dependencies, token refresh, polling or automatic retry.
- Version 0.2.0 in both workspaces; runtime and macOS bundle name `CC Same Plus`, bundle ID `io.github.a1024053774.cc-same-plus`; matching DMG/ZIP/CLI archive names. State paths and switch implementation unchanged. README credits both upstream projects; license unchanged.

### Evidence and commands

All artifacts are local under `work/T-010/` (ignored by the existing user `.gitignore` edit, which is excluded from this commit).

- Baseline computer-use capture: `before-usage.png` shows warm/orange styling, no resets and the footer. `python3 work/T-009/reproduce-active-keychain.py > work/T-010/keychain-before.json` reproduces the missing OAuth object for the service-only query; current-user query finds it. Outputs contain presence flags only.
- Focused parser contract: `python3 work/T-010/check-reset-mutant.py`. `reset-mutant.log` fails when reset retention is deliberately replaced with `[None, None]`; `reset-green.log` passes on the restored implementation. Fixture independently specifies both timestamps and null/malformed reset behavior.
- `cargo test --workspace --all-features`: PASS, 55 core and 26 integration tests; one existing native Keychain test ignored (`cargo-test.log`). `cargo clippy --workspace --all-targets --all-features`: PASS (`cargo-clippy.log`).
- `cargo test --manifest-path crates/app/Cargo.toml`: PASS, 17 tests (`app-test-final-candidate.log`). `cargo clippy --manifest-path crates/app/Cargo.toml --all-targets`: PASS (`app-clippy-final-candidate.log`). Existing dependency `block 0.1.6` emits its future-incompatibility notice.
- `scripts/bundle-macos.sh`: PASS (`bundle-final-candidate.log`), including ad-hoc signature verification. Produces `dist/CC Same Plus.app`, `dist/CC-Same-Plus-0.2.0-macos-arm64.dmg`, `dist/CC-Same-Plus-0.2.0-macos-arm64.zip`, and `dist/cc-same-plus-cli-0.2.0-macos-arm64.tar.gz`. `bundle-identity.json` records the resulting plist values. Launched that app from dist through computer use.
- Six final computer-use screenshots: `desktop-{light,dark}.png`, `cli-{light,dark}.png`, `usage-{light,dark}.png`. Content backgrounds are white/black and accents monochrome; native macOS traffic-light controls retain their system colors. Preliminary visual failures remain in `desktop-dark-colored-avatars.png` and `desktop-dark-no-selection.png`; both were corrected before the final captures.
- Real CLI UI round trip: starting slot 1 -> 2 -> 3 -> 1. After each click, `cswap status` confirmed the selected account: `switch-slot2.txt`, `switch-slot3.txt`, `switch-restored.txt`, with corresponding PNG captures. This exercises slots 2/3 and returns to the starting slot; no Desktop switch was clicked. `start-cswap.txt` and `final-cswap.txt` both show Account-1.

### Remaining usage comparison

- `cswap list --json` and `cswap list` outputs are saved as `usage-reference.{json,txt}` and `usage-final-reference.{json,txt}`. All accounts have 0% five-hour usage and no five-hour reset timestamp, so `—` is correct for those windows.
- The final UI shows CLI 19% weekly for slot 2 and CLI 73% for slot-1 account, with reset times and reading ages. Slot 3 still shows Desktop 65% with unknown reset, while cswap has cached CLI 73%. `usage-manual-refresh.png` preserves a manual refresh after the throttle period; it did not resolve the missing reading.
- `python3 work/T-010/check-usage-response.py > work/T-010/slot3-response.json`: the exact slot-3 Keychain item exists and contains an unexpired OAuth credential, but its read-only usage request returns HTTP 429. The diagnostic saves no tokens or full response bodies. The duplicate-slot warning remains untouched. Rate limiting is confirmed; whether the shared credential causes that limit is not proven.
- Reset clocks from successful fresh responses also differ by up to one displayed minute from cached cswap clocks (17:59 vs 18:00, 23:00 vs 22:59). The parser retains the API timestamps rather than adjusting them to match a cached answer. Exact real-account clock agreement is therefore not claimed.
- Next step: repeat the Usage capture and cswap comparison after the endpoint's rate limit clears. If the duplicate credential repeatedly causes the limit, decide a separately scoped request-deduplication change; do not repair slots or add speculative retries as part of this ticket.

### Restoration and checks

- Restored the app's existing appearance override to System (`appearance-restored.txt`); system appearance itself was never changed. Quit the fork and ran `open -a "CC Same"`; `original-restored.png` and `original-process.txt` verify the installed original running again. Claude Desktop was never quit/restarted and no Desktop account switch was clicked.
- `desktop-logins-restored.json` records unchanged paths, sizes and modification times for the saved-login files compared with the pre-switch snapshot; this is a metadata check, not a byte comparison. Account credentials/stores were changed only through the authorized switch path. No duplicate-slot repair was attempted.
- `git diff --check`: PASS (`diff-check.txt`). `python3 /Users/luckye/.claude/skills/project-map/scripts/project_map.py status --root .`: result in `project-map-status.txt`.

Works: monochrome pages, reset retention and rendering, renamed release artifacts, and the real CLI round trip have direct evidence. No regression: both workspace tests and clippy pass; Desktop saved-login metadata is unchanged. Risk / undo: complete live usage agreement is unverified due to HTTP 429 and differing cached reset clocks; the ticket stays open. Revert this commit to undo product changes. The original installed app remains unchanged, and the starting CLI slot and appearance preference are restored.
