# Build stage
FROM rust:1.82-slim AS builder
WORKDIR /build
COPY . .
RUN cargo build --release --bin nx

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    git ca-certificates ripgrep && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/nx /usr/local/bin/nx
ENTRYPOINT ["nx"]
CMD ["--help"]
