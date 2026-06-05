# Trusted Discovery Recall 设计

日期：2026-06-05

状态：已实现

## 摘要

当前 `issue-finder.scout` 的结果过少，主要原因不是质量过滤太严，而是 discovery 候选池过窄：只搜索两个 `good first issue` label、每个 label 取 50 条、按 GitHub `updated` 全局排序，然后最多富化 40 个候选。热门噪声、刚更新的低质量任务和单一全局搜索顺序会过早消耗 enrichment 预算。

本设计采用激进 C 方案：把 discovery 重构为多来源、多 lane、强 trusted 权重的召回系统。Good First Issue 维护的仓库列表作为强 trusted 数据源，我们自己的 trusted overlay 作为补充，全局搜索作为探索层。系统扩大候选池和 enrichment 预算，但不降低质量过滤，不绕过 `prepare_gate.rs`。

## 目标

- 用 Good First Issue 的仓库池作为强 trusted source，在合理 API 预算内保证主要召回权重。
- 保留全局搜索，发现 GFI 仓库池没有覆盖的新项目和漏网候选。
- 新增本项目自己的 trusted overlay repo pool，用 live scout 发现的高质量项目补齐 GFI 盲区。
- 让 discovery 输出带来源证据，支持 lane-aware rough rank、dedupe、预算约束和 debug。
- 把 enrichment 从固定前 25-40 个候选改为自适应批次推进，最多评估更大的候选池。
- 保持质量策略严格：已认领、已有 PR、fixed、docs-only、bounty、campaign、低深度任务仍隐藏或强降权。
- 用 mock 测试固定 GFI/overlay/global 预算行为，用 live workflow 读取 issue 正文和评论评估真实质量。

## 非目标

- 不把 Good First Issue 的结果直接当作最终推荐。
- 不在 scout 主路径实时拉取 Good First Issue 仓库列表。
- 不每次全量扫描 GFI 的 800 多个仓库。
- 不用更宽松的过滤规则凑满 `limit=10`。
- 不让 trusted source 绕过 `ValueAssessment`、`RecommendationAssessment` 或 `prepare_gate.rs`。
- 不依赖 LLM 做 discovery、ranking 或 gate 决策。
- 不修改目标仓库、创建 PR、提交目标仓库代码或安装目标仓库依赖。

## 当前问题

当前 discovery 入口在 `src/github.rs`：

```text
DISCOVERY_LABELS = ["good first issue", "good-first-issue"]
SEARCH_PER_LABEL_LIMIT = 50
sort = updated desc
```

推荐引擎随后在 `src/recommendation/engine.rs` 中执行：

```text
rank_candidates(...)
truncate(limit.clamp(25, 40))
enrich candidates concurrently
apply value assessment, quality policy and feed rank
```

这个结构有三个瓶颈：

1. 候选来源过窄，只覆盖两个 label。
2. 全局 `updated` 顺序会被噪声 repo 和刚更新的低质量 issue 占用。
3. 严格质量过滤后，如果前 25-40 个候选质量不足，scout 不会继续探索后面的候选。

## 参考 Good First Issue 的点

DeepSourceCorp/good-first-issue 的核心实现是：

- 人工维护 `data/repositories.toml` 仓库列表。
- 定时任务每天生成数据。
- 对每个仓库查询 beginner-friendly labels。
- 跳过 archived repo 和 90 天以上无 push 的 repo。
- 按仓库展示，每个仓库保留一组 issue。

它的价值在于项目池，而不是最终 ranking。我们借鉴：

- trusted repo pool；
- 多 beginner label；
- repo-scoped 查询；
- repo 活跃度过滤；
- 低并发、可控 API 预算。

我们不照搬：

- 不只按仓库展示；
- 不只看 title、label 和 comment count；
- 不忽略 issue body、评论、关联 PR、认领状态和质量策略；
- 不把它的仓库池当作唯一来源。

Good First Issue 仓库使用 MIT license。复制或 vendor 它维护的仓库列表时，必须保留 license attribution。

## 数据源

默认使用 vendored snapshot，保证 scout 可复现、可测试且不依赖远程文件即时变化。

```text
data/discovery/
  good-first-issue-repositories.toml
  trusted-overlay-repositories.toml
  LICENSE.good-first-issue
```

职责：

- `good-first-issue-repositories.toml`：从 Good First Issue 同步来的 repo list，作为 GFI trusted pool。
- `trusted-overlay-repositories.toml`：本项目维护的高质量补充 repo pool。
- `LICENSE.good-first-issue`：保留 Good First Issue 的 MIT license attribution。

第一版不在 `scout` 中动态拉取远程 `repositories.toml`。后续可以单独设计命令：

```bash
issue-finder discovery refresh-trusted-repos
```

该命令只更新 snapshot，不参与每次 scout 主路径。

## Discovery Lane 架构

新增 discovery 子系统，输出带来源证据的候选，而不是裸 `Vec<GitHubIssue>`。

```rust
pub struct DiscoveryCandidate {
    pub issue: GitHubIssue,
    pub source_lanes: Vec<DiscoveryLaneId>,
    pub trust_tier: RepoTrustTier,
    pub matched_labels: Vec<String>,
    pub rough_score: i32,
}

pub enum RepoTrustTier {
    OverlayTrusted,
    GfiTrusted,
    Global,
}
```

推荐流程改为：

```text
load trusted overlay repos
load vendored GFI repos
run trusted overlay lanes
run GFI trusted lanes
run global search lanes
merge and dedupe candidates
lane-aware rough rank
adaptive enrichment batches
value assessment
quality policy
feed ranking
select display candidates
optional exposure recording
```

## Lane 组成

### Trusted Overlay Lanes

使用本项目维护的高价值 repo 列表，优先级最高。首批 overlay 应至少包含此前 live/read 验证过的项目：

```text
bytecodealliance/jco
microsoft/markitdown
httpie/cli
```

后续可以加入 live scout 证明稳定产出高质量 issue 的 repo。

每个 overlay repo 使用 repo-scoped beginner query，最多保留 8 个候选。Overlay 候选进入 enrichment 的预算权重最高，但最终仍必须通过质量过滤。

### GFI Trusted Repo Lanes

从 vendored GFI repo snapshot 选择一批 repo 查询。它是主 trusted source，必须在合理 API 资源下体现强权重。

第一版不全扫 800 多个 repo，而是做 profile-aware preselect：

- repo name、owner、description、language 命中用户 profile 优先；
- repo stars、recent activity、known productive source 优先；
- 最近 scout 产出过高质量候选的 repo 优先；
- 其他 GFI repo 做 rotation，避免长期只扫头部项目；
- 低产出或长期无候选 repo 降频，不永久删除。

GFI repo 查询覆盖 beginner-friendly labels：

```text
good first issue
good-first-issue
beginner
beginner-friendly
easy
starter
help wanted
low-hanging-fruit
```

每个 GFI repo 最多保留 4 个候选，避免大 repo 垄断。

### Global Search Lanes

全局搜索作为探索层，保留但收紧。它用于发现 GFI 和 overlay 没覆盖的新项目。

全局 lane 示例：

```text
label:"good first issue" profile terms
label:"good-first-issue" profile terms
label:beginner profile terms actionable terms
label:easy profile terms bug/repro/expected/actual
label:"help wanted" profile terms bug/repro/expected/actual
```

全局 `help wanted` 噪声较大，必须同时命中 profile 和 actionable 信号才应进入较高 rough rank。

## API 预算与强 Trusted 权重

第一版 enrichment 预算：

```text
max_enrichment_budget = 100
batch_size = 25
concurrency = 4
```

候选来源预算：

```text
trusted overlay: up to 20%
GFI trusted:     at least 50% when enough candidates exist
global search:   up to 30%
```

规则：

- GFI trusted 是硬预算约束，不是软加分。
- 如果 overlay 不满 20%，剩余额度优先给 GFI。
- 如果 GFI 有足够候选，global 不能抢走 GFI 的 50% enrichment 保留名额。
- 只有当 GFI 候选耗尽、API budget 用完或 rate limit 触发时，global 才能 backfill GFI 保留名额。
- 最终展示不强制 50% 来自 GFI，因为质量过滤仍然优先。
- 进入 enrichment 的候选必须体现 trusted 70%、global 30% 的设计权重。
- 当前 `issue-finder.scout` 输出 contract 不需要为了第一版强制新增顶层字段；source lane 和 budget fallback 可以先通过 candidate explanation 或内部 debug summary 暴露，再在后续 tool contract spec 中决定是否结构化输出。

合理 discovery API 预算：

```text
overlay repo issue listing: 3 repos, stop after 8 candidates per repo
GFI repo issue listing:     30 repos, stop after 4 candidates per repo
global search:              up to 20 search requests
```

Trusted repo lanes use the repository issues endpoint instead of GitHub search so they do not consume the stricter search API quota. Each repo scans beginner labels in priority order and stops as soon as the repo candidate cap is reached. Global exploration remains on GitHub search and is capped separately.

## Dedupe 规则

候选合并 key：

```text
repo_full_name#issue_number
```

重复候选不丢来源，合并字段：

- `source_lanes` 取并集；
- `matched_labels` 取并集；
- `trust_tier` 取最高；
- `rough_score` 重新计算；
- `first_seen_source` 保留，用于 debug。

同一个 issue 同时被 GFI trusted lane 和 global lane 命中，应比只被 global 命中的 issue 更可靠。

## Lane-Aware Rough Rank

Rough rank 只决定谁先进入 enrichment，不代表最终推荐分。

```text
rough_score =
  trust_bonus
  + label_bonus
  + profile_match_bonus
  + actionable_text_bonus
  + freshness_bonus
  + repo_influence_hint
  - thin_task_penalty
  - broad_task_penalty
```

建议第一版权重：

```text
OverlayTrusted: +40
GfiTrusted:     +30
Global:          0

good first issue / good-first-issue: +18
beginner / beginner-friendly / easy: +12
help wanted: +6 in trusted repos, +0 global unless actionable
profile term in title: +18
profile term in repo/description/labels: +8
actionable terms: +15
body length >= 120: +6
updated <= 7d: +8
repo stars hint: 0..12

docs-only/thin wording: -20
translation/content/campaign/bounty wording: -30
```

这些权重只用于 enrichment selection。最终 feed score 仍由 value assessment、quality policy、freshness、feedback 和 feed rank 合成。

## Repo Diversity

在 enrichment selection 阶段加 repo quota：

```text
overlay repo: max 5 candidates per repo
GFI repo:     max 3 candidates per repo
global repo:  max 2 candidates per repo
```

最终展示继续使用已有 `PRIMARY_RESULTS_PER_REPO_LIMIT = 2`。这样大 repo 可以因 trusted 权重优先进入候选，但不会一次吃掉全部 enrichment 或展示预算。

## Adaptive Enrichment

当前固定截断前 25-40 个候选。新流程按批次推进：

```text
max_enrichment_budget = 100
batch_size = 25
target_displayable = limit
minimum_quality_floor = strict
```

步骤：

1. discovery 生成 overlay/GFI/global 候选池；
2. lane-aware rough rank 选择第一批 25 个；
3. 并发 enrichment；
4. 执行 value assessment、quality policy、feed rank；
5. 如果 displayable 达到 `limit`，停止；
6. 如果 displayable 不够，继续下一批；
7. 最多到 100 个 enrichment 或候选耗尽；
8. 不用低质量候选 backfill。

`includeFiltered=true` 只影响输出是否包含 filtered 项，不影响 enrichment 终止逻辑和质量判断。

## Rate Limit 与错误行为

Discovery 阶段遇到部分 lane 失败或 rate limit：

- 保留已获取候选；
- 不因某条 lane 失败导致 scout 全失败；
- 内部 debug summary 记录 lane 截断原因；
- 如果 GFI 候选不足，global 可以 backfill，但输出必须能看出这是 budget fallback。

Enrichment 阶段沿用现有语义：

- 单个候选 enrichment 失败则跳过；
- scout 可继续返回其他候选；
- 失败候选不写 exposure event；
- `recordExposure=false` 仍不写 shown events。

## Testing Workflow

自动化测试必须 mock GitHub，不依赖真实网络、真实 token 或真实用户状态。

新增或更新 mock 测试覆盖：

- GFI 有足够候选时，进入 enrichment 的 GFI 候选至少占 50%。
- Overlay/GFI/global 命中同一个 issue 时，dedupe 保留多来源。
- 单 repo 不会垄断 enrichment selection。
- Global `help wanted` 没有 actionable/profile 信号时不会压过 GFI trusted 候选。
- Adaptive enrichment 会继续第二批，直到 displayable 达标或达到 100。
- 质量过滤严格：claimed、fixed、docs-only、bounty、campaign 仍隐藏。
- `recordExposure=false` 不写 shown events。
- Discovery cache 不破坏 source lane evidence 和 budget accounting。

本地验证命令：

```bash
cargo fmt --all
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- tools list
```

## Live Quality Workflow

Live 验证只用于人工产品判断，不进入自动化测试。

推荐命令：

```bash
GITHUB_TOKEN=$(gh auth token) \
ISSUE_FINDER_HOME=/tmp/issue-finder-discovery-quality \
cargo run --quiet -- tools call issue-finder.scout \
  --arguments '{"limit":10,"refresh":true,"includeFiltered":false,"recordExposure":false}' \
  --call-id live_discovery_quality
```

解读规则：

- 必须完整阅读返回 issue 的 body、comments 和相关 PR 线索，再判断质量。
- `limit=10` 时，理想返回 8-10 个 displayable。
- 至少 5 个 displayable 应来自 GFI 或 overlay trusted source，除非 trusted 候选被质量策略正确过滤。
- 不允许已认领、已有 PR 修复、docs-only、bounty/campaign 进入可见结果。
- 如果可见结果仍少，但 hidden_quality 很多，优先修正 quality policy。
- 如果候选池耗尽，则继续扩展 trusted overlay 或 global lanes，不降低质量阈值。

建议用此前人工验证过的 issue 做 regression/benchmark seeds：

```text
aws/aws-cdk#37783
medusajs/medusa#15353
bytecodealliance/jco#1596
purrgrammer/fragua#37
httpie/cli#912
microsoft/markitdown#23
```

这些 seed 不要求永远 open，也不要求每次 live scout 都返回；它们用于验证 discovery lane 是否能覆盖同类高质量目标，以及 thin task 是否被合理降权。

## Acceptance Criteria

- Scout discovery 使用 overlay trusted、GFI trusted、global search 三类来源。
- Good First Issue repo snapshot 被作为强 trusted source 使用，并保留 MIT license attribution。
- GFI 有足够候选时，进入 enrichment 的候选至少 50% 来自 GFI trusted lane。
- Trusted overlay 和 GFI 合计目标权重约 70%，global search 目标权重约 30%。
- Global search 仍能发现不在 GFI snapshot 中的高质量 repo。
- Dedupe 保留多来源证据，debug 输出可以解释候选来自哪些 lanes。
- Adaptive enrichment 最多评估 100 个候选，并在 displayable 达标后停止。
- Repo diversity 防止单 repo 垄断 enrichment 和展示。
- 质量过滤、反馈降权、freshness、prepare gate 行为不被 trusted source 绕过。
- Mock 测试固定预算、dedupe、adaptive enrichment、repo diversity 和 failure behavior。
- Live scout workflow 记录结果数量、来源比例、filtered count、质量判断和被过滤原因。
