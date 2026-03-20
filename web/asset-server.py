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
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Access-Control-Allow-Origin', '*')
            self.end_headers()
            self.wfile.write(json.dumps({"status": "ok"}).encode())
            return

        if parsed.path == '/asset':
            params = urllib.parse.parse_qs(parsed.query)
            file_path = params.get('path', [None])[0]
            
            if not file_path or not os.path.isfile(file_path):
                self.send_response(404)
                self.send_header('Access-Control-Allow-Origin', '*')
                self.end_headers()
                self.wfile.write(b'File not found')
                return
            
            # Guess MIME type
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
                with open(file_path, 'rb') as f:
                    data = f.read()
                self.send_response(200)
                self.send_header('Content-Type', content_type)
                self.send_header('Content-Length', str(len(data)))
                self.send_header('Access-Control-Allow-Origin', '*')
                self.send_header('Cache-Control', 'public, max-age=3600')
                self.end_headers()
                self.wfile.write(data)
            except Exception as e:
                self.send_response(500)
                self.send_header('Access-Control-Allow-Origin', '*')
                self.end_headers()
                self.wfile.write(str(e).encode())
            return
        
        self.send_response(404)
        self.end_headers()
    
    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'GET, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', '*')
        self.end_headers()
    
    def log_message(self, format, *args):
        path = args[0].split('?')[0] if args else ''
        if '/health' not in path:
            super().log_message(format, *args)

if __name__ == '__main__':
    server = http.server.HTTPServer(('', PORT), AssetHandler)
    print(f'Asset server on http://localhost:{PORT}')
    print(f'Example: http://localhost:{PORT}/asset?path=C:/path/to/image.png')
    server.serve_forever()
