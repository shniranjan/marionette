#!/bin/bash
# Generate a self-signed TLS certificate for Marionette.
# Usage: ./scripts/generate-cert.sh [hostname]
# Default hostname: localhost
#
# Output: certs/marionette-key.pem  certs/marionette-cert.pem
#
# Then run Marionette with:
#   TLS_KEY=/app/certs/marionette-key.pem TLS_CERT=/app/certs/marionette-cert.pem

set -euo pipefail

HOSTNAME="${1:-localhost}"
OUTDIR="${PWD}/certs"
DAYS=3650

mkdir -p "$OUTDIR"
cd "$OUTDIR"

echo "Generating self-signed certificate for: $HOSTNAME"
echo "Output directory: $OUTDIR"
echo ""

openssl req -x509 -newkey rsa:4096 -sha256 -days "$DAYS" -nodes \
  -keyout marionette-key.pem \
  -out marionette-cert.pem \
  -subj "/CN=${HOSTNAME}" \
  -addext "subjectAltName=DNS:${HOSTNAME},DNS:localhost,IP:127.0.0.1"

chmod 600 marionette-key.pem

echo ""
echo "Certificates generated:"
echo "  Key:  $OUTDIR/marionette-key.pem"
echo "  Cert: $OUTDIR/marionette-cert.pem"
echo ""
echo "Add to docker-compose.yml:"
echo "  environment:"
echo "    - TLS_KEY=/app/certs/marionette-key.pem"
echo "    - TLS_CERT=/app/certs/marionette-cert.pem"
echo "  volumes:"
echo "    - ./certs:/app/certs:ro"
echo ""
echo "Or for docker run:"
echo "  -e TLS_KEY=/app/certs/marionette-key.pem \\"
echo "  -e TLS_CERT=/app/certs/marionette-cert.pem \\"
echo "  -v \$(pwd)/certs:/app/certs:ro"
