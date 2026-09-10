---
name: code-review
description: "对仓库分支进行全面 Code Review，检查业务逻辑、性能、安全等风险，生成结构化报告"
tags: [code-review, java, spring, mybatis, redis, kafka, security]
triggers:
  - "code review"
  - "代码审查"
  - "分支审查"
  - "review 分支"
  - "检查代码"
---

# Code Review — 仓库分支全面审查

## 触发条件

用户要求对某个仓库的分支进行 code review，或提到"代码审查"、"review"等关键词。

## 工作流程

### 第1步：信息收集

1. **确认仓库和分支**:
   ```bash
   cd /home/agent/<repo-name>
   git branch --show-current
   git fetch --all
   ```

2. **获取分支变更统计**:
   ```bash
   git log --oneline master..<branch>
   git diff master...<branch> --stat
   ```

3. **检查已有参考文档**:
   - 读取 `repo-references/<repo-name>.md`（项目总览）
   - 读取 `repo-references/<repo-name>/<branch-slug>.md`（分支理解文档）
   - 读取历史 review: `repo-references/<repo-name>/<branch-slug>-review-*.md`

4. **获取完整代码差异**:
   ```bash
   git diff master...<branch> -- "*.java" "*.xml" "*.yml" "*.yaml" "*.properties"
   ```
   注意：diff 可能很大，使用 delegate_task 并行读取关键文件。

### 第2步：深度代码审查

按以下 6 个维度逐一检查：

#### 2.1 业务逻辑错误和漏洞
- Spring `@Transactional` 自调用失效（`this.xxx()` 绕过 AOP 代理）
- 事务边界内包含远程调用（Feign/HTTP 超时导致长事务）
- 删除操作后未清理孤儿数据
- 并发竞态条件（分布式锁、乐观锁、幂等性）
- 返回值/计数不准确（saveBatch 返回值被忽略）
- 数据映射错误（putIfAbsent 静默丢弃、重复覆盖）

#### 2.2 服务/Redis/数据库性能冲击
- 单次请求内 DB 查询次数过多（N+1 问题）
- 事务持续时间过长（> 10秒）
- 连接池占用时间过长
- 全量加载到内存（无分页、无流式处理）
- SQL IN 子句过大（> 1000 个元素）
- 排序字段缺少索引
- Redis 大 Key 或慢查询

#### 2.3 Kafka 性能及数据倾斜
- 生产者消息体过大
- 消费者处理逻辑过重
- Partition Key 设计不合理导致数据倾斜
- 消费者组 rebalance 风险
- 消息格式变更的向后兼容性

#### 2.4 校验缺失
- 入参类型校验（`Long.parseLong` 无 try-catch）
- 文件上传校验（类型、大小、内容）
- SQL 参数 null 保护（`<if>` 标签缺失）
- 权限校验（brandId/doCenterCode 归属验证）
- 外部 API 响应校验（Feign 返回码检查）
- Excel 列索引与模板格式耦合

#### 2.5 安全风险
- Excel 公式注入（`=+-@` 开头未消毒）
- SQL 注入（动态 SQL 拼接）
- 敏感信息泄露（日志、审计记录、API 响应）
- 文件上传漏洞（任意文件类型/大小）
- 向后兼容性（新增必选参数导致旧调用方失败）
- Content-Disposition 文件名编码

#### 2.6 注意事项
- 废弃接口未移除
- 文件名含非法字符（Windows 不支持冒号）
- 魔法字符串硬编码
- 代码重复
- 日志级别不当

### 第3步：生成报告

报告结构：

```markdown
# Code Review: <repo> / <branch>

> **仓库**: <repo>
> **分支**: <branch>
> **Review 时间**: <timestamp>
> **Review 范围**: <N> commits, +X/-Y 行, Z 文件
> **核心变更**: <摘要>

## 问题总览

| 严重级别 | 数量 | 类别 |
|---------|------|------|
| 🔴 P0 Critical | N | ... |
| 🟠 P1 High | N | ... |
| 🟡 P2 Medium | N | ... |
| 🟢 P3 Low | N | ... |

## 一、业务逻辑错误和漏洞
## 二、服务/Redis/数据库性能冲击隐患
## 三、Kafka 性能及数据倾斜隐患
## 四、校验缺失
## 五、安全风险
## 六、注意事项
## 七、修复优先级建议
## 八、变更文件影响矩阵

---
*Review by Hermes Agent — <timestamp>*
```

每个问题的格式：
```markdown
### [级别] P[N]-[序号]: [问题标题]

**文件**: `xxx.java`
**位置**: [方法名/行号]

**问题**: [详细描述]

**影响**: [后果分析]

**修复建议**: [具体代码示例]
```

### 第4步：保存报告

```bash
# 文件名格式
repo-references/<repo-name>/<branch-slug>-review-<yyyyMMddHHmmss>.md

# branch-slug: 将 / 替换为 -
# 例: feature/user_resource_0624 → feature-user_resource_0624
```

### 第5步：返回结果

将完整报告内容返回给用户，同时告知文件保存路径。

## 严重级别定义

| 级别 | 定义 | 处理要求 |
|------|------|---------|
| 🔴 P0 | 数据丢失/损坏、安全漏洞、生产事故 | 必须修复后才能合并 |
| 🟠 P1 | 性能瓶颈、潜在数据错误、安全隐患 | 建议合并前修复 |
| 🟡 P2 | 代码质量问题、潜在风险、可维护性 | 后续迭代修复 |
| 🟢 P3 | 代码风格、命名规范、小优化 | 择机修复 |

## 参考知识来源

按优先级：
1. 分支的全部代码改动（`git diff`）
2. 仓库现有全部代码（当前分支 checkout）
3. `repo-references/<repo>.md`（项目总览文档）
4. `repo-references/<repo>/<branch>.md`（分支理解文档）
5. 历史 `repo-references/<repo>/<branch>-review-*.md`（前次 review）

## 注意事项

- 如果存在前次 review，需检查代码是否有变更，并在新 review 中标注"自上次 Review 以来是否有变更"
- 对于开发者已回复的问题，保留回复摘要和修订后级别
- 使用 `delegate_task` 并行读取多个大文件以提高效率
- diff 可能很大（> 100KB），注意截断处理，必要时分段读取
- 报告同时保存到文件并返回给用户
