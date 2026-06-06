# Recommendation Quality Calibration Staged Roadmap

## Status

已批准设计，待按 stage 执行。

用户已确认：每个 stage 必须单独提交，并且每个 stage 都必须完成 offline recommendation eval、全量测试、clippy、6 组真实 profile 运行和人工读取 top candidates 后，才能进入下一 stage。

## Background

V1 已完成离线评测系统和 baseline，不修改生产 ranking、fallback 或 GitHub discovery 行为。当前后续优化必须解决上一版真实运行暴露的稳定问题：

- Rust/Go backend profile 在 `limit=15` 时只返回 1 个 visible candidate。
- DevOps/infra profile 在 `limit=15` 时只返回 4 个 visible candidate。
- 65 个真实 visible candidates 中，`profileFit < 70` 占 51%。
- 65 个真实 visible candidates 中，创建超过 1 年的 issue 占 63%，其中不少仍吃到高 freshness。
- 65 个真实 visible candidates 中，`competition_evidence_missing` 占 86%。
- `shown` 反馈只扣 8 分，真实回放中榜首仍保持 rank 1。
- `read` 反馈只扣 35 分，真实回放中默认和 Python profile 榜首仍在首屏。
- GFI trusted 方向正确，但 global fallback 和 weak-profile GFI candidates 仍会污染榜单。

这份 spec 将 V2 到 V6 一次性定义清楚。每个 stage 都可以激进重构，但不能跳过评测和真实回放。

## Stage Contract

每个 stage 都必须遵守以下合同：

1. 在当前 stage 结束前，不开始下一 stage 的生产逻辑改动。
2. 每个 stage 使用独立 commit；如 stage 较大，可以多个聚焦 commit，但必须有一个 stage completion commit 或清晰提交边界。
3. 每个 stage 必须维护 `tests/fixtures/recommendation_eval/`，或者在报告中说明为什么无需新增样本。
4. 每个 stage 必须生成 `docs/recommendation-evals/YYYY-MM-DD-<stage>/`。
5. 每个 stage 必须运行：

```text
cargo test --test recommendation_eval
cargo test
cargo clippy --all-targets -- -D warnings
```

6. 每个 stage 必须使用隔离 `ISSUE_FINDER_HOME` 跑固定 6 组真实 profile：

```text
limit = 15
refresh = true
recordExposure = false
```

7. 真实运行必须由执行者直接读取 top candidates 的 issue 正文和评论，并人工标注 `excellent`、`good`、`weak`、`reject`。
8. 代表性 live failure 必须回填 offline fixtures，防止下一版回归。

## Global Acceptance Target

全部 stage 完成后的最终目标：

```text
live visible candidates per profile >= 11 when limit=15
target live visible candidates per profile >= 12 when limit=15
top10 reject/noise/claimed/open PR = 0
top10 profileFit < 60 <= 1
top10 old high-freshness issue <= 1
top10 competition_evidence_missing < 20%
top5 manually labeled good_or_excellent >= 80%
```

## V1: Offline Evaluation System And Baseline

V1 已完成。它建立评测基础，不改变生产算法。

Completed deliverables:

- `tests/fixtures/recommendation_eval/`
- `tests/support/recommendation_eval.rs`
- `tests/recommendation_eval.rs`
- `docs/recommendation-evals/2026-06-06-baseline/`
- `AGENTS.md`、`tests/AGENTS.md`、`src/AGENTS.md` 的 recommendation eval 协作规则

Baseline signals:

```text
samples = 48
visible = 22
precision@5 = 0.81
precision@10 = 0.81
rejectLeakage = 2
profileMismatchLeakage = 1
rankingInversions = 5
```

## V2: Quality Filtering And Weight Calibration

### Goal

先修榜单质量，不追求更多召回。V2 结束时，明确坏结果不能出现在榜单前排，profile fit 和反馈冷却必须比仓库体量更重要。

### Production Scope

V2 允许修改：

- `src/recommendation/quality_policy.rs`
- `src/recommendation/freshness.rs`
- `src/recommendation/feedback.rs`
- `src/recommendation/feed_ranker.rs`
- `src/value_scores.rs`
- `src/value_scoring.rs`
- 必要时修改 `src/recommendation/model.rs`

V2 不应实现 fallback C，不应增加 GitHub discovery API lane，不应改 competition completion 流程。

### Design

Quality policy 要更强硬：

- open PR、submitted PR、claimed、working 命中 `HiddenQuality`。
- dashboard、Renovate、dependency dashboard、toy/no-code 命中 `HiddenQuality`。
- `profileFit < 60` 默认 `HiddenQuality`，除非是明确的 fallback candidate 且后续 stage 有专门逻辑放行。V2 中不放行。
- `profileFit 60-69` 不允许进入 `HighValueReady` 或 `HighValueNeedsScoping` 前排，只能作为 lower visible 或 hidden。
- low-impact repo + weak validation path 继续强降权。

Freshness 要从“最近更新”改成“最近有价值活动”：

- 创建超过 1 年的 issue 默认 freshness cap。
- 旧 issue 只有在有维护者近期回复、新评论数量变化、或强执行证据时才能拿较高 freshness。
- stale weak-profile issue 不能因 repo activity 或 issue update 进入 top10。

Feedback cooldown 要从小 penalty 改成明确冷却：

- `shown` 后同一天不能保持 rank 1。
- `read` 后同一天应离开首屏。
- `prepared` 继续强降权。
- `done` 和 `dismissed` 继续 hidden。
- issue 有维护者新回复或实质更新后允许部分恢复，但恢复必须写入 explanation。

Value scoring 要修 profile fit 误判：

- 真正 profile-specific 的 excellent samples 不能被打到 30 分以下。
- profile term matching 应继续 token-aware，但要提高 title/body/repo topics/language 的组合表达能力。
- 不要让 generic repo description 单独制造高 profile fit。

### Offline Eval Requirements

V2 必须更新 fixtures，至少覆盖：

- global toy/no-code visible leakage。
- dashboard/dependency dashboard hidden。
- frontend excellent 被错误低 profile fit。
- backend Rust/Go excellent 被错误低 profile fit。
- Python profile excellent 被错误低 profile fit。
- shown/read cooldown replay。
- old stale high freshness。

V2 offline pass goals:

```text
rejectLeakage == 0
dashboardNoiseLeakage == 0
competitionLeakage == 0
profileMismatchLeakage <= 1
shown sample does not remain rank 1
read sample leaves first screen
```

### Live Eval Requirements

V2 live pass goals:

```text
top10 reject/noise/claimed/open PR = 0
top10 profileFit < 60 <= 1
top10 old high-freshness issue <= 1
manual top5 good_or_excellent >= 70%
```

V2 is allowed to keep visible count below 11 for weak profiles, because V3 owns recall.

## V3: Fallback C And Strong Trusted Recall

### Goal

解决 Rust/Go backend 和 DevOps/infra visible 不足，同时不扩大 weak global leakage。

### Production Scope

V3 允许修改：

- `src/discovery.rs`
- `src/github.rs`
- `src/recommendation/engine.rs`
- 必要时新增 `src/recommendation/fallback.rs` 或 `src/discovery_profile.rs`
- `data/discovery/*.toml`

V3 不应再次大幅调整 V2 的 quality weights，除非 live report 证明 fallback 需要小的 gate 适配。

### Design

Fallback C 的执行顺序：

1. 正常 trusted discovery、enrichment、ranking。
2. 如果 visible count 达到 `ceil(limit * 0.70)`，不触发 fallback。
3. 如果不足，触发 profile-specific trusted repos fallback。
4. 如果仍不足，触发 strong profile global query fallback。
5. fallback candidates enrich 后 merge、dedupe、rerank。
6. 所有 fallback candidates 必须经过 V2 quality policy。

Profile-specific trusted repos 来源：

- good-first-issue repo list 中按 language、topic、profile terms 预选。
- overlay trusted repos。
- 手动维护 profile buckets。

Profile buckets:

```text
default_cli_devtools: Rust, TypeScript, Python CLI and developer-tool repos
typescript_frontend: React, UI, browser, form, component repos
rust_backend_systems: Rust, Go, backend, compiler, cargo, service repos
python_data_cli: Python, data, pandas, testing, CLI repos
ai_agent_tools: AI, LLM, agent, eval, developer-tool repos
devops_infra: Kubernetes, Docker, CI, GitOps, cloud infra repos
```

Strong global fallback requirements:

```text
is:issue is:open
beginner/help-wanted style label
profile terms match title/body/repo/topics/language
exclude dashboard/renovate/no-code
recently updated or recently created
```

API budget:

```text
trusted repo fallback: max 20 repo issue-list requests
trusted repo fallback: max 3 candidates per repo
strong global fallback: max 8 search requests
strong global fallback: max 30 returned candidates total
fallback enrichment: max 40 additional candidates
```

### Offline Eval Requirements

V3 must add or update samples for:

- Rust/Go backend weak visible fill.
- DevOps/infra weak visible fill.
- strong global profile match accepted as fallback.
- weak global profile mismatch rejected.
- trusted repo fallback preferred over global fallback.

V3 offline pass goals:

```text
fallbackFillRate >= 0.70
rejectLeakage == 0
dashboardNoiseLeakage == 0
global weak leakage does not increase compared with V2
```

### Live Eval Requirements

V3 live pass goals:

```text
rust_backend_systems visible >= 11 when limit=15
devops_infra visible >= 11 when limit=15
all six profiles visible >= 11 when limit=15
target visible >= 12 for at least four profiles
top10 profileFit < 60 <= 1
```

## V4: Competition Evidence Completion

### Goal

降低真实榜单里的 `competition_evidence_missing`，并让 open PR、claimed、working、submitted PR 在最终可见前被重新过滤。

### Production Scope

V4 允许修改：

- `src/recommendation/engine.rs`
- `src/github_enrichment.rs`
- `src/competition.rs`
- 必要时新增 `src/recommendation/competition_completion.rs`

V4 不应新增 discovery lanes，也不应重新设计 profile scoring。

### Design

当前 enrichment 只对早期 candidates 拉 timeline。V4 要在初步 feed ranking 后执行 completion：

1. 初步 rank 所有已 enriched candidates。
2. 选取 top visible-ish 或 high-value candidates 中 missing timeline 的候选。
3. 最多补齐 `min(30, limit * 2)` 个候选的 competition timeline。
4. 补齐后重新计算 competition facts。
5. 重新运行 value assessment、quality policy 和 feed sort。
6. 输出 explanation，说明 timeline 是 completed 还是 skipped by budget。

Completion 只针对可能进入榜单的 candidates，不为明显 hidden candidates 花 API budget。

### Offline Eval Requirements

V4 must add or update samples for:

- missing timeline 的候选在 completion 后发现 open PR。
- missing timeline 的候选在 completion 后发现 no competition，保持 visible。
- claimed/working comments 和 timeline PR 同时存在时 hidden。
- completion budget 截断时 explanation 可解释。

V4 offline pass goals:

```text
competitionLeakage == 0
open PR / claimed / working hidden after completion
budgeted completion remains deterministic
```

### Live Eval Requirements

V4 live pass goals:

```text
top10 competition_evidence_missing < 20%
top10 open PR / claimed leakage = 0
visible count remains >= V3 hard pass for at least five profiles
```

## V5: Performance, Cache, And API Budget

### Goal

让 6 profile live matrix 可持续运行，避免 refresh 成本随 fallback 和 competition completion 无界增长。

### Production Scope

V5 允许修改：

- `src/github.rs`
- `src/github_enrichment.rs`
- `src/paths.rs`
- `src/recommendation/engine.rs`
- cache payload DTO 和 tests

V5 不应改变 ranking semantics，除非需要修正 cache 读写导致的错误行为。

### Design

Cache 要按 source 分层：

- discovery cache 分 lane：overlay、gfi trusted、global、fallback trusted、fallback global。
- enrichment cache 分 source：repo metadata、issue details、comments、timeline、growth。
- fallback cache 使用独立 TTL。
- competition completion 使用独立 budget 和 cache key。
- live report 可以复用 snapshot，但不能把 snapshot 作为自动测试网络依赖。

Runtime 要有可解释 budget：

- 每轮 scout 记录实际 search、repo list、issue details、comments、timeline、growth 请求数。
- report 输出 API budget usage。
- 当 budget 用尽时，candidate explanation 中说明对应 evidence missing reason。

### Offline Eval Requirements

V5 must add or update tests for:

- cache hit avoids network.
- stale cache refreshes.
- fallback cache has separate TTL.
- timeline completion cache is reused.
- budget exhaustion remains deterministic and explainable.

V5 pass goals:

```text
refresh=false < 5s on warm cache for fixed live matrix
refresh=true request count is bounded
budget exhaustion does not panic
ranking behavior remains equivalent when cache data is fresh
```

## V6: Evaluation Workflow Productization

### Goal

把当前 test-only eval/report 变成长期可执行的产品工作流，使后续算法版本不再依赖口头记忆或手工拼报告。

### Production Scope

V6 允许新增：

- `src/recommendation/eval.rs`
- CLI 子命令，例如 `issue-finder eval recommendation`
- report DTO
- tests for eval command contract

V6 不应改变 ranking semantics。

### Design

CLI workflow:

```text
issue-finder eval recommendation --offline
issue-finder eval recommendation --live --refresh --limit 15 --output docs/recommendation-evals/YYYY-MM-DD-<version>
```

Offline eval:

- 复用 `tests/fixtures/recommendation_eval/` 的数据模型，或将 evaluator 抽成 production-safe 模块。
- 不访问 GitHub 网络。
- 输出 `metrics.json`、`report.md`、`visible.jsonl`。

Live eval:

- 跑固定 6 组 profile。
- 使用隔离 `ISSUE_FINDER_HOME`。
- `recordExposure=false`。
- 输出 visible candidates、profileFit、visibility、risk tags、competition evidence state、source tier、manual review placeholders。
- 执行者必须读取 issue body/comments 后填入人工标注。

Regression:

- 对比上一版 report 的 visible count、top10 quality、profileFit leakage、competition missing、fallback leakage。
- 失败样本回填 fixture。

### Offline Eval Requirements

V6 pass goals:

```text
offline eval command writes stable metrics/report/visible files
live eval command can run without recording exposure
report output excludes tokens and generated cache directories
fixture failures can be traced to sample ids
```

## Reporting Format

Every stage report directory must contain:

```text
metrics.json
report.md
visible.jsonl
```

`report.md` must include:

1. Stage summary.
2. Production changes.
3. Offline metric diff against previous stage.
4. Live 6 profile table.
5. Manual top candidate review.
6. Failure examples.
7. Fixture additions.
8. Decision to proceed or repeat the stage.

`metrics.json` must include:

- offline dataset metrics.
- live profile metrics.
- visible count by profile.
- top10 leakage metrics.
- feedback replay metrics when relevant.
- API budget metrics from V5 onward.

`visible.jsonl` must contain compact rows only, without tokens, local cache paths, or private generated state.

## Execution Order

The approved execution order is:

```text
V2 Quality Filtering And Weight Calibration
V3 Fallback C And Strong Trusted Recall
V4 Competition Evidence Completion
V5 Performance, Cache, And API Budget
V6 Evaluation Workflow Productization
```

V2 is first because expanding recall before fixing quality would amplify weak global and weak-profile leakage. V3 follows after quality gates are reliable. V4 follows after recall because competition completion should operate on candidates likely to appear in the final feed. V5 follows after the expensive paths are known. V6 follows after the workflow has enough real stage reports to productize.

## Risks And Mitigations

Risk: V2 quality gates reduce visible count further.

Mitigation: V2 is allowed to fail visible count temporarily. V3 owns visible fill.

Risk: V3 fallback increases weak global leakage.

Mitigation: fallback candidates pass V2 quality policy, and global fallback requires strong profile match.

Risk: V4 increases API cost.

Mitigation: completion is limited to `min(30, limit * 2)` candidates and must use cache.

Risk: V5 cache changes hide stale evidence.

Mitigation: cache sources have separate TTLs and report budget/cache status explicitly.

Risk: V6 productization overcomplicates the CLI.

Mitigation: V6 only productizes the already-proven eval workflow and does not change ranking semantics.

