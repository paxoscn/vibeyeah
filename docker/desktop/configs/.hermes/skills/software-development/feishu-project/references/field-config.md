# 创建需求 — 字段配置参考

## 项目信息

- **项目**: chapanda_project (国内专属云空间)
- **project_key**: `673da2055b6df89d4cecc1ce`
- **work_item_type_key**: `story`
- **用户**: 莫日根, LARK_USER_ID=`7350143703354343425`

## 字段 Key 映射

| 字段名 | field_key | type | 必填 |
|--------|-----------|------|------|
| 需求标题 | `name` | `_name` | ✅ |
| 需求分类 | `field_112caf` | `tree-select` | ✅ |
| 优先级 | `priority` | `select` | ✅ |
| 业务线 | `business` | `_business` (tree-select search) | ✅ |
| 需求来源 | `field_3bff86` | `select` | ✅ |
| 期望上线日期 | `field_51b1e7` | `date` | ✅ |
| 产品经理 | (person field) | person select | ✅ |
| 所属项目 | `field_a5b387` | `workitem_related_select` | ✅ |

## 选项值

### 优先级 (priority)
- `0` = 紧急
- `1` = 高
- `2` = 中 (默认)
- `99` = 低

### 需求分类 (field_112caf)
- `prui_7aiw` = 业务需求
- `b49p_9b5v` = 业务子需求
- `l15x6hx_2` = 产品需求
- `c2kbxox0j` = 技术需求

### 需求来源 (field_3bff86)
- `数字化中心` (默认)
- `本地生活中心`
- `运营管理中心`
- `供应链中心`
- `品牌市场中心`
- `境外事业部`
- `研发中心`
- `法务及内控中心`
- `财务共享中心`
- `人力资源中心`
- `综合事务中心`
- `公共事务中心`
- `战略运营部`
- `咖啡业务中心`
- `产研中心`

### 业务线 (business) — 树形结构
- `673da21b45c3ebf64f0129af` = 全域营销
  - `673da21b45c3ebf64f0129b0` = 渠道运营
  - `677782197e655a4946a66e89` = 小程序
  - `673da21b45c3ebf64f0129b2` = 会员中心
  - `67d3e7830c03beeb131e8105` = 营销中心
  - `673e8dcb5d344f85eb312d87` = 增长实验室
  - `67d3e915267e0eeb21ab972d` = 通用技术
- `6790ba162dc6eb657274abc5` = 运营中台
  - `673e8ea4fb9a2754869103b8` = 基本档与主数据 (**二级父节点，不可直接选择**)
    - 组织中心 (三级叶子)
    - 商品中心 (三级叶子)
    - 门店中心 (三级叶子)
    - 商户中心 (三级叶子)
    - MDM工具 (三级叶子)
  - `67d3eb57963caaf40b483992` = 订单中心
  - `67aeb1e51fe2f3247819a23d` = 外卖中心
  - `67d3ec0cbd34440721513477` = 配送中心
  - `67d3eba32f2e5e48dab73c91` = 支付中心
  - `67d3ebcb2f2e5e48dab73c92` = 结算中心
  - `67872ea50f907cbdb28004f6` = POS
  - `67d3ec3647eaa5f56a15dc40` = KDS
- 其他: 加盟商, 供应链, 数据中台, 财务与内部管理, 境外业务, AI中台

### Roles 角色 key

| 角色 | role_key |
|------|----------|
| 需求评审委员会 | `role_673da2055b6df89d4cecc1ce_story_role_bdd99c` |
| 项目经理 | `role_673da2055b6df89d4cecc1ce_story_PM` |
| 产品经理 | `role_673da2055b6df89d4cecc1ce_story_role_e119b1` |
| 开发负责人 | `role_673da2055b6df89d4cecc1ce_story_role_520920` |
| 测试负责人 | `role_673da2055b6df89d4cecc1ce_story_role_098722` |
| 开发工程师 | `role_673da2055b6df89d4cecc1ce_story_role_85709a` |
| 前端开发工程师 | `role_673da2055b6df89d4cecc1ce_story_role_47a806` |
| UI负责人 | `role_673da2055b6df89d4cecc1ce_story_role_39641a` |
| UI设计师 | `role_673da2055b6df89d4cecc1ce_story_UI` |
| 测试工程师 | `role_673da2055b6df89d4cecc1ce_story_role_d5477b` |
| 产品部负责人 | `role_673da2055b6df89d4cecc1ce_story_role_a3ed1d` |

## API 信息

### CSRF Token
- Cookie 名: `meego_csrf_token`
- Header 名: `x-meego-csrf-token` (⚠️ 不是标准的 `X-CSRF-Token`)

### 必需请求 Headers
```
x-meego-csrf-token: <csrf>
x-lark-gw: 1
x-meego-from: web
x-meego-source: web/release_train_...
x-meego-timezone: 28800/28800
x-meego-local: 28800
x-meego-scope: create
x-content-language: en
locale: en
```

### 内部 API 服务
`window.__defaultApiService.axiosInstance` 可以发请求，但 **只接受完整 URL**（不接受相对路径）。

### 创建 API 端点
Service Worker 拦截了所有 `/goapi/*` 请求的路由。直接 POST 创建 API 会返回 404。
已知端点（均需通过 Service Worker 路由）：
- `POST /goapi/v5/workitem/create`
- `POST /goapi/v2/projects/{pk}/work_item_types/story/workitems`
- `POST /goapi/v5/workitem/batch_create`

## 获取字段配置

页面加载时自动请求的 API（可从网络拦截中获取响应）：
- `POST /goapi/v3/settings/{pk}/story/fields` — 完整字段配置（870KB，含选项值）
- `POST /goapi/v3/settings/{pk}/story/fields/keys` — 字段 key 列表
- `POST /goapi/v5/workitem/flowrole/query` — 角色配置
- `POST /goapi/v5/user/profile` — 当前用户信息
