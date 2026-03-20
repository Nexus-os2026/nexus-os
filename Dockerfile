FROM rust:stable-bookworm AS backend-builder

WORKDIR /build
COPY . .
RUN cargo build --release --package nexus-protocols --bin nexus-server

FROM node:22-bookworm-slim AS frontend-builder

WORKDIR /build/app
COPY app/package.json app/package-lock.json ./
RUN npm ci
COPY app/ ./
RUN npm run build

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /bin/bash nexus \
    && mkdir -p /data /opt/nexus/app/dist \
    && chown -R nexus:nexus /data /opt/nexus

WORKDIR /opt/nexus

ENV NEXUS_HTTP_ADDR=0.0.0.0:8080 \
    NEXUS_FRONTEND_DIST=/opt/nexus/app/dist \
    NEXUS_CONFIG_PATH=/data/config

COPY --from=backend-builder /build/target/release/nexus-server /usr/local/bin/nexus-server
COPY --from=frontend-builder /build/app/dist ./app/dist

VOLUME ["/data"]

EXPOSE 8080 9090

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8080/health >/dev/null || exit 1

USER nexus

ENTRYPOINT ["nexus-server", "start"]
