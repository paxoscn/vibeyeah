# docker image build -t desktop:20260831 .
##FROM ubuntu:22.04
FROM swr.cn-north-4.myhuaweicloud.com/ddn-k8s/docker.io/ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive \
    TZ=Asia/Shanghai \
    LANG=en_US.UTF-8 \
    DISPLAY=:1 \
    RESOLUTION=1920x1080

# ── 基础依赖 ──────────────────────────────────────────────────────────────────
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl wget gnupg2 software-properties-common \
    locales tzdata sudo procps \
    # 虚拟帧缓冲（无需 VNC）
    xvfb \
    # 窗口管理器
    openbox dbus-x11 \
    # 终端
    xterm \
    # 字体（xterm 和 X 应用必需）
    xfonts-base xfonts-75dpi xfonts-100dpi \
    # 字体
    fonts-noto-cjk fonts-noto-color-emoji \
    git unzip jq \
    && locale-gen en_US.UTF-8 \
    && ln -sf /usr/share/zoneinfo/${TZ} /etc/localtime \
    && rm -rf /var/lib/apt/lists/*

# ── Google Chrome ─────────────────────────────────────────────────────────────
RUN curl -fsSL https://dl.google.com/linux/linux_signing_key.pub \
      | gpg --dearmor -o /usr/share/keyrings/google-chrome.gpg \
    && echo "deb [arch=amd64 signed-by=/usr/share/keyrings/google-chrome.gpg] \
       https://dl.google.com/linux/chrome/deb/ stable main" \
       > /etc/apt/sources.list.d/google-chrome.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends google-chrome-stable \
    && rm -rf /var/lib/apt/lists/*

# ── Node.js 22 ────────────────────────────────────────────────────────────────
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*
    
# ── 非 root 用户 ──────────────────────────────────────────────────────────────
RUN useradd -m -s /bin/bash -G sudo agent \
    && echo "agent ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/agent
RUN mkdir -p /home/agent
RUN chown agent:agent /home/agent

# ── AI 工具 ───────────────────────────────────────────────────────────────────
RUN npm install -g @anthropic-ai/claude-code
# 以 root 安装 hermes → FHS 布局（/usr/local/lib/hermes-agent + /usr/local/bin/hermes），
# 一次系统级安装即可供 Pod 内所有用户使用；每个用户的数据在各自 HERMES_HOME
RUN curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash -s -- --commit c4066091cacba42d5afdcceb0b5a87e515019e86

ENV RUSTUP_HOME=/opt/rust/rustup
ENV CARGO_HOME=/opt/rust/cargo
ENV PATH="$CARGO_HOME/bin:$PATH"
RUN curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- -y --no-modify-path --default-toolchain stable

USER agent
WORKDIR /home/agent

RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.3/install.sh | bash
RUN sudo apt install -y openjdk-21-jdk
RUN sudo apt install -y openjdk-8-jdk
# sudo update-alternatives --config java
# sudo update-alternatives --config javac
# sudo update-alternatives --config javadoc

ENV PATH="/home/agent/.local/bin:${PATH}"
RUN curl -LsSf https://astral.sh/uv/install.sh | sh
RUN /bin/bash -c "uv venv playwright_env && source playwright_env/bin/activate && uv pip install playwright"

RUN sudo /usr/local/lib/hermes-agent/venv/bin/python -m ensurepip --upgrade
RUN sudo /usr/local/lib/hermes-agent/venv/bin/python -m pip install lark-oapi --progress-bar off

# For GIT
RUN sudo apt install -y openssh-client

# entrypoint 以 root 运行：需为每个用户创建 Linux 账户并 chown 其 NAS home
USER root
COPY --chown=agent:agent entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
