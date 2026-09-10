# Meego MobX Store Architecture

## 双 Store 架构

飞书项目创建需求表单使用 **两个独立的 React state store**，挂载在同一 fiber 节点（depth=11）的不同 state hook 上：

### Store 1: Role Owners Store（stateIdx=0）

```javascript
// fiber.memoizedState → obj.current
{
  role_owners: [  // MobX observable Array (11 entries)
    { role_key: "role_673da2055b6df89d4cecc1ce_story_role_bdd99c", role: "role_bdd99c", owners: [] },
    { role_key: "..._PM", role: "PM", owners: [] },
    { role_key: "..._role_e119b1", role: "role_e119b1", owners: ["7361617598422827011"] }, // 产品经理
    { role_key: "..._role_520920", role: "role_520920", owners: [] },  // 开发负责人
    { role_key: "..._role_098722", role: "role_098722", owners: [] },  // 测试负责人
    { role_key: "..._role_85709a", role: "role_85709a", owners: [] },
    { role_key: "..._role_47a806", role: "role_47a806", owners: [] },
    { role_key: "..._role_39641a", role: "role_39641a", owners: ["7357898334184554497"] },
    { role_key: "..._UI", role: "UI", owners: [] },
    { role_key: "..._role_d5477b", role: "role_d5477b", owners: [] },
    { role_key: "..._role_a3ed1d", role: "role_a3ed1d", owners: [] },
  ]
}
```

- `role_owners` 是 **真正的 Array**（`Array.isArray()` 返回 true）
- 每个 entry 的 `owners` 是 **MobX observable Array** — 必须用 `.push()` 修改，不能用数组替换
- 视觉选人后所有角色（产品经理/开发负责人/测试负责人）的 owners 均自动更新（v13 验证）
- **关键**：必须通过搜索框选人（输入名字 → 等下拉 → 选第一个），不能只点击 placeholder

### Store 2: Field Store（stateIdx=5）

```javascript
{
  // 表单字段值
  name: "主数据需求0722001",
  field_112caf: "l15x6hx_2",          // 需求分类 (tree-select value)
  business: "",                        // 业务线 (cascadeSelect value) ⚠️ 视觉选择不更新
  field_3bff86: "数字化中心",           // 需求来源
  field_51b1e7: 1785513600000,         // 期望上线日期 (timestamp ms)
  priority: "2",                       // 优先级
  tags: [],
  field_a5b387: 6858517133,            // 所属项目 ID
  
  // 角色 — 只有产品经理有这个属性，开发/测试负责人没有
  role_673da2055b6df89d4cecc1ce_story_role_e119b1: ["7350114863919792131"], // 产品经理
  
  // 元数据
  _invalid_fields: {},
  _fields_without_permission_validate: null,
  role_owners: { validate: null },     // 注意：不是数组，是 {validate: null} 对象
  watchers: [],
  group_type: null,
}
```

- `business` 字段：视觉选择后 store 值仍为 `""`，必须直接注入：`store.business = '673e8ea4fb9a2754869103b8'`
- 角色属性：只有 `role_e119b1`（产品经理）存在于 field store 中。**但 Roles 不需要通过 field store 注入** — 通过搜索框选人 UI 交互即可同步更新两个 store（v13）
- 直接赋值 `store.role_xxx = [...]` 不会触发 MobX 响应式更新（属性未被 observe 注册）

## Store 定位方法

```javascript
// 从需求标题的 CreateFormItem 找到 fiber
const items = document.querySelectorAll('[class*="CreateFormItem"]');
for (const item of items) {
  const label = item.querySelector('label');
  if (label?.innerText.trim() === '需求标题') {
    const fk = Object.keys(item).find(k => k.startsWith('__reactFiber$'));
    let fiber = item[fk];
    
    // 向上遍历到 depth=11
    for (let i = 0; i < 30; i++) {
      let state = fiber.memoizedState;
      let stateIdx = 0;
      while (state) {
        const obj = state.memoizedState;
        if (obj?.current && typeof obj.current === 'object') {
          if (Array.isArray(obj.current.role_owners)) {
            // stateIdx=0 → Role Owners Store
          }
          if ('business' in obj.current) {
            // stateIdx=5 → Field Store
          }
        }
        state = state.next;
        stateIdx++;
      }
      fiber = fiber.return;
    }
  }
}
```

## onSubmit 函数

- 位置：fiberDepth=11 的 `memoizedProps.onSubmit`
- 调用方式：`onSubmit()` 直接调用
- 调用前需确保所有必填字段的 store 值已正确设置
- 验证在客户端执行，store 值不对会显示 form-field-error

## 链式 Roles 选择模式

⚠️ **已弃用**：v13 验证表明，每个 Roles 字段应**独立操作 + close_popups**，而非链式。

旧方式（链式）：选完一个 role 后直接点下一个 role 的 placeholder（触发 blur 保存）。
新方式（v13，独立）：每个 role 单独完成搜索→选人→close_popups(1500ms)，再进行下一个。

```python
for role_name in ['产品经理', '开发负责人', '测试负责人']:
    await fill_role(role_name, person_name)
    await close_popups()  # Escape × 2 + click empty area + wait(1500)
```

关键区别：独立模式确保每个 picker 的搜索结果完全加载，不会互相干扰。

## 已知值参考

| 字段 | store key | 值格式 | 示例 |
|------|-----------|--------|------|
| 需求分类 | `field_112caf` | string (option key) | `"l15x6hx_2"` |
| 业务线 | `business` | string (cascade value ID) | `"673e8ea4fb9a2754869103b8"` (基本档与主数据) |
| 需求来源 | `field_3bff86` | string (label) | `"数字化中心"` |
| 期望上线日期 | `field_51b1e7` | number (ms timestamp) | `1785513600000` |
| 优先级 | `priority` | string | `"2"` (中) |
| 所属项目 | `field_a5b387` | number (project ID) | `6858517133` |
| 产品经理 | `role_e119b1` | string[] (user IDs) | `["7350114863919792131"]` |

**莫日根 user ID**: `7350114863919792131`
