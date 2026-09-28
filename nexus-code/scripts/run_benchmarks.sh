#!/usr/bin/env bash
# WITHDRAWN during Phase Zero (P0-FINAL-GATE-CLOSURE, dossier item J5).
#
# This script ran `nx bench run` and `nx bench paper` with whichever `nx` came
# first on PATH (possibly a build installed before the withdrawal), using the
# Anthropic and OpenAI keys from the environment. The standalone `nx`
# terminal is withdrawn.
#
# Running this file fails immediately, on purpose. It reads no environment
# variable or argument and runs nothing.
echo "run_benchmarks.sh: withdrawn during Phase Zero; nothing is run" >&2
exit 1
