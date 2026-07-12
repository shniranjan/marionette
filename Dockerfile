# ── STAGE 1: Rust builder ──────────────────────────────────────────────
FROM rust:1.96-alpine AS rust-builder
RUN apk add --no-cache musl-dev openssl-dev pkgconfig
WORKDIR /build

# Copy everything and build (single pass — no layered caching tricks)
COPY . .
RUN cargo build --release --bin marionette-core --bin relay-agent

# Strip debug symbols
RUN strip /build/target/release/marionette-core
RUN strip /build/target/release/relay-agent

# ── STAGE 2: Node.js frontend builder ──────────────────────────────────
FROM node:22-alpine AS node-builder
WORKDIR /build
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build   # → frontend/dist/

# ── STAGE 3: Runtime ───────────────────────────────────────────────────
FROM alpine:3.21
RUN apk add --no-cache \
    ca-certificates \
    tzdata \
    curl \
    bash \
    docker-cli \
    docker-compose \
    supervisor \
    python3

# Copy Rust binaries
COPY --from=rust-builder /build/target/release/marionette-core /usr/local/bin/marionette-core
COPY --from=rust-builder /build/target/release/relay-agent /usr/local/bin/relay-agent

# Copy frontend static assets
COPY --from=node-builder /build/dist /opt/marionette/frontend

# Copy supervisor config
COPY deploy/supervisord.conf /etc/supervisord.conf

# Copy entrypoint
COPY deploy/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

EXPOSE 9119 9120 3000

ENV MARIONETTE_PORT=9119
ENV MARIONETTE_GATEWAY_PORT=3000
ENV MARIONETTE_RELAY_PORT=9120

ENTRYPOINT ["docker-entrypoint.sh"]
