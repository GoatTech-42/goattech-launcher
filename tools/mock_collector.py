#!/usr/bin/env python3
"""Local stand-in for the GoatTech telemetry collector, used by the
runtime-capture workflow so the on-state "I'm playing" ping note can be
photographed without any contact leaving 127.0.0.1. Answers GET /healthz,
GET /v1/live (empty count) and POST /v1/live (204), and appends one line per
request (method, path, capped POST body) to the log path in argv[2], so the
artifact records exactly what the app asked for."""
import json
import sys
from http.server import BaseHTTPRequestHandler, HTTPServer


class Handler(BaseHTTPRequestHandler):
    def _log(self, extra=""):
        with open(sys.argv[2], "a") as f:
            f.write(f"{self.command} {self.path} {extra}\n")

    def log_message(self, *args):
        pass

    def do_GET(self):
        if self.path == "/healthz":
            self._log()
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"ok")
        elif self.path == "/v1/live":
            self._log()
            body = json.dumps({"playing": 0, "lan": 0, "versions": {}, "platforms": {}}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(body)
        else:
            self._log("-> 404")
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        n = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(n) if n else b""
        if self.path == "/v1/live":
            self._log(f"-> 204 body={body[:512]!r}")
            self.send_response(204)
            self.end_headers()
        else:
            self._log("-> 404")
            self.send_response(404)
            self.end_headers()


if __name__ == "__main__":
    host, port = sys.argv[1].rsplit(":", 1)
    HTTPServer((host, int(port)), Handler).serve_forever()
