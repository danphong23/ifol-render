import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';
import { spawn } from 'child_process';
import fs from 'fs';
import path from 'path';

let globalExportProgress = { status: 'idle', percent: 0, frame: 0, total: 0, fps: 0, eta: 0, error: '' };

export default defineConfig({
  plugins: [
    wasm(),
    topLevelAwait(),
    {
      name: 'export-proxy',
      configureServer(server) {
        server.middlewares.use('/api/export', (req, res) => {
          if (req.method === 'POST') {
            let body = '';
            req.on('data', chunk => body += chunk.toString());
            req.on('end', () => {
              try {
                const data = JSON.parse(body);
                // Save incoming scene JSON
                const tempPath = path.resolve('../temp_export.json');
                fs.writeFileSync(tempPath, JSON.stringify(data.scene));
                
                // Spawn Rust Backend CLI directly!
                // cwd corresponds to project root
                const cwd = path.resolve('..');
                
                // Reset progress state
                globalExportProgress = { status: 'started', percent: 0, frame: 0, total: 0, fps: 0, eta: 0, error: '' };

                // Prepare command line args
                const args = [
                   'run', '--release', '-p', 'ifol-render-cli', '--', 
                   'export', 
                   '--scene', tempPath, 
                   '--output', data.filename || 'output.mp4',
                   '--codec', data.codec || 'h264',
                   '--preset', data.preset || 'medium',
                   '--crf', data.crf ? data.crf.toString() : '23'
                ];
                
                // Add optional FFmpeg path if given
                if (data.ffmpeg) {
                    args.push('--ffmpeg', data.ffmpeg);
                }
                
                // Add optional custom FPS boundary
                if (data.fps) {
                    args.push('--fps', data.fps.toString());
                }

                // Set timer
                const startTime = performance.now();
                console.log(`[Backend-Export] Starting CLI Export: cargo ${args.join(' ')}`);
                const child = spawn('cargo', args, { cwd, shell: true });
                
                child.stdout.on('data', d => console.log(`[Export-Log] ${d.toString().trim()}`));
                child.stderr.on('data', d => {
                  const str = d.toString();
                  console.error(`[Export-Log] ${str.trim()}`);
                  
                  // Parse progress from stderr format: Frame 30/300 (10.0%) | 60.0 fps | ETA: 4s
                  const match = str.match(/Frame (\d+)\/(\d+) \(([\d.]+)%\) \| ([\d.]+) fps \| ETA: ([\d.]+)s/);
                  if (match) {
                     globalExportProgress.status = 'exporting';
                     globalExportProgress.frame = parseInt(match[1], 10);
                     globalExportProgress.total = parseInt(match[2], 10);
                     globalExportProgress.percent = parseFloat(match[3]);
                     globalExportProgress.fps = parseFloat(match[4]);
                     globalExportProgress.eta = parseFloat(match[5]);
                  }
                });
                
                child.on('close', code => {
                  const endTime = performance.now();
                  const elapsedSecs = ((endTime - startTime) / 1000).toFixed(2);
                  console.log(`[Backend-Export] Finished with code ${code} in ${elapsedSecs}s`);
                  if (code === 0) {
                      globalExportProgress.status = 'completed';
                      globalExportProgress.percent = 100;
                      globalExportProgress.elapsed = elapsedSecs;
                  } else {
                      globalExportProgress.status = 'error';
                      globalExportProgress.error = `Failed with exit code ${code} after ${elapsedSecs}s`;
                  }
                });

                res.writeHead(200, { 'Content-Type': 'application/json' });
                res.end(JSON.stringify({ status: 'started' }));

              } catch (e) {
                console.error("Export API Error:", e);
                globalExportProgress.status = 'error';
                globalExportProgress.error = e.toString();
                res.writeHead(500);
                res.end(e.toString());
              }
            });
          } else if (req.url === '/api/export/progress' && req.method === 'GET') {
            res.writeHead(200, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify(globalExportProgress));
          } else {
            res.writeHead(405);
            res.end('Method Not Allowed');
          }
        });
      }
    }
  ],
  server: {
    fs: {
      strict: false
    }
  }
});
