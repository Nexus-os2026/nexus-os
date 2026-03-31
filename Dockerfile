# ── Stage 1: Rust backend ────────────────────────────────────────────
FROM rust:1.82-bookworm AS backend-builder

WORKDIR /build

# Copy workspace manifests first for layer caching
COPY Cargo.toml Cargo.lock ./

# Copy all workspace member directories
COPY kernel/ kernel/
COPY sdk/ sdk/
COPY connectors/ connectors/
COPY protocols/ protocols/
COPY crates/ crates/
COPY agents/ agents/
COPY workflows/ workflows/
COPY cli/ cli/
COPY research/ research/
COPY content/ content/
COPY analytics/ analytics/
COPY adaptation/ adaptation/
COPY control/ control/
COPY factory/ factory/
COPY marketplace/ marketplace/
COPY self-update/ self-update/
COPY distributed/ distributed/
COPY enterprise/ enterprise/
COPY cloud/ cloud/
COPY persistence/ persistence/
COPY auth/ auth/
COPY telemetry/ telemetry/
COPY tenancy/ tenancy/
COPY integrations/ integrations/
COPY metering/ metering/
COPY llama-bridge/ llama-bridge/
COPY packaging/ packaging/
COPY tests/ tests/
COPY benchmarks/ benchmarks/
COPY app/src-tauri/ app/src-tauri/

RUN cargo build --release --package nexus-protocols --bin nexus-server

# ── Stage 2: React frontend ─────────────────────────────────────────
FROM node:22-bookworm-slim AS frontend-builder

WORKDIR /build/app
COPY app/package.json app/package-lock.json ./
RUN npm ci
COPY app/ ./
RUN npm run build

# ── Stage 3: Minimal runtime ────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

LABEL org.opencontainers.image.title="Nexus OS" \
      org.opencontainers.image.description="The Governed Agentic AI Operating System" \
      org.opencontainers.image.version="10.5.0" \
      org.opencontainers.image.source="https://gitlab.com/nexaiceo/nexus-os" \
      org.opencontainers.image.licenses="MIT"

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
