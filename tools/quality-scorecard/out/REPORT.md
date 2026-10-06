# vibeyeah 代码质量评分卡

**方法**：TypeSafe System One (Jev) 复合打分。代码负责事实（LOC、panic 点、测试数、CI 配置），
Jev 负责需要语义理解的判断（错误处理、可读性、模块化、安全、可测性），再由代码按权重合成。
**范围**：`backend/src` 全部 Rust 生产代码（7779 行 / 52 文件），外加文档、构建部署配置、
已有测试三块横向材料。**不含** `docker/desktop/configs/.hermes/`（348 个跟踪文件，为第三方
Hermes skill 的 vendored 配置，非本团队所写）。
**日期**：2026-09-21。仓库首次提交 2026-09-10，仅 11 天历史。

---

## 总分

| 维度 | 得分 | /100 | 权重 | 平均置信度 |
|---|---:|---:|---:|---:|
| 安全性 security | 0.30 | **30** | 0.20 | 0.58 |
| 错误处理 error_handling | 0.69 | 69 | 0.15 | 0.57 |
| 测试覆盖 test_coverage | 0.53 | 53 | 0.15 | 0.46 |
| 模块化 modularity | 0.48 | 48 | 0.12 | 0.58 |
| 构建与部署 infrastructure | 0.37 | 37 | 0.10 | 0.76 |
| 可读性 readability | 0.87 | 87 | 0.10 | 0.75 |
| 可测性 testability | 0.56 | 56 | 0.10 | 0.67 |
| 文档 documentation | 0.86 | 86 | 0.08 | 0.75 |
| **综合** | **0.55** | **55** | | |

权重敏感度检验：等权 58 分，安全加权 51 分，安全/可读性对调 61 分。综合分对权重不敏感，
**55 分是稳健结论**。

代码行数加权的维度分与等权分基本一致（security 0.30/0.30，modularity 0.48/0.49），
说明分数不是被某个超大文件带偏的。

> 注：仓库只有 11 天历史。文档和可读性得分高，是因为新代码通常还没被时间侵蚀；
> 模块化与安全性得分低，是因为这两项在早期最容易欠账，且后期修复成本最高。

---

## 红线（独立于加权分）

加权平均会掩盖单个致命缺陷，因此这三条作为硬性条件单独判定，不参与补偿：

| 编号 | 条件 | 结果 |
|---|---|---|
| SEC-1 | 请求可控的值不得进入 shell | **FAIL** |
| TEST-1 | 特权路径至少有一个测试 | **FAIL** |
| DEP-1 | 构建端到端可复现 | **FAIL** |

### SEC-1：命令注入（已逐行核实）

`api/sync.rs` 从**任意组织成员**的请求体中取 `paths` 和 `commit_message`，交给
`service/sync.rs::build_sync_script`，后者用 `format!` 拼成单行 shell 脚本，
经 `k8s.rs::exec_in_pod` 以 `sh -c` 在 agent pod 内执行：

```rust
for SRC in {path_list}; do          // paths 未加引号，`; 命令;` 直接注入
git remote add origin '{git_url}'   // git_url 来自组织配置，单引号可被 ' 闭合
git commit -m '{commit_message}' && // commit_message 同样可闭合单引号
git push origin {branch} &&         // branch 未加引号
```

后果不止于「在自己的 pod 里执行命令」：`git_sync_org_agents` 会把组织的
`git_password` / `git_ssh_private_key` 解密后注入同一容器，因此**普通成员可借此读到
组织级的 git 凭据**，构成越权。

模型对 `service/sync` 的 `unvalidated_input_to_sink` 判定为 **P(yes)=0.92** —— 这是本次
打分中最有价值的一次命中。

**修复方向**：`paths` 语义上应是固定白名单（如 `/home/agent/.hermes/SOUL.md`），
按白名单校验后拼接；其余插值一律做 shell 转义，或改为不经 shell 的参数化执行。

### TEST-1：10/14 个模块零测试

有测试的只有 4 个模块（`app_entry`、`user_home`、`callback`、`lark_bot`）。
**`auth`、`k8s`、`api`、`organization`、`sync` 全部无测试** —— 恰好覆盖了鉴权、
集群操作和数据同步这些最需要回归保护的地方。全仓 1.9 个测试/KLOC。

### DEP-1：构建未完全钉死

`Cargo.lock` 已提交且 CI 用 `--locked`（好），但基础镜像按 tag 浮动
（`ubuntu:22.04`、`desktop:20260608`、`desktop:20260612`），CI 用 `stable` 工具链而非固定版本。

---

## 逐模块得分

```
模块                    行数    错误   可读   模块   安全   可测   均值
service/lark_bot        1276   0.52   0.79   0.20   0.32   0.37   0.44  ←
service/auth             237   0.50   0.76   0.16   0.33   0.56   0.46  ←
service/k8s              716   0.61   0.88   0.38   0.18   0.46   0.50
service/wechat_qr        109   0.66   0.66   0.59   0.22   0.55   0.54
service/sync             247   0.71   0.91   0.39   0.14   0.55   0.54
service/agent            285   0.84   0.79   0.25   0.45   0.45   0.56
service/organization     227   0.68   0.94   0.58   0.13   0.54   0.57
app_entry                643   0.56   0.96   0.38   0.44   0.60   0.59
service/callback         373   0.66   0.99   0.47   0.20   0.66   0.59
service/lark_qr          350   0.76   0.88   0.62   0.36   0.50   0.62
api                      672   0.68   0.87   0.62   0.46   0.61   0.65
config_middleware        170   0.75   0.80   0.61   0.57   0.64   0.67
service/user_home        430   0.85   0.98   0.75   0.20   0.63   0.68
data_layer              2044   0.82   0.90   0.77   0.23   0.73   0.69
```

**模块化判断的独立佐证**：模型给出 modularity 最低的三个模块是 `lark_bot`(0.20)、
`auth`(0.16)、`agent`(0.25)。用与模型无关的结构化度量核对——生产代码中最长的三个函数
恰好落在同一批模块：

| 行数 | 函数 | 文件 |
|---:|---|---|
| 279 | `create_agent_deployment` | `service/k8s.rs` |
| 231 | `handle_add` | `service/lark_bot.rs` |
| 174 | `create_agent` | `service/agent.rs` |

195 个生产函数中 14 个超过 80 行。语义判断与结构度量方向一致，这一维度的结论可信。

---

## 模型可靠性核实

按 TypeSafe 文档「结构化输出保证接口，不保证真值」，我对高置信度的极端判定做了人工复核，
发现两处**词形误判**并已修正（原始判断保留在 `raw.json`）：

| 判定 | 原始 | 修正 | 依据 |
|---|---:|---:|---|
| `data_layer.hardcoded_secret` | 0.70 | 0.02 | 命中 `lark_app_secret` / `image_pull_secret` 两个**数据库列名**。读 `entity/organization.rs` 确认无字面量凭据。 |
| `infrastructure.secret_hygiene` | 0.30（最低档） | 0.33 | 最低档理由是 `deploy/test/init.sql` 里有 `REPLACE_WITH_YOUR_LARK_APP_SECRET`、`deployment.yaml` 里有 `postgres:postgres`。二者都是**测试清单里的占位符**，且 init.sql 明确写了「never commit real credentials」。真实问题只有一个：`.gitignore` 用 `!docker/desktop/configs/.hermes/.env` 强制放行该文件，日后有人填了真 key 会被提交。 |

也就是说：**模型在「这段代码的形状像不像有密钥」上会误报，在「这个值是不是流向了危险汇点」上判得准**。
前者靠读代码可证伪，后者需要跨文件数据流追踪——恰好是模型更擅长的部分。

置信度低于 0.50、建议人工复核的判断：`error_handling` 的 `callback`/`auth`/`data_layer`，
`security` 的 `k8s`/`user_home`，`testability` 的 `lark_qr`/`data_layer`。

---

## 客观指标

| 指标 | 值 |
|---|---|
| 生产代码 | 7779 行 / 52 文件 |
| 生产代码 panic 点（unwrap/expect/panic!） | 14（1.8 / KLOC） |
| `unsafe` | 0 |
| 测试标记数 | 15（1.9 / KLOC） |
| 零测试模块 | 10 / 14 |
| SQL 字符串拼接 | 0（全部走 sea-orm） |
| CI | fmt + clippy（仅告警，未 `-D warnings`）+ build --locked + test |

值得点名：`service/wechat_qr.rs` 109 行里有 4 处 panic，密度全仓最高（36.7 / KLOC）；
`service/organization.rs` 227 行 4 处。这两处是低成本即可还清的技术债。

---

## 优先修复顺序

1. **SEC-1 命令注入**（`service/sync.rs` + `api/sync.rs`）——唯一的高危项，越权读组织凭据。
2. **给 `auth` / `k8s` / `api` 补测试**——目前这三块改动完全没有回归保护。
3. **拆分三个超长函数**（`create_agent_deployment` 279 行、`handle_add` 231 行、`create_agent` 174 行）——
   模块化是权重第三高的维度，且这三处同时拉低可读性与可测性。
4. **钉死基础镜像与工具链**——`DEP-1`，改动成本极低。
5. **收掉 `wechat_qr` / `organization` 的 panic 点**——低成本。
6. **把 CI 的 clippy 提升为 `-D warnings`**——注释里写着「pre-existing style lints」，
   说明团队知道有存量告警，值得排期清理。

---

## 复现

```
/tmp/vibeyeah-quality/
  harness.py     收集单元 + 客观指标 + 调用 TypeSafe（写 out/raw.json）
  scorecard.py   读 raw.json + metrics.json，按权重合成（不重新推理）
  out/raw.json         17 次调用的原始判断（缓存）
  out/metrics.json     客观指标
  out/scorecard.json   合成结果
```

推理只跑一次并落盘，改 `scorecard.py` 里的 `WEIGHTS` 即可重新配分，无需再付推理成本。
本次用量：输入 122,471 tokens / 输出 2,060 tokens。

**已知局限**：单元粒度判断依赖送入的源码；`data_layer` 的 migration 只抽样了首尾各 3 个
（客观指标仍统计全部）；单元划分与维度权重是本次设定的默认值，不是唯一正确答案。
