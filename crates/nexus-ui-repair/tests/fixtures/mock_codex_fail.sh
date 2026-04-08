#!/usr/bin/env bash
# Mock Codex that exits non-zero with a fixed stderr message.
echo "simulated codex failure" >&2
exit 7
