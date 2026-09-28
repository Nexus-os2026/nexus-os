#!/usr/bin/env bash
# WITHDRAWN during Phase Zero (P0-FINAL-GATE-CLOSURE, dossier item J4).
#
# This script built nexus-cli with cargo and packaged it, with the
# packaging/linux/nexus-os.service unit, as
# target/package/nexus-os_<version>_<arch>.deb (or a tarball). nexus-cli is
# withdrawn: it only prints a withdrawal message. The desktop app's Linux
# package is built by the release workflow, not by this script.
#
# Running this file fails immediately, on purpose. It reads no argument,
# builds nothing and packages nothing.
echo "build_linux_deb.sh: withdrawn during Phase Zero; nothing is built or packaged" >&2
exit 1
