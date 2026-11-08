---
id: T-002
title: Fork and state
type: decide
status: closed
blocked_by: []
claimed_by:
supersedes:
---

## Question

Where does the fork live, and how does it get the user's existing Desktop logins without re-login?

## Resolution

Fork at `~/Documents/Code/cc-same`, branch `switch-cli`. Keep cc-same's state dir (`~/Library/Application Support/cc-same`) so saved Desktop logins are reused as they are; only one of the original app and the fork runs at a time. The in-app updater must not pull upstream releases over the fork (turn it off, do not redesign it). Proposed by Claude under the user's delegation ("不用问我"); the user can override.
