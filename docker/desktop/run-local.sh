#!/bin/bash
# run-local.sh — 本地启动 desktop 容器并开放 VNC 访问
# 用法: ./run-local.sh [容器名] [VNC端口]
#
# 默认:
#   容器名  = desktop-local
#   VNC端口 = 5900 (映射到容器内 5900)
#   VNC密码 = vibeyeah
#
# docker exec -u agent -it desktop-local bash

set -e

CONTAINER="${1:-desktop-local}"
VNC_PORT="${2:-5900}"
IMAGE="desktop:20260613"
VNC_PASS="${VNC_PASS:-vibeyeah}"
RESOLUTION="${RESOLUTION:-1920x1080}"

# 清理同名旧容器
if docker inspect "$CONTAINER" &>/dev/null; then
    echo "[*] 停止并删除旧容器 $CONTAINER ..."
    docker rm -f "$CONTAINER"
fi

echo "[*] 启动容器 $CONTAINER (image=$IMAGE) ..."
docker run -d \
    --name "$CONTAINER" \
    --shm-size=512m \
    -p "${VNC_PORT}:5900" \
    -e RESOLUTION="$RESOLUTION" \
    -e DISPLAY=":1" \
    "$IMAGE"

# 等待 Xvfb 就绪
echo "[*] 等待 Xvfb 启动..."
for i in $(seq 1 30); do
    if docker exec "$CONTAINER" test -S /tmp/.X11-unix/X1 2>/dev/null; then
        echo "[*] Xvfb 已就绪 (${i}s)"
        break
    fi
    sleep 1
done

# 在容器内安装 x11vnc 并启动
echo "[*] 安装 x11vnc ..."
docker exec -u root "$CONTAINER" bash -c "apt-get update -qq && apt-get install -y -q --no-install-recommends x11vnc"

echo "[*] 生成 VNC 密码..."
docker exec -u agent "$CONTAINER" bash -c "mkdir -p /home/agent/.vnc && x11vnc -storepasswd '${VNC_PASS}' /home/agent/.vnc/passwd"

echo "[*] 启动 x11vnc ..."
docker exec -d -u agent "$CONTAINER" bash -c "
    x11vnc \
        -display :1 \
        -rfbport 5900 \
        -rfbauth /home/agent/.vnc/passwd \
        -forever \
        -shared \
        -noxdamage \
        -noxfixes \
        -quiet \
        >> /tmp/x11vnc.log 2>&1
"

sleep 1

# 验证 VNC 是否在监听（x11vnc 输出 PORT=5900 即表示成功）
sleep 2
if docker exec "$CONTAINER" bash -c "cat /proc/net/tcp6 /proc/net/tcp 2>/dev/null | awk '{print \$2}' | grep -qi '170C\|1774'"; then
    VNC_OK=1
else
    # x11vnc PORT=5900 = 0x1774, 也接受日志里有 PORT= 字样
    if docker exec "$CONTAINER" bash -c "grep -q 'PORT=' /tmp/x11vnc.log 2>/dev/null"; then
        VNC_OK=1
    else
        VNC_OK=0
    fi
fi

if [ "$VNC_OK" = "1" ]; then
    echo ""
    echo "============================================"
    echo "  VNC 已就绪"
    echo "  地址: localhost:${VNC_PORT}"
    echo "  密码: ${VNC_PASS}"
    echo "============================================"
    echo ""
    echo "  macOS 连接方式:"
    echo "  open vnc://localhost:${VNC_PORT}"
    echo ""
    echo "  或使用 TigerVNC / RealVNC / Finder:"
    echo "  Finder → 前往 → 连接服务器 → vnc://localhost:${VNC_PORT}"
    echo ""
else
    echo "[!] VNC 启动可能有问题，查看日志:"
    docker exec "$CONTAINER" cat /tmp/x11vnc.log 2>/dev/null || true
fi

echo "  容器日志: docker logs -f $CONTAINER"
echo "  停止容器: docker rm -f $CONTAINER"

echo "  启动XTerm: docker exec -u agent desktop-local bash -c "'"'"DISPLAY=:1 xterm -bg '#1e1e2e' -fg '#cdd6f4' &"'"'
