# NEXUS Social Poster Agent

`social-poster` is the first end-to-end runnable NEXUS agent. It researches a topic, drafts social posts, runs compliance checks, and publishes to X.

## Pipeline

1. Research: web search for current topic updates.
2. Read: extract key points from the top articles.
3. Generate: create platform-ready post copy with the LLM gateway.
4. Review: enforce ToS/rate compliance checks.
5. Publish: send approved post to X.
6. Log: write a full audit trail for every step.

## Running it (withdrawn)

The standalone ways to run this agent are withdrawn during Phase Zero: the
`nexus agent` commands of `nexus-cli` and the `social-poster-agent`
executable each print only a fixed withdrawal message and exit with status
69. The repository provides no supported way to run this agent from the
command line at this point, including the dry-run demo mode. The
`social_poster_agent` library is unchanged.
