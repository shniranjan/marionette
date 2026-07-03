# relay-agent — Marionette relay for Docker hosts
# Multi-stage build — builds the Rust binary and produces the final image.
# Build on the target host:
#   docker build -t relay-agent:latest .
FROM rust:1.91-bookworm AS builder

WORKDIR /build

# Copy workspace Cargo.toml
COPY Cargo.toml Cargo.lock* ./

# Copy crate sources
COPY core/Cargo.toml core/
COPY core/src/ core/src/
COPY crates/relay-protocol/Cargo.toml crates/relay-protocol/
COPY crates/relay-protocol/src/ crates/relay-protocol/src/
COPY crates/relay-agent/Cargo.toml crates/relay-agent/
COPY crates/relay-agent/src/ crates/relay-agent/src/

# Build release binary
RUN cargo build --release -p relay-agent && \
    cp target/release/relay-agent /usr/local/bin/relay-agent

# ── Runtime stage ──
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    adduser ca-certificates && \
    rm -rf /var/lib/apt/lists/* && \
    addgroup --system relay && \
    adduser --system --group relay

COPY --from=builder /usr/local/bin/relay-agent /usr/local/bin/relay-agent
COPY crates/relay-agent/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

ENV RUST_LOG=info
ENV MARIONETTE_URL=ws://marionette:9119/relay

HEALTHCHECK --interval=15s --timeout=3s --start-period=5s --retries=3 \
    CMD kill -0 1

ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
