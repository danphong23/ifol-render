"""
Asset server for ifol-render SDK test.
Serves local files via query param, handles export via CLI.

Usage:
    python web/asset-server.py [port]

Endpoints:
    GET  /asset?path=C:/path/to/file  → serves the file (supports Range)
    GET  /health                       → {"status":"ok"}
    POST /export                       → runs ifol-render.exe export
"""

import http.server
import urllib.parse
import os
import json
import sys
import subprocess
import tempfile
import time

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
PROJECT_DIR = os.path.dirname(SCRIPT_DIR)
CLI_PATH = os.path.join(PROJECT_DIR, 'target', 'release', 'ifol-render.exe')
if not os.path.isfile(CLI_PATH):
    CLI_PATH = os.path.join(PROJECT_DIR, 'target', 'debug', 'ifol-render.exe')

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
                self._error(404, 'File not found')
                return
            
            file_size = os.path.getsize(file_path)
            content_type = self._mime(file_path)
            
            # Range request support (for video seeking)
            range_header = self.headers.get('Range')
            if range_header:
                self._serve_range(file_path, file_size, content_type, range_header)
            else:
                self._serve_full(file_path, file_size, content_type)
            return
        
        self._error(404, 'Not found')

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        
        if parsed.path == '/export':
            content_length = int(self.headers.get('Content-Length', 0))
            body = self.rfile.read(content_length).decode()
            
            try:
                data = json.loads(body) if body else {}
                result = self._run_export(data)
                self._json_response(200, result)
            except Exception as e:
                self._json_response(500, {"status": "error", "error": str(e)})
            return
        
        self._error(404, 'Not found')

    def do_OPTIONS(self):
        self.send_response(200)
        self._cors_headers()
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', '*')
        self.end_headers()

    # ── Export ──

    def _run_export(self, data):
        """Run ifol-render.exe export with scene JSON."""
        if not os.path.isfile(CLI_PATH):
            return {"status": "error", "error": f"CLI not found: {CLI_PATH}"}
        
        # Build proper scene JSON with settings embedded
        frames = data.get('frames', [])
        scene_path = data.get('scene_path')
        output_path = data.get('output', os.path.join(tempfile.gettempdir(), f'ifol_export_{int(time.time())}.mp4'))
        
        settings = data.get('settings', {})
        width = settings.get('width', data.get('width', 1920))
        height = settings.get('height', data.get('height', 1080))
        fps = settings.get('fps', data.get('fps', 30))
        
        if scene_path and os.path.isfile(scene_path):
            pass
        elif data.get('scene_json'):
            # V2 Scene JSON from web test
            scene_path = os.path.join(tempfile.gettempdir(), f'ifol_scene_{int(time.time())}.json')
            with open(scene_path, 'w') as f:
                json.dump(data['scene_json'], f)
            print(f'[export] Saved V2 scene to {scene_path}')
        elif frames:
            scene_data = {
                "settings": {
                    "width": width,
                    "height": height,
                    "fps": fps,
                    "background": settings.get('background', [0, 0, 0, 1])
                },
                "frames": frames
            }
            if 'audio_clips' in data:
                scene_data['audio_clips'] = data['audio_clips']
            
            scene_path = os.path.join(tempfile.gettempdir(), f'ifol_scene_{int(time.time())}.json')
            with open(scene_path, 'w') as f:
                json.dump(scene_data, f)
        else:
            return {"status": "error", "error": "No frames, scene_json, or scene_path provided"}
        
        # Build CLI command with all params
        cmd = [CLI_PATH, 'export', '--scene', scene_path, '-o', output_path]
        
        # Optional CLI flags
        if data.get('ffmpeg'):
            cmd.extend(['--ffmpeg', data['ffmpeg']])
        if data.get('codec'):
            cmd.extend(['--codec', data['codec']])
        if data.get('crf') is not None:
            cmd.extend(['--crf', str(data['crf'])])
        if data.get('preset'):
            cmd.extend(['--preset', data['preset']])
        if data.get('pixel_format'):
            cmd.extend(['--pixel-format', data['pixel_format']])
        # --width/--height override scene settings if provided
        if data.get('width'):
            cmd.extend(['--width', str(data['width'])])
        if data.get('height'):
            cmd.extend(['--height', str(data['height'])])
        
        print(f'[export] Running: {" ".join(cmd)}')
        
        try:
            # Use Popen to stream stderr progress in real-time
            proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            stderr_lines = []
            import select
            import threading
            
            # Read stderr in a thread to avoid blocking
            def read_stderr():
                for line in iter(proc.stderr.readline, b''):
                    decoded = line.decode('utf-8', errors='replace').rstrip()
                    if decoded:
                        stderr_lines.append(decoded)
                        print(f'[export] {decoded}')
            
            t = threading.Thread(target=read_stderr, daemon=True)
            t.start()
            
            proc.wait(timeout=600)
            t.join(timeout=2)
            
            stdout = proc.stdout.read().decode('utf-8', errors='replace')
            stderr_text = '\n'.join(stderr_lines)
            
            if proc.returncode == 0:
                return {
                    "status": "ok",
                    "path": output_path,
                    "url": f"/asset?path={urllib.parse.quote(output_path)}",
                    "stdout": stdout[-500:] if stdout else "",
                    "progress": stderr_text[-500:] if stderr_text else "",
                }
            else:
                return {
                    "status": "error",
                    "error": stderr_text[-500:] if stderr_text else f"Exit code {proc.returncode}",
                    "stdout": stdout[-500:] if stdout else "",
                }
        except subprocess.TimeoutExpired:
            proc.kill()
            return {"status": "error", "error": "Export timed out (600s)"}

    # ── File serving ──

    def _serve_full(self, path, size, content_type):
        try:
            with open(path, 'rb') as f:
                data = f.read()
            self.send_response(200)
            self.send_header('Content-Type', content_type)
            self.send_header('Content-Length', str(size))
            self.send_header('Accept-Ranges', 'bytes')
            self._cors_headers()
            self.send_header('Cache-Control', 'public, max-age=3600')
            self.end_headers()
            self.wfile.write(data)
        except Exception as e:
            self._error(500, str(e))

    def _serve_range(self, path, size, content_type, range_header):
        """Handle HTTP Range requests (for video seeking)."""
        try:
            ranges = range_header.replace('bytes=', '').split('-')
            start = int(ranges[0]) if ranges[0] else 0
            end = int(ranges[1]) if ranges[1] else size - 1
            end = min(end, size - 1)
            length = end - start + 1

            with open(path, 'rb') as f:
                f.seek(start)
                data = f.read(length)

            self.send_response(206)
            self.send_header('Content-Type', content_type)
            self.send_header('Content-Range', f'bytes {start}-{end}/{size}')
            self.send_header('Content-Length', str(length))
            self.send_header('Accept-Ranges', 'bytes')
            self._cors_headers()
            self.end_headers()
            self.wfile.write(data)
        except Exception as e:
            self._error(500, str(e))

    # ── Helpers ──

    def _cors_headers(self):
        self.send_header('Access-Control-Allow-Origin', '*')

    def _json_response(self, code, data):
        body = json.dumps(data).encode()
        self.send_response(code)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self._cors_headers()
        self.end_headers()
        self.wfile.write(body)

    def _error(self, code, msg):
        self.send_response(code)
        self._cors_headers()
        self.end_headers()
        self.wfile.write(msg.encode())

    def _mime(self, path):
        ext = os.path.splitext(path)[1].lower()
        return {
            '.png':'image/png','.jpg':'image/jpeg','.jpeg':'image/jpeg',
            '.gif':'image/gif','.webp':'image/webp','.svg':'image/svg+xml',
            '.mp4':'video/mp4','.webm':'video/webm','.mov':'video/quicktime',
            '.mp3':'audio/mpeg','.wav':'audio/wav','.ogg':'audio/ogg',
            '.ttf':'font/ttf','.otf':'font/otf','.woff':'font/woff',
            '.woff2':'font/woff2','.json':'application/json',
        }.get(ext, 'application/octet-stream')

    def log_message(self, format, *args):
        try:
            first = str(args[0]) if args else ''
            if '/health' not in first:
                super().log_message(format, *args)
        except Exception:
            pass

if __name__ == '__main__':
    server = http.server.HTTPServer(('', PORT), AssetHandler)
    print(f'Asset server on http://localhost:{PORT}')
    print(f'  GET  /asset?path=C:/path/to/file')
    print(f'  POST /export  {{frames: [...], output: "..."}}'  )
    print(f'  CLI:  {CLI_PATH}')
    print(f'  CLI exists: {os.path.isfile(CLI_PATH)}')
    server.serve_forever()
