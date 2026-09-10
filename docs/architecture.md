# Architecture

VibeYeah provisions **persistent AI agent workstations** on Kubernetes and lets
users drive them through messaging apps (Feishu/Lark, WeChat) and external
callbacks. A deployment hosts a single team's agents.

## Components

```
┌──────────────────────┐                    ┌───────────────────────────────┐
│  Feishu/Lark · WeChat│                    │  Agent Pod (K8s Deployment)   │
│  (chat / /add)       │                    │  Ubuntu desktop + hermes ×N   │
└──────────┬───────────┘                    └───────────────┬───────────────┘
           │                                                │
┌──────────▼───────────┐     ┌──────────────┐       ┌───────▼────────┐
│   Backend            │     │  PostgreSQL  │       │  Shared NAS    │
│   Rust/axum · kube   │     │  or SQLite   │       │  (RWX PVC)     │
└──────────────────────┘     └──────────────┘       └────────────────┘
```

### Backend (`backend/`, Rust)
The control plane, shipped as a reusable core library plus a small `vibeyeah`
binary. Responsibilities:
- **REST API** (axum) for users and agents; JWT auth plus Feishu (Lark) web login
  and phone/SMS login.
- **Kubernetes orchestration** (kube): creates one single-replica `Deployment`
  per agent in the configured namespace, using in-cluster config or a
  kubeconfig. Pods are located by the `app=<name>` label (`find_agent_pod`),
  since Deployment pod names are not stable.
- **Feishu bot** (openlark, long-lived WebSocket): handles `/add` (agent
  provisioning), `/set` (owner-only LLM configuration), and chat.
- **WeChat** provisioning (QR-based credential capture).
- **External callback routing**: `/callback/{skill}/{user_id}` →
  `service::callback`.
- **User home provisioning**: on `/add`, `service::user_home::prepare_user_home`
  seeds the user's NAS home and renders credentials into it before the workload
  is created.
- **Pod status sync** (periodic) and **git-sync** of agent state.

### Agent pod (`docker/desktop/`)
A headless Ubuntu desktop that hosts the AI agent:
- **Xvfb** virtual display + **openbox** window manager + **Google Chrome** +
  language toolchains (Node, Python/uv, Rust, Java) + **claude-code** +
  Playwright.
- **hermes-agent**, installed system-wide (FHS, `/usr/local/bin/hermes`) so it
  can be run by multiple Linux users.
- **Multi-user model**: `entrypoint.sh` enumerates the agent's users on the NAS
  and, for each user, creates a Linux account, links the shared `.claude`
  config, `chown`s the user's home, and starts an **independent `hermes
  gateway`** as that user with `HERMES_HOME` pointed at the user's NAS home.
  Each gateway runs in its own session (`setsid`), and PID 1 stays alive
  independently so a single gateway exiting never restarts the pod.

### Storage (shared NAS, ReadWriteMany PVC)
Mounted at `/data/nas` in the backend and agent pods:

```
/data/nas/vibeyeah/
├── configs/                              # shared seed template (.hermes, .claude*)
└── agents/<agent>/                       # <agent> = Deployment name = agent-<16 hex>
    ├── configs/                          # agent-level config (copied by init container)
    └── users/<user_id>/home/             # per-user home
        └── .hermes/                      #   user's config, .env (creds), skills, memory
```

The seed template ships with `__OPENAI_*__` / `__LARK_*__` placeholders; the
backend renders the real values into the per-user (and agent-level) copies when
it provisions an agent. The init container copies `configs/` →
`agents/<agent>/configs/` (no-clobber) on pod start; per-user homes are seeded
by the backend on `/add`.

### WebRTC sidecar (`docker/sidecar/`, optional)
A mediamtx-based sidecar that can stream the agent desktop for live viewing.

## Key Flows

### `/add` — provision an agent
1. User messages the bot `/add`.
2. Backend resolves/creates the user, runs the Feishu QR authorization to
   register the user's own bot app, and probes the bot identity.
3. Backend prepares the user's NAS home (`prepare_user_home`): seeds `.hermes`
   from the shared template and renders credentials (Lark + the LLM configured
   via `/set`) into `.env` / `config.yaml` / `.claude/settings.json`.
4. Backend creates the agent `Deployment` (init container seeds agent config;
   desktop container starts).
5. The pod's `entrypoint.sh` creates the Linux user and starts that user's
   hermes gateway, which connects to Feishu.
6. Backend polls pod readiness and notifies the user in Feishu.

### Chat
The user's messages reach their agent's hermes gateway directly via Feishu
(each user's gateway uses that user's own bot credentials), so conversation
traffic does not transit the backend.

### External callback
`GET/POST /callback/{skill}/{user_id}` → backend scans the NAS for agents that
have `skill` installed for `user_id`, returns `202`, then (in the background)
runs `hermes chat -q <prompt> --quiet -s <skill>` inside the candidate agent
pods **as that user** (via `HERMES_HOME` and that user's Linux account),
stopping at the first success. See `backend/src/service/callback.rs`.

## Isolation
Within an agent pod, **users** are isolated by Linux account and by a separate
`HERMES_HOME` (config, skills, memory, sessions) on the NAS. Each agent runs in
its own `Deployment` with its own NAS directory tree.

## Security note
Agents are high-privilege (browser, terminal, files). The platform is intended
for trusted/intranet deployment. See [`SECURITY.md`](../SECURITY.md).
