# WITHDRAWN during Phase Zero (P0-FG1).
#
# This recipe built crates/nexus-server into an image that served an HTTP API,
# the nexus-mcp tools and an A2A endpoint on ports 3000-3002 of every
# interface, with no authentication. That server is withdrawn.
#
# Building this file fails at its first step, on purpose. It installs no
# packages, copies no source, compiles nothing and produces no image.
#
# It does not stop, remove or change containers or images built from an
# earlier version of this file.
FROM debian:bookworm-slim
RUN echo "nexus-server: deploy/Dockerfile is withdrawn during Phase Zero; no image is built" >&2; exit 1
