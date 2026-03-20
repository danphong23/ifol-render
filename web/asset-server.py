"""
Lightweight asset server for ifol-render SDK test.
Serves files from any local path via query parameter.

Usage:
    python web/asset-server.py [port]

Endpoints:
    GET /asset?path=C:/path/to/file.png  → serves the file
    GET /health                           → {"status":"ok"}
"""

import http.server
import urllib.parse
import os
import json
import sys

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8000

class AssetHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        
        if parsed.path == '/health':
            self._json_response(200, {"status": "ok"})
            return

        if parsed.path == '/asset':
            params = urllib.parse.parse_qs(parsed.query)
            file_path = params.get('path', [None])[0]
            
            if not file_path or not os.path.isfile(file_path):
                self._cors_error(404, f'File not found: {file_path}')
                return
            
            ext = os.path.splitext(file_path)[1].lower()
            mime_map = {
                '.png': 'image/png', '.jpg': 'image/jpeg', '.jpeg': 'image/jpeg',
                '.gif': 'image/gif', '.webp': 'image/webp', '.svg': 'image/svg+xml',
                '.mp4': 'video/mp4', '.webm': 'video/webm', '.mov': 'video/quicktime',
                '.mp3': 'audio/mpeg', '.wav': 'audio/wav', '.ogg': 'audio/ogg',
                '.ttf': 'font/ttf', '.otf': 'font/otf', '.woff': 'font/woff',
                '.woff2': 'font/woff2', '.json': 'application/json',
            }
            content_type = mime_map.get(ext, 'application/octet-stream')
            
            try:
                file_size = os.path.getsize(file_path)
                self.send_response(200)
                self.send_header('Content-Type', content_type)
                self.send_header('Content-Length', str(file_size))
                self.send_header('Access-Control-Allow-Origin', '*')
                self.send_header('Cache-Control', 'public, max-age=3600')
                # Range support for video seeking
                self.send_header('Accept-Ranges', 'bytes')
                self.end_headers()
                with open(file_path, 'rb') as f:
                    # Stream in 64KB chunks for large files
                    while True:
                        chunk = f.read(65536)
                        if not chunk:
                            break
                        self.wfile.write(chunk)
            except Exception as e:
                self._cors_error(500, str(e))
            return
        
        self._cors_error(404, 'Not found')
    
    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', '*')
        self.end_headers()
    
    def _json_response(self, code, data):
        body = json.dumps(data).encode()
        self.send_response(code)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _cors_error(self, code, msg):
        body = msg.encode()
        self.send_response(code)
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        # Safe logging — avoid crash when args contain non-string types
        try:
            msg = format % args
            if '/health' not in msg:
                sys.stderr.write(f"{self.address_string()} - [{self.log_date_time_string()}] {msg}\n")
        except Exception:
            pass

if __name__ == '__main__':
    server = http.server.HTTPServer(('', PORT), AssetHandler)
    print(f'Asset server on http://localhost:{PORT}')
    print(f'Example: http://localhost:{PORT}/asset?path=C:/path/to/image.png')
    server.serve_forever()
