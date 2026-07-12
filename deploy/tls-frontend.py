#!/usr/bin/env python3
"""TLS reverse proxy for Marionette — HTTPS on MARIONETTE_GATEWAY_PORT.
Proxies /api/*, /relay, /health → core API. Serves frontend for /*."""

import http.server
import ssl
import os
import sys
import urllib.request
import urllib.error

CERT_DIR = "/app/certs"
PORT = int(os.environ.get("MARIONETTE_GATEWAY_PORT", "8443"))
CORE_PORT = int(os.environ.get("MARIONETTE_CORE_PORT", "8001"))
FRONTEND_DIR = "/opt/marionette/frontend"

PROXY_PREFIXES = ("/api/", "/relay", "/health")

class ProxyHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=FRONTEND_DIR, **kwargs)

    def do_GET(self):
        if any(self.path.startswith(p) for p in PROXY_PREFIXES):
            self.proxy_request("GET")
        else:
            super().do_GET()

    def do_POST(self):
        if any(self.path.startswith(p) for p in PROXY_PREFIXES):
            self.proxy_request("POST")
        else:
            super().do_GET()

    def proxy_request(self, method):
        try:
            url = f"http://127.0.0.1:{CORE_PORT}{self.path}"
            body = None
            if method == "POST":
                length = int(self.headers.get("Content-Length", 0))
                body = self.rfile.read(length) if length else None
            req = urllib.request.Request(url, data=body, method=method)
            for k, v in self.headers.items():
                if k.lower() not in ("host", "connection"):
                    req.add_header(k, v)
            with urllib.request.urlopen(req, timeout=30) as resp:
                self.send_response(resp.status)
                for k, v in resp.getheaders():
                    if k.lower() not in ("transfer-encoding", "connection"):
                        self.send_header(k, v)
                self.end_headers()
                self.wfile.write(resp.read())
        except urllib.error.HTTPError as e:
            self.send_response(e.code)
            self.end_headers()
            self.wfile.write(e.read())
        except Exception as e:
            self.send_response(502)
            self.end_headers()
            self.wfile.write(f"Proxy error: {e}".encode())

    def log_message(self, format, *args):
        sys.stderr.write(f"{self.client_address[0]} - {format % args}\n")
        sys.stderr.flush()

# --- Startup ---
print(f"TLS proxy starting — cert dir: {CERT_DIR}", flush=True)
if os.path.isdir(CERT_DIR):
    print(f"Files: {os.listdir(CERT_DIR)}", flush=True)

cert_file = key_file = None
for f in sorted(os.listdir(CERT_DIR)):
    path = os.path.join(CERT_DIR, f)
    if not os.path.isfile(path):
        continue
    with open(path, "rb") as fp:
        head = fp.read(100)
    if not cert_file and (b"CERTIFICATE" in head or b"-----BEGIN" in head):
        if f.endswith((".crt", ".cert", ".pem", ".cer")) or "cert" in f.lower():
            cert_file = path
            print(f"Found cert: {f}", flush=True)
    if not key_file and b"PRIVATE KEY" in head:
        key_file = path
        print(f"Found key: {f}", flush=True)

if cert_file and key_file:
    server = http.server.HTTPServer(("0.0.0.0", PORT), ProxyHandler)
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(cert_file, key_file)
    server.socket = ctx.wrap_socket(server.socket, server_side=True)
    print(f"HTTPS proxy on :{PORT} → core :{CORE_PORT}", flush=True)
else:
    print(f"WARNING: cert/key missing, plain HTTP on :{PORT}", flush=True)
    server = http.server.HTTPServer(("0.0.0.0", PORT), ProxyHandler)

server.serve_forever()
