# WITHDRAWN during Phase Zero (P0-FINAL-GATE-CLOSURE, dossier item J5).
#
# This recipe built `nx`, the standalone Nexus Code terminal, into an image
# whose entry point was `nx`. That terminal is withdrawn: its binary only
# prints a withdrawal message.
#
# Building this file fails at its first step, on purpose. It installs no
# packages, copies no source, compiles nothing and produces no image.
#
# It does not stop, remove or change containers or images built from an
# earlier version of this file.
FROM debian:bookworm-slim
RUN echo "nx: nexus-code/Dockerfile is withdrawn during Phase Zero; no image is built" >&2; exit 1
