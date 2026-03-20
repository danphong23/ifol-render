import http.server
import socketserver
import urllib.parse
import subprocess
import os

PORT = 8000

class FileAndMediaHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Access-Control-Allow-Origin', '*')
        super().end_headers()

    def do_OPTIONS(self):
        self.send_response(200)
        self.send_header('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', '*')
        self.end_headers()

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == "/export":
            content_length = int(self.headers.get('Content-Length', 0))
            body = self.rfile.read(content_length)
            
            temp_json = os.path.abspath(os.path.join(os.path.dirname(__file__), "temp_export.json"))
            with open(temp_json, "wb") as f:
                f.write(body)
            
            output_path = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "web_export.mp4"))
            exe_path = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "target", "debug", "ifol-render.exe"))
            root_cwd = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
            
            if not os.path.exists(exe_path):
                self.send_response(500)
                self.end_headers()
                self.wfile.write(f"CLI not found at {exe_path}. Run: cargo build -p ifol-render-cli".encode())
                return
            
            cmd = [
                exe_path, "export",
                "--scene", temp_json,
                "--output", output_path,
                "--codec", "h264",
                "--crf", "23",
                "--preset", "fast",
            ]
                
            print(f"\n{'='*60}")
            print(f"EXPORT STARTED")
            print(f"Command: {' '.join(cmd)}")
            print(f"Output:  {output_path}")
            print(f"{'='*60}")
            
            # Start process in background — stderr streams to this terminal
            subprocess.Popen(cmd, cwd=root_cwd)
            
            self.send_response(200)
            self.end_headers()
            msg = f"Export started! Watch server terminal for progress.\nOutput will be saved to: {output_path}"
            self.wfile.write(msg.encode())
            return

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        qs = urllib.parse.parse_qs(parsed.query)

        if parsed.path == "/asset":
            filepath = qs.get("path", [""])[0]
            # Convert to absolute if it's a relative path from the project root
            # E.g. assets/fonts/noto.ttf -> C:\Users\abc\.AI\Code\ifol-render\assets\fonts\noto.ttf
            if not os.path.isabs(filepath):
                filepath = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", filepath))

            if os.path.exists(filepath):
                self.send_response(200)
                self.end_headers()
                with open(filepath, 'rb') as f:
                    self.wfile.write(f.read())
            else:
                self.send_response(404)
                self.end_headers()
                self.wfile.write(b"Not Found")
            return

        elif parsed.path == "/video_frame":
            filepath = qs.get("path", [""])[0]
            time = qs.get("time", ["0"])[0]
            if not os.path.isabs(filepath):
                filepath = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", filepath))

            if os.path.exists(filepath):
                try:
                    # Extracts a single JPEG frame at the given timestamp
                    cmd = [
                        "ffmpeg", "-y", "-ss", str(time), "-i", filepath,
                        "-vframes", "1", "-f", "image2pipe", "-vcodec", "mjpeg", "-"
                    ]
                    result = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                    if result.returncode == 0:
                        self.send_response(200)
                        self.send_header("Content-Type", "image/jpeg")
                        self.end_headers()
                        self.wfile.write(result.stdout)
                    else:
                        print("FFmpeg error:", result.stderr.decode("utf-8"))
                        self.send_response(500)
                        self.end_headers()
                except Exception as e:
                    self.send_response(500)
                    self.end_headers()
            else:
                self.send_response(404)
                self.end_headers()
            return
            
        elif parsed.path == "/video_info":
            filepath = qs.get("path", [""])[0]
            if not os.path.isabs(filepath):
                filepath = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", filepath))

            if os.path.exists(filepath):
                cmd = [
                    "ffprobe", "-v", "error", "-show_entries", "format=duration,bit_rate",
                    "-show_streams", "-print_format", "json", filepath
                ]
                result = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                if result.returncode == 0:
                    self.send_response(200)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(result.stdout)
                else:
                    self.send_response(500)
                    self.end_headers()
            else:
                self.send_response(404)
                self.end_headers()
            return

        # Fallback to normal HTTP hosting
        super().do_GET()

with socketserver.TCPServer(("", PORT), FileAndMediaHandler) as httpd:
    print(f"Asset Server serving at port {PORT}")
    httpd.serve_forever()
