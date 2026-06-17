#!/bin/sh
set -e

# ── Auto-generate self-signed TLS certificate on first run ──────────
CERT_DIR="${TLS_CERT_DIR:-/app/certs}"
KEY_FILE="${TLS_KEY:-${CERT_DIR}/marionette-key.pem}"
CERT_FILE="${TLS_CERT:-${CERT_DIR}/marionette-cert.pem}"

if [ -n "${TLS_KEY}" ] && [ -n "${TLS_CERT}" ] && [ -f "${TLS_KEY}" ] && [ -f "${TLS_CERT}" ]; then
  echo "Using provided TLS certificate: ${TLS_CERT}"
elif [ -f "${KEY_FILE}" ] && [ -f "${CERT_FILE}" ]; then
  echo "Using existing certificate from ${CERT_DIR}"
  export TLS_KEY="${KEY_FILE}"
  export TLS_CERT="${CERT_FILE}"
else
  echo ""
  echo "╔══════════════════════════════════════════════════════════════╗"
  echo "║  No TLS certificate found — generating self-signed cert     ║"
  echo "╠══════════════════════════════════════════════════════════════╣"
  echo "║  Mount your own cert at: /app/certs/                        ║"
  echo "║  Files: marionette-key.pem  marionette-cert.pem             ║"
  echo "║  Or set env: TLS_KEY=/path/key.pem TLS_CERT=/path/cert.pem  ║"
  echo "╚══════════════════════════════════════════════════════════════╝"
  echo ""

  mkdir -p "${CERT_DIR}"
  openssl req -x509 -newkey rsa:4096 -sha256 -days 3650 -nodes \
    -keyout "${KEY_FILE}" \
    -out "${CERT_FILE}" \
    -subj "/CN=marionette" \
    -addext "subjectAltName=DNS:localhost,IP:127.0.0.1" \
    2>/dev/null
  chmod 600 "${KEY_FILE}"

  export TLS_KEY="${KEY_FILE}"
  export TLS_CERT="${CERT_FILE}"
  echo "Self-signed certificate generated at ${CERT_DIR}/"
  echo ""
fi

# ── Ensure required directories ──────────────────────────────────────
mkdir -p /stacks /data

# ── Start supervisor (manages core + gateway + nginx) ─────────────────
export TLS_KEY TLS_CERT
exec /usr/bin/supervisord -c /app/supervisord.conf
