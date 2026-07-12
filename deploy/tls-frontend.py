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

# Log what we find
print(f"TLS frontend starting — cert dir: {CERT_DIR}", flush=True)
if os.path.isdir(CERT_DIR):
    files = os.listdir(CERT_DIR)
    print(f"Files in {CERT_DIR}: {files}", flush=True)
else:
    print(f"ERROR: {CERT_DIR} is not a directory", flush=True)

# Find cert and key files — try multiple extension patterns
cert_file = None
key_file = None
for f in sorted(os.listdir(CERT_DIR)):
    path = os.path.join(CERT_DIR, f)
    if not os.path.isfile(path):
        continue
    lower = f.lower()
    # Cert: .crt, .pem, .cert, .cer, or contains 'cert' and not 'key'
    if not cert_file and (lower.endswith(('.crt', '.cert', '.cer')) or ('cert' in lower and 'key' not in lower) or lower.endswith('.pem')):
        # Read first few bytes to check if it's a certificate
        with open(path, 'rb') as fp:
            head = fp.read(50)
        if b'CERTIFICATE' in head or b'-----BEGIN' in head:
            cert_file = path
            print(f"Found cert: {f}", flush=True)
    # Key: .key, or contains 'key' and not 'cert'
    if not key_file and (lower.endswith('.key') or ('key' in lower and 'cert' not in lower)):
        with open(path, 'rb') as fp:
            head = fp.read(50)
        if b'PRIVATE KEY' in head or b'-----BEGIN' in head:
            key_file = path
            print(f"Found key: {f}", flush=True)

if cert_file and key_file:
    os.chdir(FRONTEND_DIR)
    server = http.server.HTTPServer(("0.0.0.0", PORT), http.server.SimpleHTTPRequestHandler)
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(cert_file, key_file)
    server.socket = ctx.wrap_socket(server.socket, server_side=True)
    print(f"Serving HTTPS on :{PORT}", flush=True)
    server.serve_forever()
else:
    print(f"WARNING: cert/key not found. Serving plain HTTP on :{PORT}", flush=True)
    if cert_file:
        print(f"  Have cert: {cert_file}, missing key", flush=True)
    elif key_file:
        print(f"  Have key: {key_file}, missing cert", flush=True)
    os.chdir(FRONTEND_DIR)
    server = http.server.HTTPServer(("0.0.0.0", PORT), http.server.SimpleHTTPRequestHandler)
    server.serve_forever()
