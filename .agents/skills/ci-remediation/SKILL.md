---
name: ci-remediation
description: Inspect failing GitHub Actions PR checks via gh CLI, diagnose root causes, and apply bounded local fixes until CI passes.
---

# CI Self-Remediation Workflow

1. Inspect PR check status via `gh pr checks <pr>`.
2. Fetch failing job logs via `gh run view <run-id> --log-failed`.
3. Reproduce and fix the root cause locally; re-run local verification commands before pushing.
4. Limit autonomous remediation to at most 3 bounded iterations; escalate clearly if blocked by external permissions or secrets.
