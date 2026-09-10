---
name: feishu-project
description: 飞书项目管理(Feishu Project) — headless QR登录 + 创建需求表单自动化。Meego框架(React+MobX)交互模式、字段配置、API参考。
tags: [feishu, lark, project, login, sso, playwright, headless]
version: 2
---

# 飞书项目管理 Headless 登录 + 创建需求

## 触发条件
- 用户需要访问飞书项目管理 (project.feishu.cn)
- 用户提到 "新建需求" / "创建需求" / "create story" / "feishu project"

## 创建需求前：必须确认的信息

**执行脚本前，必须先向用户确认以下参数：**

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `STORY_TITLE` | 需求标题 | **必填，无默认** |
| `BUSINESS_LINE_SEARCH` | 业务线搜索关键词（叶子节点名） | **必填，无默认** |
| `PRODUCT_MANAGER` | 产品经理姓名 | `莫日根` |
| `DEV_LEAD` | 开发负责人姓名 | `莫日根` |
| `TEST_LEAD` | 测试负责人姓名 | `莫日根` |
| `EXPECTED_DATE` | 期望上线日期 | `2026-08-01` |
| `PROJECT_NAME` | 所属项目搜索关键词 | `数字化中心-产品迭代项目` |
| `CATEGORY` | 需求分类 | `产品需求` |
| `SOURCE` | 需求来源 | `数字化中心` |

确认后用 `clarify` 工具或直接回复用户确认，然后再修改脚本常量并执行。

## 登录流程

```bash
source ~/playwright_env/bin/activate && PYTHONUNBUFFERED=1 python3 ~/.hermes/skills/software-development/feishu-project-login/scripts/login.py 2>&1 | tee /tmp/feishu_project_login.log
```

后台运行，等 QR 截图就绪后发给用户扫码。

### 登录日志事件

| event | 含义 |
|-------|------|
| `already_logged_in` | Cookie 有效，无需重新登录 |
| `qr_ready` + `path` | QR 码已就绪，截图在 path |
| `scan_success` | 扫码成功 |
| `timeout` | 180s 超时未扫码 |

## 创建需求流程

```bash
source ~/playwright_env/bin/activate && PYTHONUNBUFFERED=1 python3 ~/.hermes/skills/software-development/feishu-project-login/scripts/create_story.py 2>&1 | tee /tmp/create_story.log
```

脚本路径: `~/.hermes/skills/software-development/feishu-project-login/scripts/create_story.py`

### 10 个字段自动填写顺序

1. **需求标题** — textarea，mouse.click 激活 → type → blur
2. **需求分类** — tree-select 单级，mouse.click → 点选项
3. **需求来源** — select，mouse.click → 点选项
4. **期望上线日期** — date，mouse.click → 找 input → type + Enter
5. **产品经理** — person-select，mouse.click → 搜索框输入 → 选 `.semi-select-option`
6. **所属项目** — workitem_related_select，**先 scrollIntoView** → mouse.click → 搜索选择
7. **业务线** — tree-select 搜索模式（详见下方关键规则）
8. **Roles - 产品经理** — 人员选择器搜索模式
9. **Roles - 开发负责人** — 人员选择器搜索模式
10. **Roles - 测试负责人** — 人员选择器搜索模式

提交：点击 `Create` 或 `创建` 按钮。

## 🚨 关键规则（经验教训）

### 1. 业务线：搜索框 + 叶子节点 + 不要 store 注入

业务线是三级树形结构（如：运营中台 > 基本档与主数据 > 门店中心）。

**正确流程：**
1. `scrollIntoView` 到业务线字段
2. 点击 `form-field-main` 激活
3. 找到出现的 `input`，输入**叶子节点名称**（如"门店中心"）
4. 等 5 秒让树形下拉加载
5. 在 `meego-tree-select__dropdown` 中找到 `span.meego-tree-select__option-label`，其父 `li` 必须有 `level-3` 且**没有** `unavailable`
6. **真实鼠标点击**该 span 的中心坐标
7. 等 3 秒，然后**点击空白区域关闭下拉**（不用 Escape）
8. **不要做 store 注入** — UI 选择已正确设置 store 值，注入会覆盖为父节点 ID 导致验证失败

**错误做法（已验证会失败）：**
- ❌ 点击二级父节点（如"基本档与主数据"）— 不是叶子节点，验证不通过
- ❌ `dispatchEvent` 合成事件 — Meego/React 不识别，不触发 store 更新
- ❌ store 注入 `fieldStore.business = 'parentId'` — 覆盖 UI 选择的正确叶子值
- ❌ 用 `*` 通用选择器找下拉选项 — 可能点到大容器而非正确元素

### 2. Roles 人员选择：必须用 `.semi-select-option`

**正确流程：**
1. 找到 role label span（如"开发负责人"），`scrollIntoView`
2. 等 1.5 秒，re-query 位置
3. 点击 `labelX + 250, labelCy`（picker placeholder 区域）
4. **等 6 秒**让 picker 完全展开
5. 找到 search input，输入人名，**等 8 秒**让搜索结果加载
6. 在搜索结果中找 **`.semi-select-option`** 元素（不是 `*` 通用选择器！）
7. 真实鼠标点击该元素的中心坐标
8. 等 5 秒让组件内部 state 同步
9. 用 `close_popups()`（Escape × 2 + click blank）关闭

**错误做法：**
- ❌ 用 `document.querySelectorAll('*')` 找搜索结果 — 可能点到 tooltip/popover 而非 option
- ❌ 减少等待时间 — Meego 组件渲染慢，6s/8s/5s 是最低要求

### 3. Meego/React 交互铁律

| 方式 | 效果 | 适用场景 |
|------|------|----------|
| `page.mouse.click(x, y)` | **真实鼠标事件**，React 合成事件系统正确捕获，MobX store 更新 | **所有表单交互** |
| `page.evaluate(() => el.click())` | 仅更新 React 局部显示，**不触发 MobX store 更新** | 无（不要用于表单） |
| `el.dispatchEvent(new MouseEvent(...))` | 合成事件，Meego 不识别 | 无（不要用于表单） |
| `evaluate` + fiber 赋值 | 绕过 UI 直接设值 | 仅日期字段 fallback |

### 4. evaluate() 中必须用 function() + arguments

```javascript
// ✅ 正确
page.evaluate("""function() { return arguments[0]; }""", value)

// ❌ 错误 — Arrow functions 在严格模式下没有 arguments
page.evaluate("""() => { return arguments[0]; }""", value)
```

### 5. 所属项目必须先 scrollIntoView

该字段默认在视口下方（y>1000），必须先 `label.scrollIntoView({block:'center'})` 再获取坐标并点击。

### 6. close_popups() 的使用

- **Roles 选完后用 `close_popups()`（含 Escape）** — 没问题，Escape 不会清除 Roles 选择
- **业务线选完后用 `mouse.click(350, 300)` 关闭** — 不用 Escape（可能取消 tree-select 选择）
- **每步操作后必须关闭弹窗** — 否则下一步操作被遮挡

## MobX Store 架构

详见 `references/mobx-store-architecture.md`

## 文件结构

```
~/.hermes/skills/software-development/feishu-project-login/
├── SKILL.md                        # 本文件
├── scripts/
│   ├── login.py                    # 登录脚本
│   └── create_story.py             # 创建需求脚本（v14，全部10字段自动化）
├── references/
│   ├── create-story-form.md        # 表单字段详细参考
│   ├── field-config.md             # 字段key/选项值/角色key
│   └── mobx-store-architecture.md  # MobX 双 store 架构
└── cookies.json                    # 持久化的 Cookie
```
