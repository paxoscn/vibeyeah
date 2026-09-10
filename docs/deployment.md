# Deployment Guide

This guide walks through deploying VibeYeah on Kubernetes. It assumes a
**trusted / intranet** environment — see [`SECURITY.md`](../SECURITY.md) for the
threat model and the liability disclaimer.

## Prerequisites

- A **Kubernetes** cluster (1.24+) and `kubectl` access.
- A **ReadWriteMany** storage class / volume (e.g. NFS, NAS) for the shared
  `vibeyeah` PVC. Both the backend and agent pods mount it at `/data/nas`.
- **PostgreSQL** reachable from the backend (or use SQLite for a single-node
  trial — see [`README`](../README.md#configuration)).
- A container **registry** for the backend, desktop, and (optional) sidecar
  images.
- A **Feishu/Lark** app (and optionally **WeChat**) for messaging.
- An **LLM** endpoint (OpenAI-compatible or Anthropic-compatible), e.g. Alibaba
  DashScope/Qwen or OpenAI.

## 1. Build & push images

```bash
# Agent workstation
docker build -t <registry>/vibeyeah-desktop:latest docker/desktop/
docker push <registry>/vibeyeah-desktop:latest

# Optional WebRTC sidecar
docker build -t <registry>/vibeyeah-sidecar:latest docker/sidecar/
docker push <registry>/vibeyeah-sidecar:latest

# Backend (write a Dockerfile for your environment, or build the Rust binary)
cd backend && cargo build --release
```

> The desktop image installs `hermes-agent` system-wide (as root) and runs its
> entrypoint as root so it can create per-user Linux accounts. Review
> `docker/desktop/Dockerfile` and `entrypoint.sh` before deploying.

## 2. Prepare the NAS

Mount your RWX volume and seed the shared template:

```bash
# <nas> is the path where the RWX volume is mounted on your seeding host
mkdir -p <nas>/vibeyeah
cp -a docker/desktop/configs/. <nas>/vibeyeah/configs/
```

Leave the seed config as-is — the committed copies contain **placeholders only**
(`__OPENAI_*__` / `__LARK_*__`). The backend renders the real LLM credentials
(configured via the bot's `/set` command) and each user's Lark credentials into
the copies it makes under `agents/<agent>/` when an agent is provisioned, so do
not hand-edit secrets into the seed.

## 3. Configure & deploy the backend

Start from the example manifests in `deploy/test/`:
- `deployment.yaml` — backend Deployment (mounts the NAS PVC at `/data/nas`).
- `service.yaml` — backend Service.

Edit `deployment.yaml`:
- Set the backend `image` to your registry image.
- Set `DESKTOP_IMAGE` (and `SIDECAR_IMAGE` if used) to your agent images.
- Provide `DATABASE_URL` (and optionally `BIND_ADDR`) via environment or a
  Secret. **All other settings** — including the desktop/sidecar images,
  namespace and PVC name — live in the database `settings` table, which is
  created and seeded with defaults on first startup. See the configuration
  table in the [`README`](../README.md#configuration); adjust `deploy/test/deployment.yaml`
  if you keep the example's env-var style.
- Ensure the NAS PVC is mounted at `/data/nas`.

Apply:

```bash
kubectl apply -f deploy/test/service.yaml
kubectl apply -f deploy/test/deployment.yaml
```

The backend runs DB migrations on startup, begins syncing pod statuses, and
spawns the Feishu bot. On a fresh database it runs a **first-run wizard** on
stdin: run the backend once with an attached terminal
(`kubectl attach` / a local binary against the same DB) to initialize the
platform and bind the Feishu bot, or bind it later through the Feishu Open
Platform and restart.

## 4. Create the NAS PVC

Create a `PersistentVolumeClaim` named `vibeyeah-nas-pvc` (or the `nas_pvc_name`
you set in the `settings` table) backed by your RWX storage, in the namespace
where agents run. Agent pods created by the backend mount this PVC at
`/data/nas`.

## 5. Configure the LLM and provision an agent

Before anyone can create an agent, the bot owner must configure the LLM. In
Feishu, message the bot:

```
/set openai_base_url https://your-llm-endpoint.example   # no /v1 suffix
/set openai_api_key sk-xxxx
/set openai_model your-model
```

Then any user can message the bot:

```
/add
```

Scan the QR code to authorize a Feishu app for your agent. The backend prepares
your NAS home and creates the agent `Deployment`. When the pod is ready, the bot
notifies you and you can start chatting with your agent.

## 6. (Optional) External callbacks

Expose `/callback/{skill}/{user_id}` to trigger a user's skill from external
systems. **Always** set `CALLBACK_TOKEN` and require it
(`X-Callback-Token` header or `?token=`) when the endpoint is reachable beyond a
trusted network.

## Production hardening checklist

- [ ] Put the backend behind TLS + an authenticating ingress/reverse proxy.
- [ ] Use strong, unique `JWT_SECRET` and `CALLBACK_TOKEN`; manage them in a
      secrets store, not in manifests.
- [ ] Restrict the agent namespace with `NetworkPolicy` and resource limits.
- [ ] Keep agent desktops, hermes gateways, and management interfaces **off** the
      public internet.
- [ ] Scope LLM/platform/cluster credentials to least privilege.
- [ ] Rotate any credential that may have been exposed; enable log retention and
      auditing.
- [ ] Review the desktop `entrypoint.sh` (it creates Linux users via passwordless
      `sudo`) and the agent image before deploying.

See [`SECURITY.md`](../SECURITY.md) for the full threat model and disclaimer.
