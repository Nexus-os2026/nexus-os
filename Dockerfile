# Stage 1: Build
FROM rust:1.82-bookworm AS builder

WORKDIR /build
COPY . .
RUN cargo build --release --package nexus-protocols --bin nexus-server

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --shell /bin/bash nexus
USER nexus

COPY --from=builder /build/target/release/nexus-server /usr/local/bin/nexus-server

RUN mkdir -p /home/nexus/.nexus

EXPOSE 8080 8081

ENTRYPOINT ["nexus-server"]
