#!/usr/bin/env bash
# WITHDRAWN during Phase Zero (P0-FINAL-GATE-CLOSURE, dossier item J4).
#
# This script built nexus-cli with cargo and archived it with the Homebrew
# formula and the launchd job as target/package/nexus-os_<version>_macos.tar.gz.
# nexus-cli is withdrawn: it only prints a withdrawal message. The desktop
# app's macOS image is built by the release workflow, not by this script.
#
# Running this file fails immediately, on purpose. It reads no argument,
# builds nothing and packages nothing.
echo "build_macos_release.sh: withdrawn during Phase Zero; nothing is built or packaged" >&2
exit 1
