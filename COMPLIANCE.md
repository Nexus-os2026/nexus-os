# NEXUS OS Platform Compliance

## Scope

This document captures platform Terms of Service (ToS) constraints applied by NEXUS OS Early Access social workflows.

## Posting Limits Enforced

- X (Twitter): approximately 300 posts per 3-hour window.
- Instagram: 25 posts per day.
- Facebook: 50 posts per day.

These are modeled conservatively in `content/compliance.rs` via `check_compliance(platform, recent_posts)`.

## Safety Controls

- Per-platform checks run before each publish attempt.
- Blocked actions return explicit reason strings for audit visibility.
- Workflow continues to other platforms when one platform is blocked.
- Idempotency request IDs prevent accidental duplicate posting during retries.

## Residual Risk

- Vendor quotas and policy interpretations can change over time.
- Production deployments should periodically reconcile with official platform policy docs.
- Human approval gates remain mandatory for sensitive or high-volume automation.
