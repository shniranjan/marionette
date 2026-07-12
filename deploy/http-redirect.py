#!/usr/bin/env python3
"""HTTP → HTTPS redirect server for Marionette.
Listens on MARIONETTE_PORT, redirects all requests to https://HOST:HTTPS_PORT."""

import http.server
import os
import sys

HTTP_PORT = int(os.environ.get("MARIONETTE_PORT", "8000"))
HTTPS_PORT = int(os.environ.get("MARIONETTE_GATEWAY_PORT", "8443"))

class RedirectHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        host = self.headers.get("Host", f"localhost:{HTTP_PORT}").split(":")[0]
        self.send_response(301)
        self.send_header("Location", f"https://{host}:{HTTPS_PORT}{self.path}")
        self.end_headers()

    do_POST = do_GET
    do_PUT = do_GET
    do_DELETE = do_GET

    def log_message(self, format, *args):
        sys.stderr.write(f"{self.client_address[0]} → {format % args}\n")
        sys.stderr.flush()

server = http.server.HTTPServer(("0.0.0.0", HTTP_PORT), RedirectHandler)
print(f"HTTP→HTTPS redirect on :{HTTP_PORT} → :{HTTPS_PORT}", flush=True)
server.serve_forever()
