# VibeYeah

> **A Agent sandbox platform — your own fleet of always-on AI agents.**
> VibeYeah provisions private, **permanently-online AI agents** — each an isolated
> Ubuntu-desktop **sandbox** on Kubernetes — that you drive over **Feishu/Lark or
> WeChat**. Sandboxes run **inside your intranet**, so agents can safely reach internal
> systems, and internal systems can hand them tasks back. One deployment serves one team.

**English** | [简体中文](README.zh-CN.md)

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](./LICENSE)
[![Backend CI](https://github.com/paxoscn/vibeyeah/actions/workflows/backend.yml/badge.svg)](https://github.com/paxoscn/vibeyeah/actions/workflows/backend.yml)

---

## What VibeYeah is

Most "AI assistants" are chatbots: you ask, they answer, **you** go do the work.
VibeYeah is the opposite — a **Agent sandbox platform**. Every agent is a
real, permanently-online operator that actually *does* things — opens a browser, logs
into your internal systems, fills forms, runs code, reads/writes files, runs scheduled
jobs — then reports back with results and screenshots in your Feishu/WeChat chat. Each
agent is a dedicated sandbox powered by [**hermes-agent**](https://github.com/NousResearch/hermes-agent)
under a Rust (`axum`/`sea-orm`/`kube`) control plane.

### The six pillars

1. **Agent sandbox platform.** Every agent is an isolated, dedicated Ubuntu-desktop
   sandbox (`Deployment`) with its own Linux account, hermes gateway and persistent
   state — created, supervised and torn down by the control plane.
2. **Private, permanently-online agents you drive over Feishu/WeChat.** Agents stay
   online for you and are personal: drive them from Feishu/Lark or WeChat (`/add`
   provisions yours in seconds) and watch them work live over **WebRTC**.
3. **Deployed in the enterprise intranet — safe internal access & inbound triggers.**
   Sandboxes run inside your intranet / private VPC, so agents can **securely reach
   internal systems** (browser, terminal, files with your credentials), and internal
   systems can **hand agents tasks directly** via webhooks, `/callback/{skill}/{user_id}`,
   cron, or CI.
4. **Skills / knowledge / memory / environment context — centrally managed, iterated,
   distributed.** Agent skills, knowledge base, memory and environment context are
   managed as a shared catalog on the NAS and **distributed** to the right agents and
   users; agent state is **git-synced**, so capability and behavior **iterate** in a
   versioned, auditable way.
5. **A unified harness, wired into your internal CI/CD for agent-led iteration.** One
   common harness underpins every agent, and agent-produced state and artifacts can feed
   your enterprise CI/CD — letting the agent estate itself drive product & service
   iteration.
6. **A2A over Feishu — agents that team up.** Role-based agents can be composed into
   teams that coordinate over Feishu (agent-to-agent, A2A), so several specialized agents
   collaborate on one complex task.

## Architecture

```
   Feishu / WeChat                        HTTP clients
        │  (chat /  /add)                       │  (REST + JWT)
        ▼                                        ▼
┌───────────────────────────────────────────────────────────────────┐
│                     Backend  (Rust · axum)                          │
│  API · auth (JWT / Feishu login) · agent management ·               │
│  Kubernetes orchestration · Feishu/WeChat bots · callback routing · │
│  pod-status sync · git-sync of agent state                          │
└───────┬───────────────────────────────────────────┬────────────────┘
        │ creates/updates                            │ PostgreSQL / SQLite
        ▼                                            ▼
┌───────────────────────────────────────┐    ┌───────────────────┐
│  Agent Pod  (K8s Deployment, 1 replica)│    │  Database         │
│  Ubuntu desktop: Xvfb + openbox +      │    │  (PostgreSQL or   │
│  Chrome + Node/Python/Rust/Java        │    │   SQLite)         │
│  hermes gateway ×N (one per user,      │    └───────────────────┘
│  isolated HERMES_HOME, own Linux user) │
└───────┬───────────────────────────────┘
        │ reads/writes (per-agent & per-user data)
        ▼
┌───────────────────────────────────────────────────────────────────┐
│  Shared NAS (ReadWriteMany PVC) mounted at /data/nas                 │
│  vibeyeah/configs/                         ← shared seed template     │
│  vibeyeah/agents/<agent>/configs/          ← agent-level config       │
│  vibeyeah/agents/<agent>/users/<uid>/home/ ← per-user home + .hermes   │
└───────────────────────────────────────────────────────────────────┘
```

- **Backend** (`backend/`, Rust): the control plane, built as a reusable core library
  plus a small `vibeyeah` binary. Exposes a REST API, runs the Feishu long-connection
  bot, provisions/tears down agent `Deployment`s via the Kubernetes API, syncs pod
  status, prepares each user's NAS home on `/add`, and routes external callbacks into
  agent pods.
- **Agent pod** (`docker/desktop/`): the AI workstation. A headless Ubuntu
  desktop with Chrome and common toolchains, running one hermes gateway **per
  user** (each as its own Linux user with an isolated `HERMES_HOME`).
- **NAS**: a `ReadWriteMany` PVC persists agent and per-user config, skills,
  memory, and sessions across pod restarts.
- **WebRTC sidecar** (`docker/sidecar/`): optional mediamtx-based desktop
  streaming.

See [`docs/architecture.md`](docs/architecture.md) for details and
[`docs/deployment.md`](docs/deployment.md) for a deployment walkthrough.

## Repository Layout

```
backend/    Rust control plane — core library + `vibeyeah` binary (axum, sea-orm, kube)
docker/
  desktop/  Agent workstation image (Ubuntu + hermes + tools) & entrypoint
  sidecar/  Optional WebRTC streaming sidecar (mediamtx)
deploy/     Example Kubernetes manifests (deploy/test/)
docs/       Architecture & deployment guides
```

## Prerequisites

- A **Kubernetes** cluster (the backend uses in-cluster config or a kubeconfig).
- A **ReadWriteMany** storage volume (e.g. NFS/NAS) for the shared `vibeyeah`
  PVC, mounted at `/data/nas` in both the backend and agent pods.
- **PostgreSQL** for the backend (not needed for the SQLite first run in
  [Option A](#option-a--run-a-release-binary-fastest)).
- A **Feishu/Lark** app (and optionally **WeChat**) for messaging.
- An **LLM provider** with an OpenAI-compatible or Anthropic-compatible endpoint
  (e.g. Alibaba DashScope/Qwen, OpenAI).
- Container **registry** access for the backend, desktop, and sidecar images.

## Quick Start

### Option A — Run a release binary (fastest)

Download the latest release artifact (pick the one matching your OS/arch, e.g.
`vibeyeah-linux-x86_64`) from the
[Releases](https://github.com/paxoscn/vibeyeah/releases) page and run it in a terminal.
**No configuration is required to start** — on first launch the backend uses a local SQLite
file `./vibeyeah.db` (auto-created & migrated) and listens on `0.0.0.0:8080`:

```bash
chmod +x ./vibeyeah-linux-x86_64
./vibeyeah-backend            # optionally: DATABASE_URL=postgres://... BIND_ADDR=0.0.0.0:8080
```

**1 · Initialize the deployment — the first-run wizard.** Because the database is empty,
the binary starts an interactive wizard:

```
[初始化] 数据库中还没有任何组织。
是否现在创建首个组织，并为它绑定飞书应用作为 Bot 入口？[Y/n]   ← 回车即 Yes
请输入组织显示名称 [default]: my-org
✅ 已创建首个组织：my-org（slug=my-org，id=…）。
[绑定] 正在为组织注册飞书应用（Bot 入口）。请用飞书扫描下方二维码…
```

Press Enter / type `y`, enter a display name (default `default`), then **scan the printed QR
code with Feishu** (or open the link) to register/select the Feishu app that acts as the
**bot**. The app credentials are written back automatically, and the scanning account is
recorded as the **bot owner** — use that account to run admin commands such as `/set` below.
Press Enter to skip binding — you can still create the app later in the Feishu Open Platform
and put its `App ID` / `App Secret` into the `organizations` table, then restart.

**2 · Configure the LLM from Feishu.** Message the bot (the app you just bound) and, as the
bot owner, use `/set` to fill in the LLM the agents will use. This is **required before
anyone can `/add`**:

```
/set openai_base_url https://your-llm-endpoint.example   # do NOT include /v1
/set openai_api_key sk-xxxx
/set openai_model your-model
```

Each returns `✅ 已设置 …`.

**3 · Provision agents with `/add`.** Any Feishu user can message the bot with **`/add`**:

- the bot replies with a QR / link to register a **dedicated Feishu app** for that person's
  agent (a first-time user is auto-created);
- on success the backend provisions an agent sandbox pod and messages you when it is ready;
- you then chat directly with your own agent bot.

> ⚠️ Provisioning an actual agent pod also requires a **Kubernetes** cluster (the backend uses
> `~/.kube/config` or in-cluster config), a **shared NAS** seeded with `docker/desktop/configs/`
> at `<nas>/vibeyeah/configs/` and mounted at `/data/nas`, plus the **desktop image** in a
> registry your cluster can pull (image / PVC / namespace defaults live in the `settings` table —
> see [Configuration](#configuration)). Without that infrastructure, the wizard and the Feishu
> configuration steps above still work; `/add` will just fail at the "provision pod" stage.

### Option B — Deploy from Docker images / source

1. **Build images**

   ```bash
   # Agent workstation image
   docker build -t <your-registry>/vibeyeah-desktop:latest docker/desktop/
   # (optional) WebRTC sidecar
   docker build -t <your-registry>/vibeyeah-sidecar:latest docker/sidecar/
   # Backend
   cd backend && docker build -t <your-registry>/vibeyeah-backend:latest .
   ```

2. **Prepare the NAS seed** — copy `docker/desktop/configs/` to
   `<nas>/vibeyeah/configs/` so new agents/users can be seeded from it. The
   committed configs contain **placeholders only** (`__OPENAI_*__` /
   `__LARK_*__`); the real LLM / gateway values are rendered into the copies by
   the backend at `/add`, so don't hand-edit secrets into the seed.

3. **Configure the backend** — set `DATABASE_URL` (PostgreSQL, or leave empty
   for a local SQLite file) and optionally `BIND_ADDR`. Copy
   `backend/.env.example` to `backend/.env` as a template. All other settings
   live in the DB `settings` table and are seeded with defaults on first
   migration — edit them there. See [Configuration](#configuration).

4. **Deploy** — apply the manifests in `deploy/test/` (edit image references,
   namespace, PVC name, and secrets first), or use your own. Then run the
   backend once with an attached terminal so the first-run wizard initializes
   the platform and binds the Feishu bot.

5. **Provision an agent** — in Feishu, message your bot **`/add`**, scan the QR
   code to authorize a Feishu app, and VibeYeah will create your agent pod. Then
   just chat with it.

For local development see [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Configuration

Only two settings come from the environment — `DATABASE_URL` and `BIND_ADDR`. Everything
else is stored as key/value rows in the database `settings` table, which migration
`m20240019_create_settings` creates and seeds with defaults (see
[`backend/.env.example`](backend/.env.example) for the full key list). Edit the table and
restart the backend to apply. `DATABASE_URL` accepts PostgreSQL or SQLite
(`sqlite://vibeyeah.db` when empty).

| Key (`settings` table) | Default | Description |
| --- | --- | --- |
| `jwt_secret` | `change_me_to_a_long_random_secret` | Secret for signing JWTs — change to a long random value. |
| `jwt_expire_hours` | `72` | JWT lifetime. |
| `lark_app_id` / `lark_app_secret` | *(empty)* | Global Feishu app for **web login** (optional; the bot app is bound during first-run setup). |
| `k8s_namespace` | `default` | Default namespace for agents. |
| `pod_sync_interval_secs` | `30` | Pod status sync interval. |
| `desktop_image` / `sidecar_image` | `vibeyeah/*:latest` | Agent desktop / WebRTC sidecar images. |
| `webrtc_base_url` | `http://localhost:8889` | Base URL for live desktop viewing. |
| `nas_mount_root` | `/data/nas` | NAS mount point used by the backend when preparing user homes. |
| `nas_pvc_name` | `vibeyeah-nas-pvc` | Shared NAS PVC name mounted into agent pods. |
| `hermes_exec_timeout_secs` | `900` | Timeout for callback-triggered in-pod hermes runs. |
| `callback_token` | *(empty)* | If set, `/callback/...` requires `X-Callback-Token` (or `?token=`). |
| `sms_access_key_id` / `_secret` / `sign_name` / `template_code` | *(empty)* | SMS provider (reserved, for phone login). |

The **LLM the agents use** (`openai_base_url` / `openai_api_key` / `openai_model`) is *not*
a global `settings` key — the bot owner sets it at runtime with `/set` in Feishu (defaults
empty). `openai_base_url` should **not** include a `/v1` suffix.

## External Callback Routing

Any external system can trigger a user's skill:

```
GET|POST /callback/{skill}/{user_id}?foo=bar
```

The backend finds agent(s) that have `skill` installed for `user_id` under
`agents/<agent>/users/<user_id>/home/.hermes/skills/**/<skill>`, returns `202`
immediately, then invokes hermes inside the agent pod (as that user) with the
skill name, query params, and request body — trying candidate agents in order
until one succeeds. Protect it with `CALLBACK_TOKEN` in any reachable network.

## Security

⚠️ **Agents are high-privilege workloads** (browser, terminal, files) and the
system is designed for **trusted / enterprise-intranet** deployment. Read
[`SECURITY.md`](SECURITY.md) before deploying — it covers hardening guidance,
private vulnerability reporting, and an important **liability disclaimer**.

This software is provided under the Apache License 2.0 **"AS IS", without
warranty of any kind**. Operators are solely responsible for securing their
deployment. See [`SECURITY.md`](SECURITY.md#disclaimer--免责声明).

## Roadmap

- Skill marketplace (reusable industry automation modules)
- Multi-model routing (cost-optimal scheduling)
- Agent monitoring dashboard & task audit logs

## Contributing

Contributions are welcome! Please read [`CONTRIBUTING.md`](CONTRIBUTING.md) for
setup, conventions, and the pull-request process. This project follows the
[Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md).

## License

Licensed under the **Apache License 2.0** — see [`LICENSE`](LICENSE) and
[`NOTICE`](NOTICE). Third-party components (including hermes-agent and bundled
skills) are licensed under their own terms; see [`NOTICE`](NOTICE).
