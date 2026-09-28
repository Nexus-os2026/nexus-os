#!/usr/bin/env bash
# WITHDRAWN during Phase Zero (P0-FINAL-GATE-CLOSURE, dossier item J3).
#
# This script downloaded a `nexus-os` release asset (a binary, a tarball, a
# .deb or a .dmg) through the GitLab releases API and installed an executable
# from it to /usr/local/bin/nexus-os, using sudo when needed. The protocols
# `nexus-os` server it installed is withdrawn, and a previously published
# asset would install a build from before the withdrawal.
#
# Running this file fails immediately, on purpose. It reads no environment
# variable or argument, downloads nothing, installs nothing and uses no sudo.
# It does not remove a binary installed by an earlier version of this script.
# The Nexus OS desktop app is installed with its release installers.
echo "install.sh: withdrawn during Phase Zero; nothing is downloaded or installed" >&2
exit 1
