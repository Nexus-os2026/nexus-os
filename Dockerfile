# WITHDRAWN during Phase Zero (P0-FINAL-GATE-CLOSURE, dossier items J2 and J3).
#
# This recipe built the protocols server (`nexus-protocols-server`) into an
# image that ran the HTTP gateway on port 8080 of every interface, exposed
# ports 8080 and 9090 and served the frontend. That server is withdrawn: its
# binary only prints a withdrawal message.
#
# Building this file fails at its first step, on purpose. It installs no
# packages, copies no source, compiles nothing and produces no image.
#
# It does not stop, remove or change containers or images built from an
# earlier version of this file, and it does not remove images published
# elsewhere.
FROM debian:bookworm-slim
RUN echo "nexus-protocols-server: Dockerfile is withdrawn during Phase Zero; no image is built" >&2; exit 1
