#!/bin/bash
set -e

RESOLUTION="${RESOLUTION:-1920x1080}"
DISPLAY=":1"

cleanup() {
    kill $(jobs -p) 2>/dev/null || true
    # 经 runuser 派生的 gateway 不一定在 jobs 里，兜底清理
    pkill -f 'hermes gateway' 2>/dev/null || true
    exit 0
}
trap cleanup SIGTERM SIGINT

# 启动虚拟帧缓冲，sidecar 的 ffmpeg 直接通过 x11grab 读取
echo "[desktop] starting Xvfb on ${DISPLAY} at ${RESOLUTION}..."
Xvfb ${DISPLAY} -screen 0 ${RESOLUTION}x24 \
    -ac +extension GLX +extension RANDR +render -noreset \
    -fp /usr/share/fonts/X11/misc,/usr/share/fonts/X11/75dpi,/usr/share/fonts/X11/100dpi &
sleep 1

export DISPLAY=${DISPLAY}

# 注册额外字体路径（xterm 等需要）
xset +fp /usr/share/fonts/X11/misc /usr/share/fonts/X11/75dpi /usr/share/fonts/X11/100dpi 2>/dev/null || true
xset fp rehash 2>/dev/null || true

# 窗口管理器
openbox-session &

# ── 多用户：按用户目录创建 Linux 用户，并为每个用户启动独立 hermes gateway ──
# 用户 home 位于 NAS：/data/nas/vibeyeah/agents/<name>/users/<user_id>/home，
# 其 .hermes（网关凭据/技能/记忆）由后端在 /add 时预置。
# 这里按目录存在性决定是否创建同名 Linux 用户，链接共享配置，并以该用户身份起 gateway。
#
# users 根目录：优先由 AGENT_CONFIG_DIR 推导（…/agents/<name>/configs -> …/agents/<name>/users），
# 回退到 AGENT_NAME。
USERS_ROOT=""
if [ -n "${AGENT_CONFIG_DIR:-}" ]; then
    USERS_ROOT="$(dirname "${AGENT_CONFIG_DIR}")/users"
elif [ -n "${AGENT_NAME:-}" ]; then
    USERS_ROOT="/data/nas/vibeyeah/agents/${AGENT_NAME}/users"
fi

# gateway 以系统级安装的 hermes 运行（root FHS 安装，见 Dockerfile）；
# 可用环境变量覆盖（测试用）
HERMES_BIN="${HERMES_BIN:-/usr/local/bin/hermes}"

start_user_gateway() {
    user_id="$1"
    user_home="$2"
    hermes_home="$3"

    # Linux 用户名须以字母开头且不超过 32 位，UUID（36 位、数字开头）不满足；
    # 由此派生一个合法且稳定的用户名：前缀 u + 小写、去连字符后的前若干位。
    linux_user="$(printf 'u%s' "${user_id}" | tr 'A-Z' 'a-z' | tr -cd 'a-z0-9' | cut -c1-32)"

    # 容器以非 root（uid 1000）运行；建用户/改属主/切换用户需 root，经 sudo 提权
    # （agent 用户已配置 NOPASSWD sudo）。
    # 1) 按需创建 Linux 用户（home 直接指向 NAS 用户 home，已存在，不用 -m）
    if ! id -u "${linux_user}" >/dev/null 2>&1; then
        if ! sudo -n useradd -d "${user_home}" -s /bin/bash "${linux_user}"; then
            echo "[desktop] WARNING: 创建用户 ${linux_user}（user_id=${user_id}）失败，跳过"
            return 1
        fi
    fi

    # 2) 准备目录与链接
    # 共享的 agent 级 .claude 配置（来自 AGENT_CONFIG_DIR，如存在）；经 sudo 写入以避开属主问题
    if [ -n "${AGENT_CONFIG_DIR:-}" ]; then
        if [ -e "${AGENT_CONFIG_DIR}/.claude.json" ]; then
            sudo -n ln -sfn "${AGENT_CONFIG_DIR}/.claude.json" "${user_home}/.claude.json" || true
        fi
        if [ -e "${AGENT_CONFIG_DIR}/.claude/settings.json" ]; then
            sudo -n mkdir -p "${user_home}/.claude" || true
            sudo -n ln -sfn "${AGENT_CONFIG_DIR}/.claude/settings.json" "${user_home}/.claude/settings.json" || true
        fi
    fi
    if ! sudo -n chown -R "${linux_user}:${linux_user}" "${user_home}"; then
        echo "[desktop] WARNING: chown ${user_home} 失败，跳过用户 ${user_id}"
        return 1
    fi

    # 3) 以该用户身份启动独立 gateway（日志按用户分开）。
    #    用 setsid 让每个 gateway 运行在自己的会话/进程组中：否则 gateway 重启时
    #    向其进程组发出的清理信号会波及作为 PID 1 的 entrypoint，导致整个 pod 被重启。
    echo "[desktop] starting hermes gateway for user ${user_id} (user=${linux_user}, HERMES_HOME=${hermes_home})..."
    setsid sudo -n -u "${linux_user}" -- env \
        HOME="${user_home}" \
        HERMES_HOME="${hermes_home}" \
        DISPLAY="${DISPLAY}" \
        PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin" \
        "${HERMES_BIN}" gateway >> "/tmp/hermes-gateway-${user_id}.log" 2>&1 &
    return 0
}

started=0
if [ ! -x "${HERMES_BIN}" ]; then
    echo "[desktop] WARNING: 未找到 ${HERMES_BIN}，未启动任何 gateway"
elif [ -z "${USERS_ROOT}" ] || [ ! -d "${USERS_ROOT}" ]; then
    echo "[desktop] WARNING: 未找到用户目录（${USERS_ROOT:-unset}），未启动任何 gateway"
else
    for user_dir in "${USERS_ROOT}"/*/; do
        [ -d "${user_dir}" ] || continue
        user_id="$(basename "${user_dir%/}")"
        user_home="${user_dir%/}/home"
        hermes_home="${user_home}/.hermes"

        # 新用户的 .hermes 由 /add 预置；缺失则跳过
        if [ ! -d "${hermes_home}" ]; then
            echo "[desktop] WARNING: 用户 ${user_id} 缺少 ${hermes_home}，跳过"
            continue
        fi

        if start_user_gateway "${user_id}" "${user_home}" "${hermes_home}"; then
            started=$((started + 1))
        fi
    done
    echo "[desktop] started ${started} hermes gateway(s)"
fi

. "/opt/rust/cargo/env"

echo "[desktop] ready. DISPLAY=${DISPLAY}"
# PID 1 不等待任何子进程：任何进程（Xvfb/openbox/gateway）的退出都不应导致
# entrypoint 结束、进而触发 pod 重启。用 sleep+wait 循环常驻；
# wait 可被 SIGTERM/SIGINT 打断，从而触发上方 cleanup 陷阱优雅退出。
while true; do
    sleep 3600 &
    wait $! || true
done
