#!/usr/bin/env python3
"""
轻量级 HTTP 控制服务，管理 ffmpeg 推流进程。

端口: 9998
接口:
  POST /start   启动 ffmpeg，从 VNC(localhost:5901) 采集画面推到 mediamtx
  POST /stop    停止 ffmpeg
  GET  /status  返回当前推流状态 JSON
"""

import http.server
import json
import os
import signal
import subprocess
import threading

VNC_HOST = os.environ.get("VNC_HOST", "localhost")
VNC_PORT = int(os.environ.get("VNC_PORT", "5901"))
MEDIAMTX_RTSP = os.environ.get("MEDIAMTX_RTSP", "rtsp://localhost:8554/desktop")
CTRL_PORT = int(os.environ.get("CTRL_PORT", "9998"))
RESOLUTION = os.environ.get("RESOLUTION", "1920x1080")

_ffmpeg_proc = None
_lock = threading.Lock()


def ffmpeg_cmd():
    return [
        "ffmpeg",
        "-loglevel", "warning",
        # 从 x11grab（VNC 虚拟帧缓冲通过 Xvfb 暴露）读取
        # sidecar 与 desktop 共享 /tmp/.X11-unix，DISPLAY=:1
        "-f", "x11grab",
        "-r", "15",
        "-s", RESOLUTION,
        "-i", ":1.0",
        # 无音频
        "-an",
        # 视频编码
        "-c:v", "libx264",
        "-preset", "ultrafast",
        "-tune", "zerolatency",
        "-pix_fmt", "yuv420p",
        "-g", "30",
        # 推到 mediamtx RTSP
        "-f", "rtsp",
        "-rtsp_transport", "tcp",
        MEDIAMTX_RTSP,
    ]


def start_stream():
    global _ffmpeg_proc
    with _lock:
        if _ffmpeg_proc is not None and _ffmpeg_proc.poll() is None:
            return {"ok": False, "reason": "already_running"}
        _ffmpeg_proc = subprocess.Popen(ffmpeg_cmd())
    return {"ok": True, "pid": _ffmpeg_proc.pid}


def stop_stream():
    global _ffmpeg_proc
    with _lock:
        if _ffmpeg_proc is None or _ffmpeg_proc.poll() is not None:
            return {"ok": False, "reason": "not_running"}
        _ffmpeg_proc.send_signal(signal.SIGTERM)
        try:
            _ffmpeg_proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            _ffmpeg_proc.kill()
        _ffmpeg_proc = None
    return {"ok": True}


def get_status():
    with _lock:
        running = _ffmpeg_proc is not None and _ffmpeg_proc.poll() is None
        pid = _ffmpeg_proc.pid if running else None
    return {"streaming": running, "pid": pid}


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass  # 静默 access log

    def _respond(self, data, code=200):
        body = json.dumps(data).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        if self.path == "/start":
            self._respond(start_stream())
        elif self.path == "/stop":
            self._respond(stop_stream())
        else:
            self._respond({"error": "not_found"}, 404)

    def do_GET(self):
        if self.path == "/status":
            self._respond(get_status())
        else:
            self._respond({"error": "not_found"}, 404)


if __name__ == "__main__":
    server = http.server.HTTPServer(("0.0.0.0", CTRL_PORT), Handler)
    print(f"[stream-ctrl] listening on :{CTRL_PORT}", flush=True)
    server.serve_forever()
