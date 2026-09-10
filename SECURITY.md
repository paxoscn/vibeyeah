# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

Only the latest released minor version receives security fixes. Users are
strongly encouraged to run the newest release and to track this repository for
updates.

## Reporting a Vulnerability

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, use one of the following private channels:

- GitHub → **Security** tab → **Report a vulnerability** (private security
  advisory), if enabled for this repository; or
- Email the maintainers at the address listed in the repository profile /
  `README.md`.

Please include:

1. A description of the issue and its potential impact.
2. Step-by-step reproduction instructions (or a proof of concept).
3. Affected version(s) and deployment configuration.
4. Any suggested remediation, if you have one.

We aim to acknowledge reports within **5 business days** and to provide an
initial assessment within **10 business days**. We will coordinate disclosure
with you and credit reporters who follow this responsible-disclosure process
(unless you prefer to remain anonymous).

## Threat Model & Hardening Guidance

VibeYeah orchestrates **AI agents that can drive a real browser, execute
terminal commands, and read/write files** inside per-user containers, and it
connects to messaging platforms (Feishu/Lark, WeChat) and to a Kubernetes
cluster. Treat every agent as a **high-privilege workload**:

- **Network**: Deploy the backend, agent pods, and the shared NAS inside a
  trusted, isolated network (e.g. a corporate intranet or a private VPC).
  Do **not** expose agent desktops, the hermes gateway, or management
  interfaces directly to the public internet. Put the backend behind an
  authenticating reverse proxy / ingress with TLS.
- **Credentials**: Provide LLM, Feishu/Lark, and WeChat credentials via
  environment variables or a secrets manager. Never commit real keys (see the
  `.env.example` templates). Rotate any credential that may have been exposed.
- **Kubernetes**: Run agent pods in a dedicated namespace with appropriate
  `NetworkPolicy`, resource limits, and (where your platform allows) a
  restrictive Pod Security standard. The desktop entrypoint creates per-user
  Linux accounts via passwordless `sudo`; understand this before deploying.
- **Callback route**: `/callback/{skill}/{user_id}` is unauthenticated by
  default. In any reachable environment, set `CALLBACK_TOKEN` and require it,
  and restrict who can reach the endpoint.
- **Least privilege**: Scope the LLM/API keys, platform bot permissions, and
  the cluster RBAC used by the backend to the minimum required.
- **Auditing**: Agent activity (browser/terminal/file operations) can be
  sensitive. Enable and retain logs, and review the WebRTC desktop stream
  where appropriate.

## Disclaimer / 免责声明

> **English.** This software is provided under the Apache License 2.0
> "**AS IS**, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND" (see `LICENSE`,
> Sections 7 and 8). It is intended for deployment in **trusted, controlled
> environments such as a corporate intranet**. The operator/deployer is solely
> responsible for securing their own deployment — including network isolation,
> authentication, authorization, secrets management, least-privilege access,
> monitoring, and compliance with applicable laws and internal policies. The
> maintainers and contributors are **not liable** for any security incidents,
> data loss or breach, unauthorized access, or any direct, indirect,
> incidental, special, or consequential damages arising from software
> vulnerabilities, misconfiguration, or from deploying or exposing the system
> in untrusted or internet-facing environments. Because the agents can execute
> arbitrary browser/terminal/file operations with the credentials you supply,
> you assume all risk associated with their use.
>
> **中文.** 本软件依据 Apache License 2.0「**按原样**（AS IS），不提供任何明示或
> 默示的担保」提供（见 `LICENSE` 第 7、8 条）。本项目**面向受信任的、受控的部署环境
> （如企业内网）**。部署者/运维方须自行负责其部署的安全，包括但不限于网络隔离、身份
> 认证、访问授权、密钥管理、最小权限、监控审计，以及遵守适用的法律法规与内部制度。
> 对于因软件漏洞、配置不当，或将系统部署/暴露于不可信或公网环境而导致的安全事件、
> 数据丢失或泄露、未授权访问，或任何直接、间接、偶发、特殊或后果性损害，维护者与
> 贡献者**概不承担责任**。由于 Agent 能够以您提供的凭据执行任意浏览器/终端/文件操作，
> 使用本软件的一切风险由您自行承担。

If you discover a vulnerability, please follow the responsible-disclosure
process above so we can address it for all users.
