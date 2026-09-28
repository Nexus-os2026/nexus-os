# WITHDRAWN during Phase Zero (P0-FINAL-GATE-CLOSURE, dossier item J4).
#
# This formula built nexus-cli from a source archive with cargo, installed it
# and registered a Homebrew service that kept it running. nexus-cli is
# withdrawn: it only prints a withdrawal message.
#
# Loading this file raises immediately, on purpose: no formula is defined, so
# nothing is downloaded, built, installed or started. It does not uninstall a
# formula or stop a service installed from an earlier version.
raise "nexus-os formula: withdrawn during Phase Zero; nothing is built or installed"
