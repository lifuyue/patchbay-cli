# Progressive Handoff Pack 设计

日期：2026-06-03

状态：已确认设计方向

## 摘要

Issue Finder 的核心定位保持不变：本地检索、计算和评估高价值 GitHub issue，准备 workspace，并生成可交给 coding agent 的 handoff。渐进式披露只服务于这个核心输出，目标是降低交给 agent 的初始上下文噪声，而不是把 Issue Finder 做成独立 agent、通用 tool registry 或 MCP host。

第一期采用 **Progressive Handoff Pack + Codex-local `issue-finder` skill**：

- `prepare` 和 `daily` 继续生成 canonical `handoff.json` 与人类可读 `handoff.md`。
- 每个 prepared inbox item 额外生成 `codex.md`、`context/*.md` 和 `.agents/skills/issue-finder/SKILL.md`。
- `codex.md` 是给 Codex 的最短入口；agent 初始只读入口和安全边界，再按任务阶段读取更详细的上下文文件。
- `issue-finder` skill 是 Codex 消费 Issue Finder handoff pack 的适配层，不执行 Issue Finder、不启动 agent、不替代 Codex。

## 产品边界

Issue Finder 负责：

- 本地发现和排序高价值 issue。
- 本地 enrichment、value scoring、evidence pack 生成。
- 准备本地 workspace、分支、候选文件和验证命令建议。
- 生成结构化 handoff 和渐进式上下文文件。

Issue Finder 不负责：

- 实现独立 agent runtime。
- 启动 Codex、Cursor、Claude Code 或其他 coding agent。
- 管理通用工具市场、MCP server、plugin install 或 tool execution。
- 在目标仓库安装依赖、提交、推送或创建 PR。

## 参考借鉴

从 `reference/openai-codex` 借鉴以下机制，但不复制其 agent runtime：

- `codex-rs/tools/src/tool_search.rs`：先暴露可搜索摘要，再用 `defer_loading` 延迟完整工具内容。
- `codex-rs/protocol/src/dynamic_tools.rs`：`defer_loading` 是显式字段，旧语义 `exposeToContext: false` 等价于延迟加载。
- `codex-rs/core/src/mcp_tool_exposure.rs`：工具数量超过阈值后不直接暴露，而转入 deferred discovery。
- `codex-rs/core-skills/src/render.rs`：skills 元数据有上下文预算，必要时缩短描述或省略高噪声内容。
- `codex-rs/core-skills/src/model.rs`：skill 有 metadata、scope、dependencies、policy 和 path，完整 `SKILL.md` 不是唯一入口。

Issue Finder 的对应实现应更轻：用文件边界和 manifest 控制披露，不引入交互式 `tool_search` runtime。

## 输出结构

每个 prepared inbox item 生成以下结构：

```text
inbox/<id>/
  handoff.json
  handoff.md
  codex.md
  context/
    entry.md
    value.md
    issue.md
    repo.md
    validation.md
    safety.md
  .agents/
    skills/
      issue-finder/
        SKILL.md
        refs.json
```

### `handoff.json`

`handoff.json` 仍是 canonical 机器可读输出。为避免破坏现有消费者，第一期不因为新增 pack 引用而提升 handoff 顶层 `version`；新增独立字段 `context_pack`，由它自己维护版本。

建议结构：

```json
{
  "context_pack": {
    "version": 1,
    "kind": "issue_finder_progressive_handoff_pack",
    "disclosure": "progressive",
    "entrypoint": "./codex.md",
    "context_dir": "./context",
    "skill": {
      "name": "issue-finder",
      "path": "./.agents/skills/issue-finder/SKILL.md"
    },
    "files": [
      {
        "id": "entry",
        "path": "./context/entry.md",
        "default_visible": true,
        "defer_loading": false
      },
      {
        "id": "value",
        "path": "./context/value.md",
        "default_visible": false,
        "defer_loading": true
      }
    ]
  }
}
```

`context_pack` 只描述本地文件入口和披露策略，不承载完整上下文正文。

### `codex.md`

`codex.md` 是用户最推荐交给 Codex 的入口文件。它应保持短小，包含：

- issue 标题、repo、编号、URL。
- workspace 绝对路径和分支。
- handoff pack 绝对路径。
- `issue-finder` skill 的绝对路径。
- 初始读取顺序：先读 `context/entry.md` 和 `context/safety.md`。
- 明确不要一次性读取全部 context 文件。

`codex.md` 可以同时包含相对路径和绝对路径。绝对路径用于从任意 cwd 调用 Codex 时仍能解析，相关文件中的相对路径用于 pack 自身可读性。

### `context/entry.md`

`entry.md` 是 agent 初始上下文导航，应包含：

- 本次任务的一句话目标。
- 为什么 Issue Finder 认为它适合进入 agent 工作流的极简摘要。
- 当前 workspace、分支、候选文件概览。
- 下一步读取建议。
- 不超过一屏的安全提醒。

它不应复制完整 issue body、完整 evidence、完整 repo scan 或全部验证命令。

### `context/value.md`

`value.md` 承载高价值判断细节：

- `value_score`、`execution_gate_score`、`recommendation`、`opportunity_type`。
- value signals、risks、missing evidence。
- `EvidencePack` 中的 high-value 和 actionable 证据。
- 解释为什么值得做、为什么适合 agent 做。

只有当 agent 需要判断优先级、解释选题或复核价值时才读取。

### `context/issue.md`

`issue.md` 承载 issue 原始上下文：

- 标题、URL、labels、更新时间。
- issue body。
- 关键 enrichment 摘要。
- 如果后续实现已采集 comment/maintainer signals，也放在这里。

它用于理解问题细节，不作为初始入口。

### `context/repo.md`

`repo.md` 承载 workspace 和 repo scan：

- workspace 路径、默认分支、Issue Finder 分支、dirty 状态。
- candidate files。
- scan warnings。
- repo description、topics、stars/forks/open issues 等必要仓库上下文。

agent 在规划代码修改前应读取它。

### `context/validation.md`

`validation.md` 承载建议验证方式：

- `repo_scan` 推断出的 validation commands。
- 每条命令的来源和适用原因。
- 如果没有检测到命令，给出保守说明，不凭空创造复杂验证流程。

Issue Finder 只建议命令，不自动执行命令。

### `context/safety.md`

`safety.md` 承载必须始终可见的边界：

- Issue Finder 不安装依赖、不提交、不推送、不创建 PR。
- agent 不应把 Issue Finder 生成文件当成目标 repo 源码修改对象。
- 若 workspace dirty，应先解释风险。
- 若验证命令需要网络、长耗时或破坏性操作，agent 应先说明并获得用户确认。

### `.agents/skills/issue-finder/SKILL.md`

生成的 skill 标题固定为：

```md
# issue-finder
```

它是 Codex 消费当前 handoff pack 的本地适配器。它应指示 Codex：

1. 用户提供 Issue Finder handoff 目录、`codex.md` 或 inbox item 时使用此 skill。
2. 先读 `context/entry.md` 和 `context/safety.md`。
3. 不要一次性读取所有 context 文件。
4. 需要评估价值时读 `context/value.md`。
5. 需要 issue 原文时读 `context/issue.md`。
6. 规划代码修改前读 `context/repo.md`。
7. 验证前读 `context/validation.md`。
8. 保持 Issue Finder 和 agent 的职责边界。

`SKILL.md` 不应复制完整 evidence 或 issue body；它只定义读取顺序和安全边界。

### `.agents/skills/issue-finder/refs.json`

`refs.json` 是 skill 的结构化引用索引，用于后续 Codex wrapper 或其他 agent wrapper 消费：

```json
{
  "version": 1,
  "skill": "issue-finder",
  "handoff_id": "<id>",
  "default_load": ["context/entry.md", "context/safety.md"],
  "deferred": [
    {
      "id": "value",
      "path": "context/value.md",
      "load_when": "Assessing why this issue is worth doing"
    },
    {
      "id": "repo",
      "path": "context/repo.md",
      "load_when": "Planning code changes"
    }
  ]
}
```

第一期不需要实现搜索 runtime；`refs.json` 只是稳定 manifest。

## 用户交付方式

推荐用户把 `codex.md` 交给 Codex，或在 Codex 中引用该文件的绝对路径。

`codex.md` 应明确写出：

```md
Use the local skill at:
<absolute path>/inbox/<id>/.agents/skills/issue-finder/SKILL.md

Start with:
<absolute path>/inbox/<id>/context/entry.md
<absolute path>/inbox/<id>/context/safety.md
```

这样即使 Codex 当前 cwd 是目标 workspace，而不是 Issue Finder inbox 目录，也能解析 handoff pack。

## 工作流影响

### `issue-finder prepare`

准备单个 issue 后：

1. 继续写 `handoff.json`、`handoff.md`。
2. 生成 progressive context files。
3. 生成 `codex.md`。
4. 生成 `.agents/skills/issue-finder/SKILL.md` 和 `refs.json`。
5. CLI 输出中保留 JSON/Markdown 路径，并新增 Codex 入口路径。

### `issue-finder daily`

对每个成功 prepared item 执行相同 pack 生成逻辑。日报可以列出 `codex.md` 路径，但不需要展开 context 内容。

### `issue-finder handoff`

保持现有行为：

- `--json` 输出 canonical `handoff.json`。
- `--print` 输出 `handoff.md`。

第一期不新增 `issue-finder agent` 子命令，也不要求 `handoff` 命令读取 progressive pack。后续如果需要，可以增加轻量 `--codex` 输出 `codex.md`，但不作为第一期范围。

## 失败模式

- 如果 pack 文件写入失败，当前 prepare 应视为失败；`daily` 记录该 issue 为 prepare failed 并继续后续任务。
- 写文件应沿用 Issue Finder 的 atomic write 边界，避免半写文件。
- 旧 inbox item 没有 `context_pack` 时，现有 `handoff` 行为不变。
- 如果 skill 文件缺失，`codex.md` 仍应让用户从 `context/entry.md` 开始；skill 缺失是降级，不应阻止用户读取 handoff。

## 测试计划

新增或扩展测试覆盖：

- `handoff.json` 包含 `context_pack` 引用，且不内嵌完整 context 正文。
- `prepare` 成功后写出 `codex.md`、`context/*.md`、`.agents/skills/issue-finder/SKILL.md` 和 `refs.json`。
- `codex.md` 包含 handoff pack、skill、entry、safety 的绝对路径。
- `entry.md` 不包含完整 issue body 或完整 evidence，保持 compact。
- `value.md` 包含 value assessment、signals、risks、missing evidence 和 evidence pack。
- `repo.md` 包含 workspace、branch、candidate files 和 scan warnings。
- `validation.md` 包含检测到的 validation commands；无命令时有保守说明。
- `safety.md` 包含 Issue Finder 不提交、不推送、不创建 PR 的边界。
- `.agents/skills/issue-finder/SKILL.md` 标题为 `# issue-finder`，并要求渐进读取。
- `daily` 对每个 prepared item 生成 pack，单个写入失败时仍延续现有失败隔离语义。

## 非目标

第一期不做：

- `issue-finder agent` 子命令。
- 通用 tool registry 或 tool search runtime。
- MCP server、plugin install、connector install。
- agent execution、agent orchestration 或多 agent 调度。
- 自动运行验证、安装依赖、提交、推送或创建 PR。
- 对 Cursor、Claude Code 等平台生成独立适配器；文件结构保持 portable，后续再扩展。

## 验收标准

- 用户运行 `issue-finder prepare owner/repo#123` 后，可以把生成的 `codex.md` 交给 Codex。
- Codex 初始只需要读取 `codex.md`、`context/entry.md` 和 `context/safety.md` 就能开始理解任务。
- 更详细的价值、issue、repo、validation 信息都可通过明确文件路径按需读取。
- Issue Finder 的核心竞争力仍聚焦在本地高价值 issue 计算和 handoff 生成，而不是 agent runtime。
