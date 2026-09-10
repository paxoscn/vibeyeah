# 飞书项目 — 创建需求表单参考

## 用户偏好

- **业务线不需要填写**（用户明确告知 2026-07-22）
- 点击字段后等待 **1 秒** 即可（不需要 3-4 秒）
- 所属项目选择: "数字化中心-产品迭代项目（非年度规划）"
- 产品经理/开发负责人/测试负责人 均选: 莫日根

## URL 模式

- 创建页面：`https://project.feishu.cn/chapanda_project/story/create?parentUrl=%2Fchapanda_project%2Fstory%2Fhomepage`
- 需求列表：`https://project.feishu.cn/chapanda_project/story/homepage`
- 项目概览：`https://project.feishu.cn/chapanda_project/overview`

## 页面导航

需求列表页的 "New" 按钮（`button[class*="fake-create-button"]`）点击后导航到 `/story/create`，**不是弹窗**。

### 触发方式

```javascript
// 方式1: Playwright force click
await page.click('button[class*="fake-create-button"]', { force: true });

// 方式2: JS dispatchEvent 序列
await page.evaluate(() => {
    const btn = document.querySelector('button[class*="fake-create-button"]');
    if (btn) {
        btn.dispatchEvent(new MouseEvent('mousedown', {bubbles: true, cancelable: true, button: 0}));
        btn.dispatchEvent(new MouseEvent('mouseup', {bubbles: true, cancelable: true, button: 0}));
        btn.dispatchEvent(new MouseEvent('click', {bubbles: true, cancelable: true, button: 0}));
    }
});
```

### 前置操作

```javascript
// 关闭通知横幅（会遮挡点击）
document.querySelectorAll('[class*="notification-notice-icon-close"]').forEach(el => el.click());
```

## DOM 结构

### 表单字段识别

- 字段容器：`div[class*="CreateFormItem"]`
- Label：`label[class*="semi-form-field-label"]`
- **必填标记**：label 含 `semi-form-field-label-re` class
- 字段值区域：`div[class*="semi-form-field-main"]`

### 字段完整列表（从上到下）

| Y坐标 | 字段名 | 必填 | 类型 | 默认值 |
|-------|--------|------|------|--------|
| ~233 | 需求标题 | ✅ | 文本 | Empty |
| ~277 | 需求描述 | - | 富文本 | Empty（placeholder: "如：背景、目标、详细描述等"） |
| ~389 | 需求分类 | ✅ | 下拉 | Empty |
| ~433 | 优先级 | ✅ | 下拉 | "中" |
| ~477 | 业务线 | - | 下拉 | Empty |
| ~521 | 需求来源 | ✅ | 下拉 | Empty |
| ~565 | 期望上线日期 | ✅ | 日期 | Empty |
| ~609 | 标签 | - | 多选 | Empty |
| ~653 | 产品经理 | ✅ | 人员选择 | Empty |
| ~697 | 业务需求审批编号 | - | 文本 | Empty |
| ~741 | 子需求 | - | 多选关联 | Empty |
| ~785 | 业务需求MRD | - | 文件 | Empty |
| ~829 | 关联产品计划 | - | 下拉 | Empty |
| ~873 | 系统模块 | - | 下拉 | Empty |
| ~917 | 备注 | - | 文本 | Empty |
| ~961 | 是否关键需求 | - | 下拉 | "否" |
| ~1005 | 关键需求备注 | - | 文本 | Empty |
| ~1049 | 需求评估时效 | - | 文本 | Empty |
| ~1093 | 需求审批单据 | - | 文件 | Empty |
| ~1137 | 所属项目 | ✅ | 下拉 | 提示"父子需求尽量保持在一个项目" |
| ~1201 | 风险备注说明 | - | 文本 | Empty |
| ~1297 | 关注人 | - | 人员选择 | Empty |
| ~1341 | Chat group | - | 单选 | "Do not create" |

### Roles & Members 区域（~Y1385）

| 角色 | 必填 |
|------|------|
| 需求评审委员会 | - |
| 项目经理 | - |
| 产品经理 | ✅（有 `*` 星号） |
| 开发负责人 | ✅（有 `*` 星号） |
| 测试负责人 | ✅（有 `*` 星号） |
| 开发工程师 | - |
| 前端开发工程师 | - |
| UI负责人 | - |
| UI设计师 | - |
| 测试工程师 | - |
| 产品部负责人 | - |

### 底部操作区（~Y822，固定在页面中部）

- `Create Another` checkbox
- `Cancel` 按钮
- `Save Draft` 按钮
- `Create` 按钮（`button[class*="semi-button-content"]` text="Create"）

### 其他

- `Simplified mode` 开关（右上角）
- `Invalid Param` toast 可能在页面加载时出现（不影响操作）
- `Don't Show Again` checkbox 可关闭重复提示

## 已验证的自动化交互模式

### 需求标题（textarea）
```python
# 1. evaluate 找 clickable 元素坐标
coords = await page.evaluate("""() => {
    const item = [...document.querySelectorAll('[class*="CreateFormItem"]')]
        .find(i => i.querySelector('label')?.innerText.trim() === '需求标题');
    const clickable = item.querySelector('.meego-text.clickable, .meego-text');
    const r = clickable.getBoundingClientRect();
    return {cx: r.x + r.width/2, cy: r.y + r.height/2};
}""")
# 2. mouse.click 激活
await page.mouse.click(coords['cx'], coords['cy'])
await page.wait_for_timeout(2500)  # textarea 需要 2.5s 渲染
# 3. 找 textarea 坐标并点击
ta = # evaluate 找 textarea 坐标
await page.mouse.click(ta['cx'], ta['cy'])
await page.keyboard.type("标题文字", delay=30)
await page.mouse.click(800, 300)  # blur 确认
await page.wait_for_timeout(1500)
```

### 需求分类/需求来源（select / tree-select 单级）
```python
# 1. mouse.click main 区域
await page.mouse.click(mainCX, mainCY)
await page.wait_for_timeout(3000)  # 下拉需要 3s
# 2. evaluate 找选项坐标 + mouse.click
opt = await page.evaluate("""() => {
    const el = [...document.querySelectorAll('*')]
        .find(e => e.innerText.trim() === '产品需求' && e.getBoundingClientRect().y > 50);
    const r = el.getBoundingClientRect();
    return {cx: r.x + r.width/2, cy: r.y + r.height/2};
}""")
await page.mouse.click(opt['cx'], opt['cy'])
await page.wait_for_timeout(1500)
await page.mouse.click(350, 300)  # 关闭
```

### 业务线（tree-select 多级）
```python
# 1. 打开下拉
await page.mouse.click(mainCX, mainCY)
await page.wait_for_timeout(3500)
# 2. 找"运营中台"的箭头并点击展开
arrow = await page.evaluate("""() => {
    const el = [...document.querySelectorAll('*')]
        .find(e => e.innerText.trim() === '运营中台' && e.getBoundingClientRect().y > 50);
    const parent = el.parentElement;
    const svg = parent?.querySelector('svg, [class*="arrow"]');
    if (svg && svg.getBoundingClientRect().width < 30) {
        const r = svg.getBoundingClientRect();
        return {cx: r.x + r.width/2, cy: r.y + r.height/2};
    }
    const r = el.getBoundingClientRect();
    return {cx: r.x + r.width/2, cy: r.y + r.height/2};
}""")
await page.mouse.click(arrow['cx'], arrow['cy'])
await page.wait_for_timeout(4000)  # 子级展开需要 4s
# 3. 如果子选项不在视口，滚动下拉框
# 4. mouse.click 子选项
```

### 期望上线日期（date）
```python
await page.mouse.click(mainCX, mainCY)
await page.wait_for_timeout(3000)
# 找日期 input（placeholder 含 'yyyy'）
di = # evaluate 找 input 坐标
await page.mouse.click(di['cx'], di['cy'])
await page.keyboard.press("Control+a")
await page.keyboard.type("2026-08-01", delay=80)
await page.keyboard.press("Enter")
await page.wait_for_timeout(1500)
```

### 所属项目（workitem_related_select — 搜索型）
```python
await page.mouse.click(mainCX, mainCY)
await page.wait_for_timeout(4000)  # 搜索框需要 4s
inp = # evaluate 找 input（排除 'Enter target value'）
await page.mouse.click(inp['cx'], inp['cy'])
await page.keyboard.type("产品迭代", delay=100)
await page.wait_for_timeout(5000)  # 搜索结果需要 5s
target = # evaluate 找包含"产品迭代"+"非年度规划"的结果
await page.mouse.click(target['cx'], target['cy'])
await page.wait_for_timeout(3000)  # 选择后等 3s 让 store 更新
```

## 常见失败原因

| 失败现象 | 根因 | 修复 |
|----------|------|------|
| 下拉选项找不到 | 等待不够长 | 至少等 1s，tree-select 等 2-3s |
| 子选项找不到 | 未展开父级/未滚动 | 先点箭头展开，等 3s，必要时滚动下拉框 |
| 人员搜索框不出现 | 点击了错误元素 | 必须点击 `.meego-user-display` 或 `.meego-user-display-placeholder` |
| 搜索结果找不到 | 等待不够/搜索词不匹配 | 等 5s，用 `startsWith` 而非精确匹配 |
| 选择后表单仍报 required | MobX store 未更新 | **已知限制**: Roles 区域 person picker 视觉更新但 store 不更新 |
| 通知横幅遮挡 | 页面顶部 toast | 先关闭 `[class*="notification-notice-icon-close"]` |
| scrollIntoView 后坐标错位 | DOM 重新渲染 | scrollIntoView 后立即重新获取坐标 |
| Roles 中"产品经理"定位到主字段 | 表单 label 和 Roles span 同名 | 找第二个同名 span 或检查 y 坐标区分 |
| 所属项目点击无反应 | `miigo-work-item-related-select` 组件不响应外部事件 | **已知限制**: 所有点击方式（mouse.click, el.click, dblclick）均无效 |
| 前一步弹窗未关闭 | 遮挡后续操作 | 每步结束后 Escape×2 + mouse.click(350,300) + wait(1.5s) |

## API 端点（创建需求时触发）

| 方法 | 端点 | 说明 |
|------|------|------|
| POST | `/goapi/v5/work_item_types/batch` | 批量获取工作项类型 |
| POST | `/goapi/v5/workitem/model/field/multi_query` | 查询字段模型 |
| POST | `/goapi/v5/workitem/flowrole/query` | 查询流程角色 |
| GET | `/goapi/v1/businessline/list/{project_id}` | 业务线列表 |
| GET | `/goapi/v2/projects/{project_id}/work_item_types` | 工作项类型列表 |

项目 ID: `673da2055b6df89d4cecc1ce`
