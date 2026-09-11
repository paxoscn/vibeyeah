# ~/.bashrc — hermes agent shell init
# 由 user_home::prepare_user_home 从模板拷贝到每个用户的 home 目录

# 如果不在交互式 shell，则立即返回
[ -z "$PS1" ] && return

# ── 历史记录 ──────────────────────────────────────────────────────────────────
shopt -s histappend
HISTSIZE=10000
HISTFILESIZE=20000
HISTCONTROL=ignoreboth:erasedups

# ── PATH ──────────────────────────────────────────────────────────────────────
export RUSTUP_HOME=/opt/rust/rustup
export CARGO_HOME=/opt/rust/cargo
export PATH="$CARGO_HOME/bin:$PATH"
 
. "/opt/rust/cargo/env"

# ── NVM ───────────────────────────────────────────────────────────────────────
export NVM_DIR="$HOME/.nvm"
[ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"  # 加载 nvm
[ -s "$NVM_DIR/bash_completion" ] && \. "$NVM_DIR/bash_completion"  # 加载 nvm bash_completion

# ── 别名 ──────────────────────────────────────────────────────────────────────
alias ll='ls -alF'
alias la='ls -A'
alias l='ls -CF'
alias ..='cd ..'
alias ...='cd ../..'
