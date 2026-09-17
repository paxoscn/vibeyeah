#!/usr/bin/env bash
#
# 打包后端发行包：构建 `vibeyeah` 二进制，并把 `docker/desktop/configs` 等一起打进 tar.gz。
#
# 用法：
#   scripts/package.sh [-o <输出目录>] [--no-rev] [--target <triple>] [--keep-staging]
#
# 产物：
#   <输出目录>/vibeyeah-<版本>-<平台>[-<短rev>].tar.gz
#   <输出目录>/vibeyeah-<版本>-<平台>[-<短rev>].tar.gz.sha256
#
# 包内布局（解压后该目录即运行目录：二进制按 cwd 找 `docker/desktop/configs`
# 播种配置模板，并在 cwd 下创建 `data/nas` 作为企业 NAS 根目录）：
#   vibeyeah-<版本>-<平台>[-<短rev>]/
#     vibeyeah                  后端二进制
#     docker/desktop/configs/   共享配置模板（企业创建时拷到 <nas>/vibeyeah/configs）
#     .env.example              DATABASE_URL / BIND_ADDR 示例
#     deploy/test/              Kubernetes 清单示例
#     CHANGELOG.md              变更记录
#     README.md                 快速开始（cwd 要求、首次运行向导、回调路径）
#
# 依赖：bash、cargo（stable）、tar、git（可选，用于版本后缀）。
# 交叉编译本机之外的平台时用 `--target <triple>`（需已 `rustup target add` 对应目标）。

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
BIN_NAME=vibeyeah
PKG_BASENAME=vibeyeah

OUT_DIR=$REPO_ROOT/dist
INCLUDE_REV=1
TARGET_TRIPLE=""
KEEP_STAGING=0

# 帮助信息直接取自本文件头部注释，避免两处维护
usage() {
    sed -n '/^# 用法：/,/^# 依赖：/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

while [ $# -gt 0 ]; do
    case "$1" in
        -o | --output-dir)
            [ $# -ge 2 ] || { echo "缺少 --output-dir 的值" >&2; exit 1; }
            OUT_DIR=$2
            shift 2
            ;;
        --no-rev)
            INCLUDE_REV=0
            shift
            ;;
        --target)
            [ $# -ge 2 ] || { echo "缺少 --target 的值" >&2; exit 1; }
            TARGET_TRIPLE=$2
            shift 2
            ;;
        --keep-staging)
            KEEP_STAGING=1
            shift
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            echo "未知参数：$1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

# ── 版本号与平台标签 ───────────────────────────────────────────────────────────
VERSION=$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' \
    "$REPO_ROOT/backend/Cargo.toml" | head -1)
[ -n "$VERSION" ] || { echo "无法从 backend/Cargo.toml 读取版本号" >&2; exit 1; }

if [ -n "$TARGET_TRIPLE" ]; then
    PLATFORM=$TARGET_TRIPLE
    RELEASE_DIR=$REPO_ROOT/backend/target/$TARGET_TRIPLE/release
else
    PLATFORM="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
    RELEASE_DIR=$REPO_ROOT/backend/target/release
fi

REV=""
if [ "$INCLUDE_REV" = 1 ] && command -v git >/dev/null 2>&1 &&
    git -C "$REPO_ROOT" rev-parse --short HEAD >/dev/null 2>&1; then
    REV=$(git -C "$REPO_ROOT" rev-parse --short HEAD)
fi

DIR_NAME=$PKG_BASENAME-$VERSION-$PLATFORM
[ -n "$REV" ] && DIR_NAME=$DIR_NAME-$REV
ARCHIVE=$OUT_DIR/$DIR_NAME.tar.gz

# ─ 构建 ──────────────────────────────────────────────────────────────────────
if ! command -v cargo >/dev/null 2>&1; then
    echo "未找到 cargo，请先安装 Rust 工具链（https://rustup.rs）" >&2
    exit 1
fi
command -v protoc >/dev/null 2>&1 ||
    echo "[package] 提示：未检测到 protoc；若构建报错请先安装（CI 使用 arduino/setup-protoc）" >&2

echo "[package] 构建后端：cargo build --release --locked ${TARGET_TRIPLE:+(--target $TARGET_TRIPLE)}"
CARGO_ARGS=(build --release --locked --manifest-path "$REPO_ROOT/backend/Cargo.toml")
[ -n "$TARGET_TRIPLE" ] && CARGO_ARGS+=(--target "$TARGET_TRIPLE")
cargo "${CARGO_ARGS[@]}"

BIN_PATH=$RELEASE_DIR/$BIN_NAME
[ -f "$BIN_PATH" ] || { echo "构建产物不存在：$BIN_PATH" >&2; exit 1; }

# ── 暂存目录（包内布局在这里组装） ─────────────────────────────────────────────
CONFIGS_SRC=$REPO_ROOT/docker/desktop/configs
[ -d "$CONFIGS_SRC" ] || { echo "缺少配置模板目录：$CONFIGS_SRC" >&2; exit 1; }

STAGE=$(mktemp -d "${TMPDIR:-/tmp}/vibeyeah-package.XXXXXX")
cleanup() {
    if [ "$KEEP_STAGING" = 1 ]; then
        echo "[package] 保留暂存目录：$STAGE"
    else
        rm -rf "$STAGE"
    fi
}
trap cleanup EXIT

DEST=$STAGE/$DIR_NAME
mkdir -p "$DEST/docker/desktop"

install -m 0755 "$BIN_PATH" "$DEST/$BIN_NAME"
# 构建产物保留调试符号，压缩前就地 strip（只影响包内副本，不动 target/）
if command -v strip >/dev/null 2>&1; then
    strip "$DEST/$BIN_NAME" 2>/dev/null ||
        echo "[package] 提示：strip 失败，包内二进制保留调试符号" >&2
fi

cp -a "$CONFIGS_SRC" "$DEST/docker/desktop/configs"
cp -a "$REPO_ROOT/backend/.env.example" "$DEST/.env.example"
cp -a "$REPO_ROOT/deploy" "$DEST/deploy"
cp -a "$REPO_ROOT/CHANGELOG.md" "$DEST/CHANGELOG.md"

# 包内说明：cwd 要求与首次运行流程
cat >"$DEST/README.md" <<'PKG_README'
# VibeYeah 后端发行包

包内已含后端二进制与共享配置模板 `docker/desktop/configs`。

## 运行

**必须在解压目录内启动**——二进制按当前工作目录（cwd）定位配置模板，
并在 cwd 下创建企业中转用的 NAS 根目录：

```bash
cd vibeyeah-<版本>-<平台>
chmod +x ./vibeyeah
./vibeyeah                       # 默认：SQLite ./vibeyeah.db + 监听 0.0.0.0:8080
```

可选的运行环境变量（其余配置都在数据库 `settings` 表里）：

```bash
DATABASE_URL=postgres://user:pass@host:5432/vibeyeah BIND_ADDR=0.0.0.0:8080 ./vibeyeah
```

## 首次运行

数据库为空时会在终端启动交互式向导，创建首个企业（organization）并可选地绑定飞书 Bot 应用。
**创建企业时**后端会：

1. 把 `organizations.nas_mount_root` 置为 `<cwd>/data/nas`；
2. 确保该目录存在；
3. 若 `<cwd>/data/nas/vibeyeah/configs` 不存在，则把本包内 `docker/desktop/configs`
   完整拷过去（等价 `cp -a docker/desktop/configs/. <nas>/vibeyeah/configs/`）。

非交互式启动不会进入向导，可后续登录控制台创建企业。

## 外部回调

```
GET|POST /callback/{org_id}/{skill}/{user_id}?foo=bar
```

需带 `X-Callback-Token`（或 `?token=`）当 `settings.callback_token` 非空时。
`org_id` 即企业的 UUID（企业列表 / API 返回的 id）。

## 其他

- `deploy/test/`：Kubernetes 清单示例（Deployment / Service / init.sql）。
- `CHANGELOG.md`：本版本变更记录。
- 完整文档见 <https://github.com/paxoscn/vibeyeah>。
PKG_README

# ── 打包 ──────────────────────────────────────────────────────────────────────
mkdir -p "$OUT_DIR"
rm -f "$ARCHIVE" "$ARCHIVE.sha256"
tar -czf "$ARCHIVE" -C "$STAGE" "$DIR_NAME"

if command -v sha256sum >/dev/null 2>&1; then
    (cd "$OUT_DIR" && sha256sum "$DIR_NAME.tar.gz" >"$DIR_NAME.tar.gz.sha256")
elif command -v shasum >/dev/null 2>&1; then
    (cd "$OUT_DIR" && shasum -a 256 "$DIR_NAME.tar.gz" >"$DIR_NAME.tar.gz.sha256")
else
    echo "[package] 提示：未找到 sha256sum / shasum，跳过校验和生成" >&2
fi

echo "[package] 完成：$ARCHIVE ($(du -h "$ARCHIVE" | cut -f1))"
[ -f "$ARCHIVE.sha256" ] && echo "[package] 校验和：$ARCHIVE.sha256"
echo "[package] 包内条目 $(tar -tzf "$ARCHIVE" | wc -l) 个，顶层内容："
tar -tzf "$ARCHIVE" |
    sed -n "s|^$DIR_NAME/||p" |
    cut -d/ -f1 |
    grep -v '^$' |
    sort -u |
    sed 's/^/  /'