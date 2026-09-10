# Contributing to VibeYeah

Thanks for your interest in contributing! This document explains how to set up
your environment, the conventions we follow, and how to submit changes.

By participating in this project you agree to abide by the
[Code of Conduct](CODE_OF_CONDUCT.md).

## Project Layout

- `backend/` — Rust control plane (axum, sea-orm, kube).
- `frontend/` — React + Vite + TypeScript web UI.
- `docker/desktop/` — agent workstation image and `entrypoint.sh`.
- `docker/sidecar/` — optional WebRTC streaming sidecar.
- `deploy/` — example Kubernetes manifests and DB seed.
- `docs/` — architecture and deployment guides.

## Development Setup

### Backend (Rust)

Requires a recent [Rust toolchain](https://rustup.rs/) (edition 2021).

```bash
cd backend
cp .env.example .env          # then fill in DATABASE_URL, JWT_SECRET, etc.
cargo build                   # debug build
cargo test                    # run unit tests
cargo fmt                     # format (run before committing)
cargo clippy --all-targets    # lints
```

You will need a reachable PostgreSQL and, for full functionality, a Kubernetes
cluster (or a kubeconfig). Many changes can be developed and unit-tested without
a live cluster.

### Frontend (Node)

Requires Node.js 20+ and npm.

```bash
cd frontend
npm ci
npm run dev        # local dev server
npm run build      # type-check (tsc) + production build
npm run lint       # eslint
```

### Agent image (Docker)

```bash
docker build -t vibeyeah-desktop:dev docker/desktop/
# run locally (see docker/desktop/run-local*.sh for helpers)
```

## Making Changes

1. **Create a branch** off `main` with a descriptive name, e.g.
   `feat/agent-restart` or `fix/callback-timeout`.
2. **Keep changes focused.** One logical change per pull request.
3. **Add or update tests** where practical (backend: `cargo test`; frontend:
   keep `npm run build` and `npm run lint` green).
4. **Match the surrounding style.** Rust: `cargo fmt` + `cargo clippy` clean.
   Frontend: eslint clean. Shell: `bash -n` and prefer POSIX-friendly syntax.
5. **Update docs** (`README.md`, `docs/`, `CHANGELOG.md`) when you change
   behavior, configuration, or the public API.

### Commit Messages

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <summary>

feat:     a new feature
fix:      a bug fix
refactor: a code change that neither fixes a bug nor adds a feature
docs:     documentation only
chore:    build/tooling/release housekeeping
test:     adding or correcting tests
```

Example: `fix(entrypoint): keep PID 1 alive without waiting on child processes`.

### Pull Requests

- Fill in the PR template (if present): what & why, how you tested it, and any
  deployment/config notes.
- Ensure CI is green.
- Request review from a maintainer.

## Security

Please **do not** open public issues for security vulnerabilities. Follow the
private reporting process in [`SECURITY.md`](SECURITY.md).

## Licensing

This project is licensed under [Apache-2.0](LICENSE). By submitting a
contribution you agree that it may be distributed under that license. Do not
commit secrets, customer data, or third-party code whose license is incompatible
with Apache-2.0.
