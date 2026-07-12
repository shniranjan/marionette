#!/usr/bin/env python3
"""TLS-wrapped HTTP server for Marionette frontend.
Reads cert/key from /app/certs, serves frontend on MARIONETTE_GATEWAY_PORT."""

import http.server
import ssl
import os
import sys

CERT_DIR = "/app/certs"
PORT = int(os.environ.get("MARIONETTE_GATEWAY_PORT", "8443"))
FRONTEND_DIR = "/opt/marionette/frontend"

# Find cert and key files
cert_file = None
key_file = None
for f in os.listdir(CERT_DIR):
    path = os.path.join(CERT_DIR, f)
    if not os.path.isfile(path):
        continue
    if f.endswith(".crt") or f.endswith(".pem") or f.endswith(".cert"):
        cert_file = path
    elif f.endswith(".key"):
        key_file = path

if not cert_file or not key_file:
    print(f"TLS cert/key not found in {CERT_DIR}", file=sys.stderr)
    # Fall back to plain HTTP
    os.chdir(FRONTEND_DIR)
    server = http.server.HTTPServer(("0.0.0.0", PORT), http.server.SimpleHTTPRequestHandler)
    print(f"WARNING: serving HTTP (no TLS) on :{PORT}", file=sys.stderr)
else:
    os.chdir(FRONTEND_DIR)
    server = http.server.HTTPServer(("0.0.0.0", PORT), http.server.SimpleHTTPRequestHandler)
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(cert_file, key_file)
    server.socket = ctx.wrap_socket(server.socket, server_side=True)
    print(f"HTTPS frontend on :{PORT} (cert={os.path.basename(cert_file)})", file=sys.stderr)

sys.stderr.flush()
server.serve_forever()
