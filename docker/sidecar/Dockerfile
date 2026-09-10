FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive \
    TZ=Asia/Shanghai

# ── 依赖：ffmpeg + mediamtx 运行时 + 轻量 HTTP 控制服务 ───────────────────────
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    ffmpeg \
    python3 \
    && rm -rf /var/lib/apt/lists/*

# ── mediamtx (WebRTC / RTSP / HLS media server) ───────────────────────────────
# https://github.com/bluenviron/mediamtx/releases
ARG MEDIAMTX_VERSION=v1.9.1
RUN ARCH=$(uname -m) \
    && case "$ARCH" in \
        x86_64)  GOARCH=amd64 ;; \
        aarch64) GOARCH=arm64 ;; \
        *) echo "unsupported arch $ARCH" && exit 1 ;; \
    esac \
    && curl -fsSL \
       "https://github.com/bluenviron/mediamtx/releases/download/${MEDIAMTX_VERSION}/mediamtx_${MEDIAMTX_VERSION}_linux_${GOARCH}.tar.gz" \
       -o /tmp/mediamtx.tar.gz \
    && tar -xzf /tmp/mediamtx.tar.gz -C /usr/local/bin mediamtx \
    && chmod +x /usr/local/bin/mediamtx \
    && rm /tmp/mediamtx.tar.gz

# ── 配置文件 & 控制脚本 ────────────────────────────────────────────────────────
COPY mediamtx.yml        /etc/mediamtx/mediamtx.yml
COPY stream-ctrl.py      /usr/local/bin/stream-ctrl
COPY entrypoint.sh       /usr/local/bin/entrypoint.sh

RUN chmod +x /usr/local/bin/stream-ctrl /usr/local/bin/entrypoint.sh

# mediamtx WebRTC / RTSP / API 端口
# 8554 RTSP  8889 WebRTC-HTTP  8890 WebRTC-UDP  9997 API
EXPOSE 8554 8889 8890/udp 9997

# 流控制 HTTP 服务端口
EXPOSE 9998

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
