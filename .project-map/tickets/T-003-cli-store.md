---
id: T-003
title: CLI account store
type: decide
status: closed
blocked_by: []
claimed_by:
supersedes:
---

## Question

Where do CLI accounts and their credentials come from?

## Resolution

Read and write claude-swap's store directly: `~/.claude-swap-backup/sequence.json` (accounts, sequence, `activeAccountNumber`) and its backup Keychain items (service `claude-swap`), and the active `Claude Code-credentials` item plus `oauthAccount` in `~/.claude.json`, exactly as `cswap switch` does. No second store; `cswap list` must agree with the app after a GUI switch. Byte formats follow claude-swap's source (`credentials.py`, `switcher.py`, `macos_keychain.py`). Proposed by Claude under delegation.
