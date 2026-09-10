# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Open-source readiness: Apache-2.0 `LICENSE`, `NOTICE`, `README`,
  `SECURITY.md` (with vulnerability-reporting process and an enterprise-intranet
  liability disclaimer), `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, this
  `CHANGELOG`, hardened `.gitignore`, GitHub Actions CI, and `docs/`.
- Removed committed secrets and customer/internal content from the repository;
  seed configs now ship with placeholders only.

### Changed
- Restructured the backend as a reusable **core library** (`vibeyeah-core`)
  plus a small `vibeyeah` binary. The core targets a **single deployment** and
  exposes the user and agent APIs plus provisioning only.
- First-run bootstrap is now an interactive wizard: the auto-seeded default
  organization migration was removed, and when the `organizations` table is
  empty the backend prompts to create the platform's initial organization
  (default name `default`) and to bind its Feishu/Lark bot app via a
  scan-to-register QR code (skippable; non-interactive runs only log).
- Configuration moved from environment variables to the database `settings`
  table (only `DATABASE_URL` / `BIND_ADDR` remain env-driven).

## [0.1.0]

### Added
- **Control plane (Rust / axum / sea-orm / kube)**: REST API, JWT auth, Feishu
  (Lark) web login, phone/SMS login, and agent management.
- **Kubernetes orchestration**: each agent runs as a single-replica
  `Deployment`. Pod status is synced periodically.
- **Agent workstation image** (`docker/desktop/`): headless Ubuntu desktop
  (Xvfb + openbox) with Chrome and Node/Python/Rust/Java toolchains, running
  [hermes-agent](https://github.com/NousResearch/hermes-agent).
- **Multi-user agent pods**: the entrypoint enumerates users under
  `agents/<agent>/users/<user_id>`, creates a Linux account per user, links the
  shared `.claude` config, and starts an independent hermes gateway per user
  (isolated `HERMES_HOME` on shared NAS). PID 1 stays alive independently of any
  child process, and each gateway runs in its own session (`setsid`).
- **Messaging integration**: Feishu/Lark long-connection bot (`/add` to
  provision an agent via QR authorization, chat to drive it) and WeChat
  provisioning.
- **`/add` provisioning**: prepares the user's NAS home (`.hermes` seeded from
  the shared template with the user's Lark credentials) before creating the
  agent workload.
- **External callback routing**: `GET/POST /callback/{skill}/{user_id}` locates
  agents that have the skill installed for the user and invokes hermes in-pod
  (as that user) with the skill name, query params, and request body, falling
  back across candidate agents.
- **Persistence**: agent and per-user state on a shared ReadWriteMany NAS PVC;
  optional git-sync of agent state to a private repository.
- **Live view**: optional WebRTC desktop streaming via a mediamtx sidecar
  (`docker/sidecar/`).

[Unreleased]: https://github.com/paxoscn/vibeyeah/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/paxoscn/vibeyeah/releases/tag/v0.1.0
