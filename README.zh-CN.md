# VibeYeah

> **一个 Agent 沙箱平台——属于你自己的、永远在线的 AI agent 队伍。**
> VibeYeah 在 Kubernetes 上为每个用户提供一个隔离的 Ubuntu 桌面**沙箱**，即私有、**常驻在线**
> 的 AI **agent**；你通过**飞书 / 微信**驱动它干活。沙箱运行在**企业内网**内，agent 因此能
> 安全访问内部系统，内部系统也能反过来把任务交给它。一次部署服务一个团队。

[English](README.md) | **简体中文**

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](./LICENSE)
[![Backend CI](https://github.com/paxoscn/vibeyeah/actions/workflows/backend.yml/badge.svg)](https://github.com/paxoscn/vibeyeah/actions/workflows/backend.yml)

---

## VibeYeah 的本质

市面上大多数「AI 助手」本质上是聊天机器人：你问、它答，然后**你**自己去执行。VibeYeah
相反——它是一个** Agent 沙箱平台**。每个 agent 都是一个真实、永远在线的执行者：
打开浏览器、登录你的内部系统、填写表单、运行代码、读写文件、执行定时任务，并把结果与截图
直接回报到你的飞书 / 微信对话。每个 agent 都是隔离的沙箱，运行
[**hermes-agent**](https://github.com/NousResearch/hermes-agent)，由 Rust
（`axum`/`sea-orm`/`kube`）控制平面统一编排。

### 六大核心定位

1. **Agent 沙箱平台。** 每个 agent 都是一台隔离、专属的 Ubuntu 桌面沙箱（`Deployment`），
   拥有独立的 Linux 账户、hermes gateway 与持久化状态；由控制平面统一创建、监管与回收。
2. **私有、常驻在线的 agent，通过飞书 / 微信驱动。** agent 为你长期在线且私有：在飞书 / Lark
   或微信上即可驱动它（发送 `/add`，数秒内开通你的 agent），并可经 **WebRTC** 实时观看它操作。
3. **部署于企业内网——安全访问内部系统，也接受内部系统触发。** 沙箱运行在你的内网 / 私有
   VPC 内，agent 因此能**安全访问内部系统**（以你的凭据操作浏览器 / 终端 / 文件）；内部系统
   也能**直接把任务交给 agent**——通过 webhook、`/callback/{org_id}/{skill}/{user_id}`、定时任务或 CI。
4. **Skill / 知识库 / 记忆 / 环境上下文——集中管理、迭代与分发。** agent 的技能、知识库、
   记忆与环境上下文作为共享目录在 NAS 上统一管理，并**分发**到正确的 agent 与用户；agent
   状态支持 **git 同步**，使能力与行为得以**版本化迭代、可审计**。
5. **统一 Harness 基座，对接企业内部 CI/CD，实现 Agent 主导的迭代。** 一个统一的 Harness
   承载所有 agent；agent 产出的状态与制品可喂给企业 CI/CD，让整个 agent 体系自身驱动产品与
   服务的迭代。
6. **基于飞书实现 A2A——agent 组队协作。** 不同角色的 agent 可被组合成团队，在飞书上实现
   agent 与 agent（A2A）的相互协调，让多个各有所长的 agent 协作完成一项复杂任务。

## 开源 vs 闭源 Agent Runtime：企业内数据安全上的优势

闭源 Agent Runtime 通常由**厂商托管**：要用它，就得把数据和凭据交给别人的服务，并且只能信任
一个你无法审查的二进制。VibeYeah 的选择相反——整个运行时（控制平面**与** agent 沙箱）都是
**Apache-2.0 且自托管在企业内网**，因此它的安全属性是你能**验证并强制**的，而不是只能选择相信的。

| — | 闭源 / 厂商托管 Runtime | VibeYeah |
| --- | --- | --- |
| 对话、文件、记忆、技能存在哪 | 通常存在厂商基础设施 | **企业自有 NAS**（RWX PVC）与自有集群 |
| 能否看见有哪些数据出网 | 不能，只有二进制 | **能**——源码可读，可 grep 每一处出网调用 |
| 模型与 API 密钥归属 | 往往由厂商持有 | **属于企业**——存于自己的数据库，只发往自己配置的端点 |
| 部署位置 | 厂商云 / SaaS | 企业内网、私有 VPC，**甚至完全离线** |
| 可持续性与退出 | 订阅、功能开关、厂商路线图 | Apache-2.0：**不失效、不激活，必要时可 fork** |
| 漏洞修复 | 等厂商发版 | 自己打补丁，当天上线 |

具体到 VibeYeah：

- **数据不动窝。** agent 的对话、技能、记忆、浏览器状态与文件都在你挂载的共享 NAS
  （`/data/nas`）和自有的 PostgreSQL 里，不在厂商云上。控制平面**不带任何遥测、统计上报或激活
  回调**；出网端点只有你自己配置的那几个：飞书（`open.feishu.cn` / `accounts.feishu.cn`）、
  微信，以及你的大模型端点。
- **天生可审计。** 每一处出网调用都是可读、可 grep 的 Rust 代码——「不把数据发出去」不是一句
  承诺，而是部署前就能核对的事实。
- **自带模型。** 大模型端点、密钥与模型名是**你自己数据库**里的企业级配置
  （`/set openai_base_url …`），提示词与业务数据从你的集群直达你选定的模型端点，没有第三方中转。
- **隔离深度由你决定。** 每个用户独立 Linux 账户与独立 `HERMES_HOME`；每个企业独立 NAS 根目录、
  K8s namespace 与 kubeconfig。你还可以再叠加自己的 `NetworkPolicy`、镜像基线与审计日志，并固定
  使用自己构建的镜像。
- **部署在规则允许的地方。** 内网、私有 VPC，或完全离线的环境——用 `scripts/package.sh` 打出
  自我包含的压缩包，投放到不接公网的机器上即可。
- **安全响应不被厂商排期卡住。** `SECURITY.md` 提供私密漏洞上报流程，Apache-2.0 允许你自行加固、
  打补丁并立即重新部署——包括那些不会有厂商替你排期的改动。

> 开源本身不等于安全：agent runtime 是高权限工作负载，仍须按[安全](#安全)章节加固。开源给你的是
> **验证**这些防护的能力、在你自己的边界上**强制**它们的能力，以及**无需请示**就能修好它们的能力。

## 架构

```
   飞书 / 微信                               HTTP 客户端
        │  (对话 / /add)                          │  (REST + JWT)
        ▼                                          ▼
┌───────────────────────────────────────────────────────────────────┐
│                       后端 Backend (Rust · axum)                     │
│  API · 鉴权(JWT / 飞书登录) · agent 管理 · Kubernetes 编排 ·          │
│  飞书/微信 bot · 回调路由 · pod 状态同步 · agent 状态 git 同步         │
└───────┬───────────────────────────────────────────┬────────────────┘
        │ 创建/更新                                   │ PostgreSQL / SQLite
        ▼                                             ▼
┌───────────────────────────────────────┐    ┌───────────────────┐
│  Agent Pod (K8s Deployment, 单副本)    │    │  数据库            │
│  Ubuntu 桌面: Xvfb + openbox +         │    │  (PostgreSQL 或    │
│  Chrome + Node/Python/Rust/Java        │    │   SQLite)         │
│  hermes gateway ×N (每用户一个,        │    └───────────────────┘
│  独立 HERMES_HOME, 独立 Linux 用户)    │
└───────┬───────────────────────────────┘
        │ 读写 (每 agent / 每用户数据)
        ▼
┌───────────────────────────────────────────────────────────────────┐
│  共享 NAS (ReadWriteMany PVC), 挂载于 /data/nas                      │
│  vibeyeah/configs/                         ← 共享种子模板             │
│  vibeyeah/agents/<agent>/configs/          ← agent 级配置             │
│  vibeyeah/agents/<agent>/users/<uid>/home/ ← 每用户 home + .hermes     │
└───────────────────────────────────────────────────────────────────┘
```

- **后端**（`backend/`，Rust）：控制平面，以「可复用核心库 + 精简 `vibeyeah` 可执行文件」形式
  构建。提供 REST API，运行飞书长连接 bot，通过 Kubernetes API 创建 / 销毁 agent
  `Deployment`，同步 pod 状态，在 `/add` 时准备每个用户的 NAS home，并把外部回调路由进 agent pod。
- **Agent pod**（`docker/desktop/`）：AI 工作站。一个无头 Ubuntu 桌面，装有 Chrome 与常用工具链，
  **为每个用户**运行一个 hermes gateway（各自独立的 Linux 用户与独立的 `HERMES_HOME`）。
- **NAS**：一个 `ReadWriteMany` PVC，持久化 agent 与每个用户的配置、技能、记忆与会话，跨 pod
  重启保留。
- **WebRTC sidecar**（`docker/sidecar/`）：可选的、基于 mediamtx 的桌面推流。

详见 [`docs/architecture.md`](docs/architecture.md) 与 [`docs/deployment.md`](docs/deployment.md)。

## 仓库结构

```
backend/    Rust 控制平面——核心库 + `vibeyeah` 可执行文件 (axum, sea-orm, kube)
docker/
  desktop/  Agent 工作站镜像 (Ubuntu + hermes + 工具链) 与 entrypoint
  sidecar/  可选的 WebRTC 推流 sidecar (mediamtx)
deploy/     Kubernetes 清单示例 (deploy/test/)
scripts/    `package.sh`——构建后端并与配置模板一起打包
docs/       架构与部署文档
```

## 先决条件

- 一个 **Kubernetes** 集群（后端使用 in-cluster 配置或 kubeconfig）。
- 一个 **ReadWriteMany** 存储卷（如 NFS/NAS）用于共享的 `vibeyeah` PVC，在后端与 agent pod 内
  均挂载到 `/data/nas`。
- 供后端使用的 **PostgreSQL**（[方式 A](#方式-a运行-release-二进制最快) 用内置 SQLite 首次运行则不需要）。
- 一个**飞书 / Lark** 应用（以及可选的**微信**）用于消息接入。
- 一个 **LLM** 服务，提供 OpenAI 兼容或 Anthropic 兼容端点（如阿里云 DashScope/Qwen、OpenAI）。
- 用于存放后端、desktop、sidecar 镜像的容器**镜像仓库**访问权限。

## 快速开始

### 方式 A：运行 Release 二进制（最快）

从 [Releases](https://github.com/paxoscn/vibeyeah/releases) 页面下载最新的发布压缩包（按你的
系统 / 架构选择，如 `vibeyeah-0.1.0-linux-x86_64.tar.gz`），解压后**在解压目录内**运行二进制
——它按当前工作目录定位随包携带的 `docker/desktop/configs` 模板并创建 NAS 根目录。**无需任何
配置即可启动**——首次启动后端会使用当前目录的 SQLite 文件 `./vibeyeah.db`（自动创建并迁移），
监听 `0.0.0.0:8080`：

```bash
tar -xzf vibeyeah-0.1.0-linux-x86_64.tar.gz
cd vibeyeah-0.1.0-linux-x86_64
./vibeyeah                    # 可选：DATABASE_URL=postgres://... BIND_ADDR=0.0.0.0:8080
```

**1 · 初始化部署——首次运行向导。** 因数据库为空，二进制会进入交互式向导：

```
[初始化] 数据库中还没有任何组织。
是否现在创建首个组织，并为它绑定飞书应用作为 Bot 入口？[Y/n]   ← 回车即 Yes
请输入组织显示名称 [default]: my-org
✅ 已创建首个组织：my-org（slug=my-org，id=…）。
[绑定] 正在为组织注册飞书应用（Bot 入口）。请用飞书扫描下方二维码…
```

回车 / 输入 `y`，输入显示名称（默认 `default`），随后**用飞书扫描终端打印的二维码**（或
打开链接）注册 / 选择作为 **Bot** 的飞书应用。应用凭据会自动写回，扫码账号会被记录为
**Bot 管理员**——后续的 `/set` 等管理指令用它执行。若暂不绑定可按回车跳过——之后可在飞书
开放平台创建应用，把 `App ID` / `App Secret` 写入 `organizations` 表并重启。

**2 · 在飞书上配置大模型。** 给 bot（刚绑定的应用）发消息，由 Bot 管理员用 `/set` 填好 agent
要使用的大模型——**任何成员 `/add` 之前都必须先配齐**：

```
/set openai_base_url https://your-llm-endpoint.example   # 不要带 /v1
/set openai_api_key sk-xxxx
/set openai_model your-model
```

每条都会返回 `✅ 已设置 …`。

**3 · 通过 `/add` 开通 agent。** 任何飞书用户都能给 bot 发送 **`/add`**：

- bot 会回一个二维码 / 链接，用来为该成员注册一个**专属飞书应用**（首次使用会自动建号）；
- 成功后后端会开通一个 agent 沙箱 pod，就绪后会通过飞书通知你；
- 之后直接与你的专属 agent bot 对话即可。

> ⚠️ 真正开通 agent pod 还需要 **Kubernetes** 集群（后端使用 `~/.kube/config` 或 in-cluster
> 配置）、共享 **NAS**（把 `docker/desktop/configs/` 播种到 `<nas>/vibeyeah/configs/`，挂载于
> `/data/nas`），以及集群可拉取的 **desktop 镜像**（镜像 / PVC / namespace 默认值在
> `settings` 表，见[配置](#配置)）。缺少这些基础设施时，上面的向导与飞书配置步骤依然可用，
> `/add` 会在「创建智能体（开通 pod）」阶段报错，属预期行为。

### 方式 B：从镜像 / 源码部署

1. **构建镜像**

   ```bash
   # Agent 工作站镜像
   docker build -t <your-registry>/vibeyeah-desktop:latest docker/desktop/
   # （可选）WebRTC sidecar
   docker build -t <your-registry>/vibeyeah-sidecar:latest docker/sidecar/
   # 后端
   cd backend && docker build -t <your-registry>/vibeyeah-backend:latest .
   ```

2. **准备 NAS 种子**——创建企业时后端已会自动把 `docker/desktop/configs/` 复制到
   `<nas>/vibeyeah/configs/`（企业的 NAS 根目录默认是 `<后端 cwd>/data/nas`），因此手工播种
   仅用于预置存储卷的场景。仓库内这些配置**只含占位符**（`__OPENAI_*__` / `__LARK_*__`）；
   真实的大模型 / 网关值会在 `/add` 时由后端渲染进副本（因此不要把密钥手工写进种子）。

3. **配置后端**——设置 `DATABASE_URL`（PostgreSQL；留空则用本地 SQLite），可选设置 `BIND_ADDR`。
   把 `backend/.env.example` 复制为 `backend/.env` 作模板。其余所有配置存于数据库 `settings`
   表，首次迁移即写入默认值，可改表调整。详见[配置](#配置)。

4. **部署**——应用 `deploy/test/` 中的清单（请先修改镜像引用、namespace、PVC 名称与密钥），或
   使用你自己的清单。随后带终端运行一次后端，让首次运行向导初始化平台并绑定飞书 Bot。

5. **开通 agent**——在飞书里给你的 bot 发送 **`/add`**，扫码授权一个飞书应用，VibeYeah 便会
   创建你的 agent pod。之后直接对话即可。

**或构建发布压缩包**——`scripts/package.sh` 会构建后端，并把二进制与 `docker/desktop/configs`、
`.env.example`、清单示例、CHANGELOG 一起打包，解压目录即自包含（二进制按工作目录找模板、创建
NAS 根目录）：

```bash
scripts/package.sh                 # -> dist/vibeyeah-<版本>-<平台>[-<rev>].tar.gz（含 .sha256）
scripts/package.sh --target x86_64-unknown-linux-musl   # 交叉编译（需先 rustup target add）
scripts/package.sh --help
```

本地开发见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。

## 配置

只有两项从环境变量读取——`DATABASE_URL` 与 `BIND_ADDR`。其余运行配置均作为 key/value 存于
数据库 `settings` 表，由迁移 `m20240019_create_settings` 建表并写入默认值（完整键名见
[`backend/.env.example`](backend/.env.example)）。改表后重启 backend 生效。`DATABASE_URL` 支持
PostgreSQL 或 SQLite（留空默认为 `sqlite://vibeyeah.db`）。

| `settings` 表键 | 默认值 | 说明 |
| --- | --- | --- |
| `jwt_secret` | `change_me_to_a_long_random_secret` | 签发 JWT 的密钥——请改为足够长的随机值。 |
| `jwt_expire_hours` | `72` | JWT 有效期（小时）。 |
| `lark_app_id` / `lark_app_secret` | *（空）* | 用于**网页登录**的全局飞书应用（可选；Bot 应用在首次运行时绑定）。 |
| `k8s_namespace` | `default` | agent 的默认 namespace。 |
| `pod_sync_interval_secs` | `30` | pod 状态同步间隔（秒）。 |
| `desktop_image` / `sidecar_image` | `vibeyeah/*:latest` | agent 桌面 / WebRTC sidecar 镜像。 |
| `webrtc_base_url` | `http://localhost:8889` | 桌面实时观看的基础 URL。 |
| `nas_pvc_name` | `vibeyeah-nas-pvc` | 挂载进 agent pod 的共享 NAS PVC 名称。 |
| `hermes_exec_timeout_secs` | `900` | 回调触发的 pod 内 hermes 运行超时（秒）。 |
| `callback_token` | *（空）* | 若设置，`/callback/...` 需携带 `X-Callback-Token`（或 `?token=`）。 |
| `sms_access_key_id` / `_secret` / `sign_name` / `template_code` | *（空）* | 短信服务商（预留，手机号登录用）。 |

agent 使用的大模型（`openai_base_url` / `openai_api_key` / `openai_model`）**不是**全局
`settings` 项——由 Bot 管理员通过飞书 `/set` 在运行时设置（默认空）。`openai_base_url`
**不要带 `/v1` 尾缀**。

NAS 根目录（`organizations.nas_mount_root`）同样是企业级配置：创建企业时写入
`<后端进程 cwd>/data/nas`（容器内 cwd 为 `/` 即 `/data/nas`，与部署清单一致），
同时确保该目录存在，并在缺少 `vibeyeah/configs` 时从仓库 `docker/desktop/configs` 播种。

## 外部回调路由

任意外部系统都可触发某个用户的技能：

```
GET|POST /callback/{org_id}/{skill}/{user_id}?foo=bar
```

后端先按 `org_id` 找到对应的企业（不存在返回 `404`），再在该企业的 NAS 根目录
（`organizations.nas_mount_root`）下的 `agents/<agent>/users/<user_id>/home/.hermes/skills/**/<skill>`
找到为 `user_id` 安装了 `skill` 的 agent，立即返回 `202`，随后在 agent pod 内（以该用户身份）
调用 hermes，把技能名、query 参数与请求体传给它——并按顺序尝试候选 agent，直到某个成功为止。
在任何可被访问的网络中，请用 `CALLBACK_TOKEN` 保护该端点。

## 安全

⚠️ **Agent 是高权限工作负载**（浏览器、终端、文件），且本系统面向**受信任的企业内网**部署。
部署前请阅读 [`SECURITY.md`](SECURITY.md)——它包含加固指引、私密漏洞上报流程，以及重要的
**责任免责声明**。

本软件按 Apache License 2.0「**按原样**（AS IS），不提供任何担保」提供。运维方须自行负责其
部署的安全。详见 [`SECURITY.md`](SECURITY.md#disclaimer--免责声明)。

## 路线图

- 技能市场（可复用的行业自动化模块）
- 多模型路由（成本最优调度）
- Agent 监控看板与任务审计日志

## 贡献

欢迎贡献！请阅读 [`CONTRIBUTING.md`](CONTRIBUTING.md) 了解环境搭建、约定与 Pull Request 流程。
本项目遵循 [Contributor Covenant 行为准则](CODE_OF_CONDUCT.md)。

## 许可证

采用 **Apache License 2.0** 许可——见 [`LICENSE`](LICENSE) 与 [`NOTICE`](NOTICE)。第三方组件
（包括 hermes-agent 与内置技能）按其各自的许可证授权；详见 [`NOTICE`](NOTICE)。
