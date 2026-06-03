# 多轴推荐质量与评分模型重构设计

日期：2026-06-03

状态：设计已确认，待实现计划

## 摘要

Patchbay 的“高价值”推荐语义需要从单一 `value_score` 升级为多轴判断。新的产品定义中，社区关注代表贡献后的外部回报和可见度，是最高权重信号；可执行性代表 issue 是否适合现在交给 coding agent 做；风险标签负责解释活动噪音、低工程深度、模板化和 triage 成本。

本设计采用两步重构：

1. 先建立推荐质量标准和样本集，用真实搜索结果锁定产品判断边界。
2. 再完整重构核心评分模型，并迁移 CLI 输出、handoff、daily report、context pack 和测试到多轴评分语义。

目标不是让 Patchbay 变成 GitHub Trending，而是吸收 Trending 的核心思想：社区关注决定 worth doing；可执行性决定 can do now。

## 当前问题

当前搜索结果中，`lingdojo/kana-dojo` 的内容填充任务、`devtrack` 的移动端 bug、`xevrion-v2/agent-playground` 的 JSDoc 微任务、`commitpulse` 的批量测试 variation 都可能被解释成同一种 `strong_candidate`。这不是单纯排序错误，而是产品解释失真。

主要问题：

- `value_score` 把社区关注、执行清晰度、profile fit 和风险压缩成一个不透明分数。
- `execution_gate_score` 不能表达低工程深度和高关注并存的任务。
- `strong_candidate` / `candidate` 分类不足以区分高关注微贡献、agent-ready 工程任务和需要 triage 的活动噪音。
- `rs`、`ts`、`tsx` 等短 alias 通过 substring 匹配产生误判。
- 多个不同质量的 issue 容易被 clamp 到 `100`，排序分辨率不足。

## 产品语义

新的推荐判断拆成四个主轴：

- `attention_score`: 这个 issue 所属项目和上下文是否值得投入，代表社区关注、增长和外部可见度。
- `execution_score`: 这个 issue 是否适合现在交给 coding agent 做，代表问题清晰度、代码路径、复现步骤、验收标准和验证路径。
- `profile_fit_score`: 这个 issue 与用户配置的技术栈和偏好是否匹配。
- `risk_penalty`: 降权项，代表批量模板、活动噪音、过多 open issues、无代码任务、内容填充、任务过薄、maintainer 信号不足等风险。

最终排序分为：

```text
final_rank_score =
  attention_score * 0.55
  + execution_score * 0.30
  + profile_fit_score * 0.10
  - risk_penalty * 0.15
```

排序可以偏向 `attention_score`，但输出必须同时展示 `execution_score` 和风险标签，避免用户把所有高分任务理解成同一种高工程价值任务。

## 推荐分类

第一期使用五类推荐：

- `agent_ready_high_value`: 社区关注高，同时 issue 具备明确代码路径、复现、验收标准或验证方式。这是 Patchbay 最理想的推荐类型。
- `high_attention`: 社区关注高，但执行深度或清晰度一般。仍适合快速获得可见贡献、first contribution 或低成本 PR。
- `high_attention_low_depth`: 社区关注高，但任务本身工程深度低。例如 `No Code Required`、`under 60 seconds`、内容填充、JSDoc 注释、批量模板微任务。
- `niche_but_actionable`: 社区关注不高，但 issue 很清晰，适合 agent 做。
- `needs_triage`: 有明显噪音、模板化、批量活动、open issues 过多、maintainer 信号不足或范围过大的风险，需要人工判断。

分类规则不完全由 `final_rank_score` 决定，而由多轴组合和风险标签决定：

```text
attention high && execution high && risk_penalty < 30
=> agent_ready_high_value

attention high && low_depth_tag
=> high_attention_low_depth

attention high && risk_penalty >= 45
=> needs_triage

attention high
=> high_attention

execution high
=> niche_but_actionable

else
=> needs_triage
```

## 推荐质量标准和样本集

先建立 fixture-driven 推荐质量样本集，位置：

```text
tests/fixtures/recommendation_quality/
  samples.json
```

每条样本包含：

```json
{
  "id": "devtrack_export_button_loading",
  "issue": {},
  "enrichment": {},
  "expected": {
    "attention_band": "high",
    "execution_band": "high",
    "category": "agent_ready_high_value",
    "required_risk_tags": [],
    "forbidden_risk_tags": ["no_code_required", "micro_contribution"]
  }
}
```

### Attention 标准

- 高：stars、forks、recent stars、recent forks、recent push、recent issue activity、maintainer attention 任意组合强。
- 中：项目小但近期活跃，或 star/fork 有一定信号。
- 低：无社区关注、无增长、维护信号弱。

### Execution 标准

- 高：有复现步骤、expected/actual、明确文件路径、acceptance criteria、suggested fix、验证提示。
- 中：有清晰目标，但缺复现、具体文件或验证方式。
- 低：任务很薄、只说加注释/加内容、无代码、无 clone、浏览器内完成。

### Risk 标签

第一期实现这些风险标签：

```text
no_code_required
micro_contribution
content_fill
template_like
event_noise
thin_task
high_triage_load
missing_maintainer_signal
weak_validation_path
```

### 首批真实样本

- `lingdojo/kana-dojo#19364`: `high_attention_low_depth`; required tags: `no_code_required`, `micro_contribution`, `content_fill`
- `lingdojo/kana-dojo#19362`: `high_attention_low_depth`; required tags: `no_code_required`, `micro_contribution`, `content_fill`
- `Priyanshu-byte-coder/devtrack#1025`: `agent_ready_high_value`; high attention, high execution
- `Priyanshu-byte-coder/devtrack#1912`: `agent_ready_high_value`; high attention, high execution
- `Priyanshu-byte-coder/devtrack#1890`: `high_attention`; feature request with visible community context but weaker execution evidence than the bug samples
- `xevrion-v2/agent-playground#1`: `high_attention_low_depth`; required tags: `thin_task`, optionally `micro_contribution`
- `JhaSourav07/commitpulse#2691`: `needs_triage`; required tags: `template_like`, `event_noise`, `high_triage_load`
- `7shep/context-drift#4`: `niche_but_actionable`; high execution, low attention
- `alternative-down/ad-product-forge#5405`: `needs_triage`; large file count and audit scope should be treated as range risk

Fixture tests should assert:

- expected category matches
- attention and execution bands match
- required risk tags exist
- forbidden risk tags do not exist
- `agent_ready_high_value` always has high execution
- `high_attention_low_depth` always has high attention and at least one low-depth tag

## 核心评分模型

`ValueAssessment` becomes the canonical scoring model:

```rust
pub struct ValueAssessment {
    pub final_rank_score: i32,
    pub attention_score: i32,
    pub execution_score: i32,
    pub profile_fit_score: i32,
    pub risk_penalty: i32,

    pub recommendation_category: RecommendationCategory,
    pub attention_band: ScoreBand,
    pub execution_band: ScoreBand,

    pub signals: Vec<ValueSignal>,
    pub risk_tags: Vec<RiskTag>,
    pub missing_evidence: Vec<String>,
    pub explanation: Vec<String>,
}
```

Because this is a full semantic migration, old fields should be removed rather than kept as deprecated aliases:

- remove `value_score`
- remove `execution_gate_score`
- remove `recommendation`
- remove `opportunity_type`

### Module Boundaries

`value_signals.rs`:

- extracts typed signals
- extracts risk tags
- avoids broad substring matching for short aliases

`value_scoring.rs`:

- computes axis scores
- computes score bands
- computes `final_rank_score`
- assigns `recommendation_category`

`scoring.rs`:

- keeps only discovery coarse ranking, or is narrowed to preliminary attention/profile sorting before enrichment
- should not duplicate the final recommendation semantics

### Attention Score Signals

- repository stars and forks
- recent stars and forks
- recent push
- recent issue activity
- maintainer attention
- repository activity
- growth trend

### Execution Score Signals

- issue clarity
- file path references
- reproduction steps
- expected/actual behavior
- acceptance criteria
- suggested fix
- validation hints

`scout` does not have `repo_scan`, so first implementation should keep `execution_score` independent from workspace scan. `prepare` can include repo scan evidence in handoff/context, but should not need a second scoring pass yet.

### Profile Fit Score

Profile matching should use token-aware matching:

- exact normalized token or phrase matches for normal terms
- special handling for aliases like `ts`, `tsx`, `rs`, `js`, `jsx`
- no substring matching for aliases shorter than three characters

### Risk Penalty

Risk tags produce additive penalty:

- `no_code_required`: high penalty
- `micro_contribution`: medium/high penalty
- `content_fill`: medium/high penalty
- `template_like`: medium penalty
- `event_noise`: medium penalty
- `thin_task`: medium penalty
- `high_triage_load`: medium/high penalty
- `missing_maintainer_signal`: low/medium penalty
- `weak_validation_path`: low/medium penalty

Risk tags are not always filters. High-attention low-depth tasks should remain visible, but their category and tags must make the tradeoff explicit.

## Workflow Changes

### Scout

Text output should show:

```text
rank 92 | agent_ready_high_value | attention 88 | execution 84 | fit 70 | risk 10
tags: clear_repro, file_path, validation_hint
risks: none
```

For high-attention low-depth tasks:

```text
rank 89 | high_attention_low_depth | attention 96 | execution 28 | fit 40 | risk 45
risks: no_code_required, micro_contribution, content_fill
```

JSON output includes the full `ValueAssessment`.

### Prepare

Prepare output continues to show generated artifact paths:

```text
Prepared <id>
Category: high_attention_low_depth | attention 96 | execution 28 | risk 45
JSON: ...
Markdown: ...
Codex: ...
```

### Daily

The prepare gate changes from old `execution_gate_score < 40` to category/attention-aware logic:

```text
skip only when category == needs_triage && attention_score < 60
```

High-attention low-depth tasks may enter daily, but must be labeled as such.

Daily report groups prepared tasks by category:

```md
## Agent-Ready High Value Tasks
## High Attention Tasks
## High Attention, Low Depth
## Niche but Actionable
## Needs Triage
```

Each prepared item should include:

```text
rank 89 | attention 96 | execution 28 | risk 45 | category high_attention_low_depth
```

### Handoff JSON

`handoff.json` keeps `value_assessment` as canonical, with the new model:

```json
{
  "value_assessment": {
    "final_rank_score": 89,
    "attention_score": 96,
    "execution_score": 28,
    "profile_fit_score": 40,
    "risk_penalty": 45,
    "recommendation_category": "high_attention_low_depth",
    "attention_band": "high",
    "execution_band": "low",
    "signals": [],
    "risk_tags": ["no_code_required", "micro_contribution"],
    "missing_evidence": [],
    "explanation": []
  }
}
```

### Handoff Markdown

Top summary changes to:

```md
- Category: high_attention_low_depth
- Final rank score: 89
- Attention score: 96 (high)
- Execution score: 28 (low)
- Profile fit score: 40
- Risk penalty: 45
- Risk tags: no_code_required, micro_contribution, content_fill
```

### Progressive Context Pack

- `codex.md`: add category and short axis summary.
- `context/entry.md`: add category and short axis summary; keep it compact.
- `context/value.md`: keep filename for compatibility, but retitle content to `Recommendation Assessment`.
- `context/value.md`: split into attention, execution, profile fit, risk tags, and evidence sections.
- `context/safety.md`: unchanged.

### Evidence Pack

Rename evidence semantics to match the new product model:

```text
why_this_has_high_attention
why_this_is_agent_ready
risk_factors
missing_evidence
source_refs
```

This replaces:

```text
why_this_is_high_value
why_this_is_actionable
```

## Testing Plan

Update and add tests for:

- fixture-driven recommendation quality scenarios
- token-aware profile matching, especially short aliases
- attention score bands
- execution score bands
- risk tag detection
- final category assignment
- final rank formula behavior
- `render_ranked` output with category, axis scores, and risk tags
- `render_prepare_outcome` output with category summary and Codex path
- daily report category grouping
- handoff JSON no longer contains old scoring fields
- handoff Markdown shows multi-axis summary
- context pack `entry.md` and `value.md` show multi-axis recommendation assessment
- evidence pack uses high-attention and agent-ready terminology

## Non-Goals

This refactor does not add:

- GitHub Trending API integration
- LLM-based scoring decisions
- automatic validation execution
- automatic PR creation
- a second post-prepare scoring pass based on repo scan

## Acceptance Criteria

- The sample set classifies known recommendations according to the confirmed product rubric.
- `lingdojo/kana-dojo` style tasks remain visible but are labeled `high_attention_low_depth`.
- `devtrack` clear engineering bugs are labeled `agent_ready_high_value`.
- `commitpulse` template-like event tasks carry triage risk tags.
- `context-drift#4` style low-attention but clear CLI/docs tasks can appear as `niche_but_actionable`.
- CLI, daily report, handoff JSON, handoff Markdown, and progressive context pack all use the new multi-axis scoring language.
- Tests no longer depend on old `value_score`, `execution_gate_score`, `recommendation`, or `opportunity_type` semantics.
