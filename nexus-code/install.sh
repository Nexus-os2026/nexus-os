#!/usr/bin/env bash
# WITHDRAWN during Phase Zero (P0-FINAL-GATE-CLOSURE, dossier item J5).
#
# This script downloaded an `nx` release archive (nx-<platform>-<arch>.tar.gz)
# from github.com/nexaiceo/nexus-os and installed it to ~/.local/bin/nx, with
# NX_VERSION and NX_INSTALL_DIR overrides. The standalone `nx` terminal is
# withdrawn, and a previously published archive would install a build from
# before the withdrawal.
#
# Running this file fails immediately, on purpose. It reads no environment
# variable or argument, downloads nothing and installs nothing. It does not
# remove an `nx` installed by an earlier version of this script.
echo "nexus-code/install.sh: withdrawn during Phase Zero; nothing is downloaded or installed" >&2
exit 1
