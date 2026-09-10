#!/bin/bash
set -e

cleanup() {
    echo "[sidecar] shutting down..."
    kill $(jobs -p) 2>/dev/null || true
    exit 0
}
trap cleanup SIGTERM SIGINT

# ── 等待 desktop 容器的 VNC 就绪（共享 X socket，最多等 60s） ─────────────────
echo "[sidecar] waiting for Xvfb display :1..."
for i in $(seq 1 60); do
    if [ -S /tmp/.X11-unix/X1 ]; then
        echo "[sidecar] X socket found after ${i}s"
        break
    fi
    sleep 1
done

# ── 启动 mediamtx（常驻） ─────────────────────────────────────────────────────
echo "[sidecar] starting mediamtx..."
mediamtx /etc/mediamtx/mediamtx.yml &
MEDIAMTX_PID=$!
sleep 2

# ── 启动流控制 HTTP 服务（常驻，ffmpeg 推流默认不启动） ───────────────────────
echo "[sidecar] starting stream-ctrl on :9998..."
python3 /usr/local/bin/stream-ctrl &
CTRL_PID=$!

echo "[sidecar] ready. mediamtx WebRTC=:8889  stream-ctrl=:9998"
wait $MEDIAMTX_PID $CTRL_PID
