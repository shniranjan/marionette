# relay-agent — Marionette relay for Docker hosts
# Build: from marionette repo root:
#   cargo build --release -p relay-agent
#   tar -czf - Dockerfile ../target/release/relay-agent | docker build -t relay-agent:latest -
FROM debian:bookworm-slim

RUN addgroup --system relay && adduser --system --group relay

COPY relay-agent /usr/local/bin/relay-agent

USER relay

ENV RUST_LOG=info
ENV MARIONETTE_URL=ws://marionette:9119/relay

HEALTHCHECK --interval=15s --timeout=3s --start-period=5s --retries=3 \
    CMD /usr/local/bin/relay-agent --health || exit 1

ENTRYPOINT ["/usr/local/bin/relay-agent"]
