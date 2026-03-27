# Stage 1: Build
FROM rust:1.82-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

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

# Build only the server binary
RUN cargo build --release -p nexus-server

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -s /bin/bash nexus

WORKDIR /app

COPY --from=builder /app/target/release/nexus-server /app/nexus-server

# Copy agent manifests
COPY agents/prebuilt/ /app/agents/prebuilt/

RUN mkdir -p /app/nexus-data && chown nexus:nexus /app/nexus-data

USER nexus

EXPOSE 3000 3001 3002

VOLUME ["/app/nexus-data"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD curl -f http://localhost:3000/health || exit 1

ENTRYPOINT ["/app/nexus-server"]
CMD ["--port", "3000", "--mcp-port", "3001", "--a2a-port", "3002", "--data-dir", "/app/nexus-data"]
