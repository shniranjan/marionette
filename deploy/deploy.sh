#!/usr/bin/env bash
set -euo pipefail

# Marionette deployment script
# Usage: ./deploy/deploy.sh [marionette|relay|all|verify]

REGISTRY="${REGISTRY:-ghcr.io/nous}"
IMAGE="${IMAGE:-marionette}"
TAG="${TAG:-latest}"
MARIONETTE_HOST="${MARIONETTE_HOST:-192.168.1.59}"
RELAY_HOST="${RELAY_HOST:-192.168.1.17}"
SSH_USER="${SSH_USER:-root}"
FULL_IMAGE="${REGISTRY}/${IMAGE}:${TAG}"

# ── build_and_push: build multi-arch image and push to registry ──
build_and_push() {
  if ! docker buildx version &>/dev/null; then
    echo "Error: docker buildx not available" >&2
    exit 1
  fi
  echo "Building and pushing ${FULL_IMAGE} ..."
  docker buildx build --platform linux/amd64,linux/arm64 \
    -t "${FULL_IMAGE}" --push .
  echo "Push complete: ${FULL_IMAGE}"
}

# ── deploy_marionette: deploy core to MARIONETTE_HOST (.59) ──
deploy_marionette() {
  echo "=== Deploying marionette to ${MARIONETTE_HOST} ==="
  ssh "${SSH_USER}@${MARIONETTE_HOST}" <<'ENDSSH'
    set -euo pipefail
    cd /opt/marionette
    docker compose -f docker-compose.prod.yml pull marionette 2>/dev/null || echo "Pull skipped (image may be local)"
    docker compose -f docker-compose.prod.yml up -d marionette
    echo "Waiting for health check..."
    for i in $(seq 1 30); do
      if curl -sf http://localhost:9119/health >/dev/null 2>&1; then
        echo "✓ Marionette healthy on localhost:9119"
        exit 0
      fi
      sleep 2
    done
    echo "✗ Marionette health check timed out after 60s" >&2
    exit 1
ENDSSH
}

# ── deploy_relay: deploy relay agent to RELAY_HOST (.17) ──
deploy_relay() {
  echo "=== Deploying relay agent to ${RELAY_HOST} ==="
  ssh "${SSH_USER}@${RELAY_HOST}" <<'ENDSSH'
    set -euo pipefail
    cd /opt/marionette
    docker compose -f docker-compose.prod.yml --profile relay pull relay-agent 2>/dev/null || echo "Pull skipped (image may be local)"
    docker compose -f docker-compose.prod.yml --profile relay up -d relay-agent
    echo "Waiting for health check..."
    for i in $(seq 1 30); do
      if curl -sf http://localhost:9120/health >/dev/null 2>&1; then
        echo "✓ Relay healthy on localhost:9120"
        exit 0
      fi
      sleep 2
    done
    echo "✗ Relay health check timed out after 60s" >&2
    exit 1
ENDSSH
}

# ── verify: health-check both hosts ──
verify() {
  echo "=== Verifying deployments ==="
  if curl -sf "http://${MARIONETTE_HOST}:9119/health" >/dev/null 2>&1; then
    echo "✓ Marionette healthy — http://${MARIONETTE_HOST}:9119/health"
  else
    echo "✗ Marionette UNHEALTHY — http://${MARIONETTE_HOST}:9119/health" >&2
  fi

  if curl -sf "http://${RELAY_HOST}:9120/health" >/dev/null 2>&1; then
    echo "✓ Relay healthy — http://${RELAY_HOST}:9120/health"
  else
    echo "✗ Relay UNHEALTHY — http://${RELAY_HOST}:9120/health" >&2
  fi
}

# ── main ──
case "${1:-all}" in
  build)
    build_and_push
    ;;
  marionette)
    deploy_marionette
    ;;
  relay)
    deploy_relay
    ;;
  all)
    deploy_marionette
    deploy_relay
    verify
    ;;
  verify)
    verify
    ;;
  *)
    echo "Usage: $0 {marionette|relay|all|verify|build}"
    echo ""
    echo "  marionette  Deploy marionette core to ${MARIONETTE_HOST}"
    echo "  relay       Deploy relay agent to ${RELAY_HOST}"
    echo "  all         Deploy both + verify (default)"
    echo "  verify      Health-check both hosts"
    echo "  build       Build and push multi-arch image to ${REGISTRY}/${IMAGE}"
    exit 1
    ;;
esac
