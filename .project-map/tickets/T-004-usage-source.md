---
id: T-004
title: Usage source
type: decide
status: closed
blocked_by: []
claimed_by:
supersedes:
---

## Question

Where does each account's usage come from for the unified view?

## Resolution

Desktop accounts: cc-same's existing reading of Desktop's `plan-usage-history.json` (no network). CLI accounts: `GET https://api.anthropic.com/api/oauth/usage` with that account's stored access token, as claude-swap does, when the usage view opens and on manual refresh, at most once a minute per account; no token refresh, no background polling. An expired token shows "needs `cswap` refresh", not an error dialog. Accounts with the same email are one row with both sources. Proposed by Claude under delegation.
