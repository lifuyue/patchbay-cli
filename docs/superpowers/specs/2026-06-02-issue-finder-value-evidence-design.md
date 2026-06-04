# Issue Finder Value Evidence Design

## Summary

Issue Finder should strengthen its core advantage as a local-first task preparation CLI by producing high-value, evidence-backed issue recommendations and handoff payloads. The next design step upgrades Issue Finder from a lightweight `good first issue` ranker into a local decision engine that can explain why an issue is worth a developer's time and why an agent can act on it.

The first priority is:

- **Selection value:** find high-upside issues, not just easy issues.
- **Execution value:** give downstream agents a context evidence package they can verify and use.

LLM support remains optional. It may review and summarize evidence for display, but it must not decide ranking, scores, or daily selection.

## Goals

- Rank issues by high-value opportunity, with balanced support for established high-impact repositories and high-growth repositories.
- Enrich candidate issues with GitHub metadata before final local scoring.
- Produce structured value signals with explicit evidence references.
- Add a `value_assessment` and `evidence_pack` to `handoff.json`.
- Keep the core scoring deterministic, local, and explainable.
- Keep the first implementation within Issue Finder's safety boundary: no target repo code edits, no validation command execution, no commits, pushes, or pull requests.

## Non-Goals

- Do not scrape the GitHub Trending web page in the first version.
- Do not claim that `growth_momentum` is identical to GitHub Trending.
- Do not let the LLM alter value scores, recommendations, or daily selection.
- Do not build a full agent adapter or coding agent.
- Do not add remote artifact publishing.

## Product Positioning

Issue Finder's competitive edge should be a local app logic layer that returns high-value information. A good handoff should answer:

- Why is this issue worth doing?
- Why is this repository worth attention?
- Is the opportunity from established impact, growth momentum, or both?
- What facts support the recommendation?
- What risks or missing evidence could make the recommendation wrong?
- What context can an agent use immediately?

The CLI should remain conservative: it prepares and explains; it does not execute the contribution.

## Architecture

The enhancement adds a metadata enrichment and value evidence pipeline while preserving existing modules.

### `github_enrichment.rs`

Fetches and normalizes additional GitHub data for candidate issues. It does not score or judge candidates.

Inputs:

- `GitHubIssue` from existing discovery.
- Configured request budget and cache policy.

Outputs:

- `EnrichedIssue`
- partial data warnings
- source fetch metadata

Data sources:

- repository metadata
- recent stargazers sample
- newest forks sample
- issue comments
- issue labels and author association
- repository activity timestamps

### `value_signals.rs`

Turns enriched facts into local, explainable signals.

Each `ValueSignal` includes:

- `kind`
- `score_delta`
- `confidence`
- `summary`
- `evidence_refs`

Signals are factual interpretations, not final scores.

### `value_scoring.rs`

Aggregates signals into final deterministic scores:

- `value_score`
- `execution_gate_score`
- `recommendation`
- `opportunity_type`
- `growth_confidence`

The main ranking uses `value_score`. The execution gate prevents high-impact but poorly actionable issues from being selected by `daily`.

### `evidence_pack.rs`

Builds the agent-facing evidence package for `handoff.json` and report rendering.

It groups information into:

- why this is high value
- why this is actionable
- risk factors
- missing evidence
- source references

### `llm_review.rs`

Performs optional review and display enhancement only.

The LLM may produce:

- review summary
- fact-check notes
- possible overclaims
- agent brief

It must not modify local scores, recommendations, or selection decisions.

## Data Model

### `EnrichedIssue`

`EnrichedIssue` is the normalized factual object after GitHub metadata enrichment.

Fields:

- `issue`
  - title
  - body
  - labels
  - comments count
  - updated timestamp
  - author association
- `repository`
  - stars
  - forks
  - subscribers when available
  - open issues
  - pushed timestamp
  - created timestamp
  - updated timestamp
  - default branch
  - archived flag
  - topics
- `activity`
  - recent issue activity
  - recent repo activity
  - maintainer recent response
- `participants`
  - issue author
  - commenters
  - maintainer commenters
- `comments`
  - bounded recent comment excerpts
  - author association
  - created timestamp
- `growth`
  - recent stargazer sample
  - newest fork sample
  - sample limits and confidence notes
- `source_fetched_at`

`EnrichedIssue` must not include recommendation language.

### `ValueSignal`

Example:

```json
{
  "kind": "maintainer_attention",
  "score_delta": 12,
  "confidence": "high",
  "summary": "A maintainer responded within the last 7 days.",
  "evidence_refs": ["issue:comments.3", "repo:pushed_at"]
}
```

Initial signal kinds:

- `established_impact`
- `growth_momentum`
- `repo_activity`
- `maintainer_attention`
- `issue_clarity`
- `contribution_window`
- `issue_fit`
- `execution_readiness`
- `staleness_risk`
- `noise_risk`

### `ValueAssessment`

`ValueAssessment` is the final deterministic local assessment.

Fields:

- `value_score`: 0-100
- `execution_gate_score`: 0-100
- `recommendation`: `strong_candidate | candidate | weak_candidate | avoid`
- `opportunity_type`: `established_project | growth_project | balanced | niche_but_actionable | low_signal`
- `growth_confidence`: `high | medium | low`
- `signals`
- `risks`
- `missing_evidence`
- `explanation`

Ranking uses `value_score`. `daily` should not auto-prepare candidates with `execution_gate_score < 40`, unless the user explicitly prepares one issue.

## Growth-Aware Value Model

High-value opportunities come from two balanced sources:

- **Established impact:** already visible repositories where a contribution is likely to be seen.
- **Growth momentum:** repositories that may not be huge yet but are gaining attention quickly.

The scoring model:

```text
value_score =
  established_impact_score
  + growth_momentum_score
  + maintainer_attention_score
  + contribution_window_score
  + issue_fit_score
  - risk_penalty
```

### Established Impact

Signals:

- stargazers count
- forks count
- subscribers count when available
- repository age
- topics and language fit
- open-source maturity indicators

### Growth Momentum

Issue Finder must use official GitHub API data to compute growth. It should not scrape GitHub Trending in the first version.

Signals:

- `recent_star_velocity_7d`
- `recent_star_velocity_14d`
- `recent_star_velocity_30d`
- `recent_fork_velocity_30d`
- recent push activity
- recent release activity when available
- attention-to-size ratio
- repository age adjustment

Evidence examples:

- `repo:stargazers.sample_recent_100`
- `repo:forks.sample_newest_100`
- `repo:pushed_at`
- `repo:created_at`

The growth calculation is approximate when sampling is capped. Issue Finder must report `growth_confidence` and caveats instead of presenting sampled growth as exact truth.

### Opportunity Types

- `established_project`: established impact is clearly high.
- `growth_project`: growth momentum is clearly high.
- `balanced`: established impact and growth momentum are both strong.
- `niche_but_actionable`: modest impact, but fit and execution readiness are strong.
- `low_signal`: value is unclear or evidence is insufficient.

## Command Behavior

### `issue-finder scout`

Default behavior should use balanced enrichment.

Flow:

1. Search GitHub for candidate good-first issues.
2. Apply existing lightweight scoring for rough ordering.
3. Enrich the rough Top 25-40 candidates with additional metadata.
4. Generate value signals and assessments.
5. Print ranked candidates with value score, recommendation, opportunity type, key evidence, and top risk.

The target runtime is 1-3 minutes.

Existing flags remain:

- `--limit`
- `--refresh`
- `--json`

Future optional flag:

- `--fast`: skip enrichment and use only lightweight scoring.

### `issue-finder daily`

Flow:

1. Run the enriched scout pipeline.
2. Select Top N candidates with `recommendation != avoid` and `execution_gate_score >= 40`.
3. Prepare each selected issue.
4. Add repo scan evidence to `execution_readiness` and `evidence_pack`.
5. Write handoff files and daily report.

Daily report should become a high-value opportunity report:

- top recommendations
- value score
- opportunity type
- why it is worth doing
- biggest risk
- missing evidence
- handoff paths

Single-issue failures continue to be recorded without stopping the full daily run.

### `issue-finder prepare owner/repo#123`

Explicit prepare should still work even when a candidate has a low execution gate. In that case, Issue Finder writes the risk clearly into the handoff warnings.

Flow:

1. Fetch issue details.
2. Run enrichment.
3. Generate value assessment.
4. Prepare workspace and scan repository.
5. Generate final evidence pack.
6. Write handoff and inbox entry.

## Handoff Output

`handoff.json` should include new top-level fields:

```json
{
  "value_assessment": {
    "value_score": 84,
    "execution_gate_score": 72,
    "opportunity_type": "growth_project",
    "recommendation": "strong_candidate",
    "growth_confidence": "medium",
    "signals": []
  },
  "evidence_pack": {
    "why_this_is_high_value": [],
    "why_this_is_actionable": [],
    "risk_factors": [],
    "missing_evidence": [],
    "source_refs": []
  }
}
```

`source_refs` must be used whenever possible. If a conclusion cannot cite a source reference, it should be a warning or omitted.

## LLM Review

LLM review is optional and display-only.

Input:

- `value_assessment`
- `evidence_pack`
- bounded issue context

Output:

- `review_summary`
- `fact_check_notes`
- `possible_overclaims`
- `agent_brief`
- `warnings`

Rules:

- LLM output cannot modify scores.
- LLM output cannot modify recommendations.
- LLM output cannot decide daily selection.
- LLM review must cite existing `source_refs` where practical.
- If the LLM cannot ground a statement in evidence, Issue Finder records a warning.

## Caching and Request Budget

Issue Finder should preserve the 1-3 minute target by bounding enrichment.

Cache files:

- `cache/github-issues.json`: search candidates, TTL 10 minutes.
- `cache/enrichment/<owner__repo__issue>.json`: enriched issue data, TTL 30-60 minutes.

Request limits:

- Search returns roughly 80-120 candidates.
- Full enrichment runs on rough Top 25-40.
- Recent stargazer sample is capped at 100.
- Newest fork sample is capped at 100.
- Issue comments are capped at 30-50 recent comments.

`--refresh` bypasses both search and enrichment caches.

## Error Handling

- If enrichment for one issue fails, continue with partial evidence and lower confidence.
- If star or fork samples are insufficient, keep the candidate and mark growth confidence as low or medium.
- If GitHub rate limits occur, show a clear rate-limit error and use valid cache when available.
- If LLM review fails, set `llm_review.status = "failed"` and continue.
- If repo scan fails, keep GitHub metadata evidence and lower execution readiness.
- If handoff writes fail for one issue during daily, record the issue as failed and continue the daily run.

## Testing Strategy

Unit tests:

- star velocity bucket calculation
- fork velocity proxy calculation
- opportunity type classification
- value score aggregation
- execution gate threshold
- evidence refs de-duplication
- evidence ref completeness checks

Integration tests:

- mocked GitHub enrichment responses
- partial enrichment failure still produces assessment
- daily skips low execution gate candidates
- explicit prepare writes low-gate warnings
- handoff includes `value_assessment`
- handoff includes `evidence_pack`
- LLM review cannot affect score or recommendation
- cache is used unless `--refresh` is passed

## Acceptance Criteria

The feature is complete when:

- `issue-finder scout` can display enriched, evidence-backed value rankings.
- `issue-finder daily` selects high-value candidates using value score and execution gate.
- `handoff.json` includes deterministic `value_assessment`.
- `handoff.json` includes an agent-usable `evidence_pack`.
- Growth opportunities are supported through official GitHub API-derived `growth_momentum`.
- LLM review is display-only and cannot affect deterministic selection.
- Tests cover scoring, evidence, enrichment failure, daily selection, and handoff output.
