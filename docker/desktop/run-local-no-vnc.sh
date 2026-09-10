#!/bin/bash
# run-local.sh — 本地启动 desktop 容器并开放 VNC 访问
# 用法: ./run-local-no-vnc.sh [容器名] [VNC端口]
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
IMAGE="desktop:20260831"
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

echo "  容器日志: docker logs -f $CONTAINER"
echo "  停止容器: docker rm -f $CONTAINER"
