# ── Stage 1: Rust core build ─────────────────────────────────
FROM rust:1.96-alpine AS rust-builder
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig gcc make
WORKDIR /build
COPY core/Cargo.toml core/Cargo.lock ./
COPY core/src/ ./src/
RUN cargo build --release --bin marionette-core
RUN strip target/release/marionette-core

# ── Stage 2: Frontend build ──────────────────────────────────
FROM node:22-alpine AS frontend-builder
WORKDIR /build
COPY frontend/package.json ./
RUN npm install
COPY frontend/ ./
RUN npm run build

# ── Stage 3: Gateway build ───────────────────────────────────
FROM node:22-alpine AS gateway-builder
WORKDIR /build
COPY gateway/package.json gateway/package-lock.json ./
RUN npm install
COPY gateway/tsconfig.json ./
COPY gateway/src/ ./src/
RUN npm run build

# ── Stage 4: Runtime ─────────────────────────────────────────
FROM alpine:3.21
RUN apk add --no-cache \
    nodejs \
    npm \
    supervisor \
    curl \
    ca-certificates \
    docker-cli \
    docker-cli-compose \
    nginx

WORKDIR /app

# Copy Rust binary
COPY --from=rust-builder /build/target/release/marionette-core /usr/local/bin/

# Copy gateway (production deps + built JS)
COPY --from=gateway-builder /build/package.json /build/package-lock.json /app/gateway/
RUN cd /app/gateway && npm install --omit=dev
COPY --from=gateway-builder /build/dist/ /app/gateway/dist/

# Copy frontend SPA
COPY --from=frontend-builder /build/dist/ /app/frontend/dist/

# Copy supervisor config + entrypoint
COPY supervisord.conf /app/
COPY scripts/entrypoint.sh /usr/local/bin/
RUN chmod +x /usr/local/bin/entrypoint.sh

# Required dirs
RUN mkdir -p /data /stacks /etc/nginx/upstreams /run/nginx

# Configure nginx to include marionette upstreams
RUN echo 'include /etc/nginx/upstreams/*.conf;' >> /etc/nginx/http.d/default.conf

# Placeholder so nginx -t passes when no upstreams exist yet
RUN echo '# marionette placeholder' > /etc/nginx/upstreams/placeholder.conf

EXPOSE 8000
HEALTHCHECK --interval=15s --timeout=5s --retries=3 \
    CMD curl -sf http://localhost:9119/health || exit 1

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
