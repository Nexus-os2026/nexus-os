# NEXUS Social Poster Agent

`social-poster` is the first end-to-end runnable NEXUS agent. It researches a topic, drafts social posts, runs compliance checks, and publishes to X.

## Pipeline

1. Research: web search for current topic updates.
2. Read: extract key points from the top articles.
3. Generate: create platform-ready post copy with the LLM gateway.
4. Review: enforce ToS/rate compliance checks.
5. Publish: send approved post to X.
6. Log: write a full audit trail for every step.

## Run Guide

1. Configure keys:
   - `nexus setup`
2. Create the agent:
   - `nexus agent create agents/social-poster/manifest.toml`
3. Start the agent:
   - `nexus agent start social-poster`
4. View logs:
   - `nexus agent logs social-poster`
5. Verify post on X.

## Demo Mode (No Real Posting)

Use dry-run to execute the complete pipeline without calling real X posting:

- `nexus agent start social-poster --dry-run`

Dry-run still performs research, reading, generation, compliance checks, and audit logging, but only prints generated content.
