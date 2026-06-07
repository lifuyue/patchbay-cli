use anyhow::Result;
use futures::stream::{self, StreamExt};
use std::collections::{HashMap, HashSet};

use crate::config::Config;
use crate::discovery::{select_enrichment_candidates, DiscoveryCandidate};
use crate::github::{GitHubClient, GitHubIssue};
use crate::github_enrichment::{competition_timeline_missing, GitHubEnrichmentClient};
use crate::paths::IssueFinderPaths;
use crate::value_scoring::{assess_issue, RankedValueIssue};

use super::competition_completion::{self, CompetitionCompletionStatus};
use super::events::{record_event_for_issue, RecommendationEventSource, RecommendationEventType};
use super::feed_ranker::{apply_recommendation_assessments, displayable, sort_by_feed};
use super::state::load_state_map;

const ENRICHED_SCOUT_CANDIDATE_LIMIT: usize = 180;
const FALLBACK_ENRICHMENT_CANDIDATE_LIMIT: usize = 80;
const TRUSTED_FALLBACK_ENRICHMENT_CANDIDATE_LIMIT: usize = 40;
const ENRICHMENT_BATCH_SIZE: usize = 25;
const COMPETITION_TIMELINE_CANDIDATE_LIMIT: usize = 20;
const ENRICHMENT_CONCURRENCY_LIMIT: usize = 2;
const COMPETITION_COMPLETION_CONCURRENCY_LIMIT: usize = 2;
const POST_COMPLETION_TRUSTED_REFILL_LIMIT: usize = 32;
const POST_COMPLETION_GLOBAL_REFILL_LIMIT: usize = 32;
const PRIMARY_RESULTS_PER_REPO_LIMIT: usize = 2;
const COMPETITION_COMPLETED_RESULTS_PER_REPO_LIMIT: usize = 4;

#[derive(Debug, Clone, Copy)]
pub struct ScoutOptions {
    pub include_filtered: bool,
    pub record_exposure: bool,
    pub source: RecommendationEventSource,
}

impl ScoutOptions {
    pub fn cli() -> Self {
        Self {
            include_filtered: false,
            record_exposure: true,
            source: RecommendationEventSource::CliScout,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScoutResult {
    pub ranked: Vec<RankedValueIssue>,
    pub discovery_count: usize,
    pub filtered_count: usize,
}

struct AdditionalRankingRequest<'a> {
    enrichment: &'a GitHubEnrichmentClient,
    ranked: &'a mut Vec<RankedValueIssue>,
    discovery_by_key: &'a mut HashMap<String, DiscoveryCandidate>,
    candidates: Vec<DiscoveryCandidate>,
    refresh: bool,
    display_limit: usize,
    max_budget: usize,
    stop_visible_at: Option<usize>,
}

pub struct RecommendationEngine<'a> {
    paths: &'a IssueFinderPaths,
    config: &'a Config,
}

impl<'a> RecommendationEngine<'a> {
    pub fn new(paths: &'a IssueFinderPaths, config: &'a Config) -> Self {
        Self { paths, config }
    }

    pub async fn scout(
        &self,
        limit: usize,
        refresh: bool,
        options: ScoutOptions,
    ) -> Result<ScoutResult> {
        self.paths.ensure_layout()?;
        let github = GitHubClient::new(self.config)?;
        let enrichment = GitHubEnrichmentClient::new(self.config)?;
        let candidates = github
            .discover_candidates(self.paths, refresh, &self.config.profile)
            .await?;
        let discovery_count = candidates.len();
        let (mut ranked, mut discovery_by_key) = self
            .rank_discovered_candidates(&enrichment, candidates, limit, refresh)
            .await;
        let hard_pass = hard_pass_visible_count(limit);
        let fallback_target = fallback_target_visible_count(limit);
        let completion_prefill_target = completion_prefill_visible_count(limit);

        if display_count(&ranked, limit, false) < hard_pass {
            let mut fallback_enrichment_budget = FALLBACK_ENRICHMENT_CANDIDATE_LIMIT;
            let trusted_budget =
                TRUSTED_FALLBACK_ENRICHMENT_CANDIDATE_LIMIT.min(fallback_enrichment_budget);
            let fallback = github
                .discover_trusted_fallback_candidates(&self.config.profile)
                .await?;
            let consumed = self
                .rank_additional_candidates(AdditionalRankingRequest {
                    enrichment: &enrichment,
                    ranked: &mut ranked,
                    discovery_by_key: &mut discovery_by_key,
                    candidates: fallback,
                    refresh,
                    display_limit: limit,
                    max_budget: trusted_budget,
                    stop_visible_at: Some(fallback_target.max(completion_prefill_target)),
                })
                .await?;
            fallback_enrichment_budget = fallback_enrichment_budget.saturating_sub(consumed);

            if display_count(&ranked, limit, false) < hard_pass && fallback_enrichment_budget > 0 {
                let fallback = github
                    .discover_global_fallback_candidates(&self.config.profile)
                    .await?;
                self.rank_additional_candidates(AdditionalRankingRequest {
                    enrichment: &enrichment,
                    ranked: &mut ranked,
                    discovery_by_key: &mut discovery_by_key,
                    candidates: fallback,
                    refresh,
                    display_limit: limit,
                    max_budget: fallback_enrichment_budget,
                    stop_visible_at: Some(hard_pass.max(completion_prefill_target)),
                })
                .await?;
            }
        }

        let mut completion_statuses = self
            .complete_competition_evidence(&enrichment, &mut ranked, refresh, limit)
            .await;
        self.apply_feed_ranking(&mut ranked);
        append_discovery_reasons(&mut ranked, &discovery_by_key);
        competition_completion::append_completion_explanations(&mut ranked, &completion_statuses);

        if competition_limited_display_count(&ranked, limit, false) < hard_pass {
            let fallback = github
                .discover_trusted_fallback_candidates(&self.config.profile)
                .await?;
            self.rank_additional_candidates(AdditionalRankingRequest {
                enrichment: &enrichment,
                ranked: &mut ranked,
                discovery_by_key: &mut discovery_by_key,
                candidates: fallback,
                refresh,
                display_limit: limit,
                max_budget: POST_COMPLETION_TRUSTED_REFILL_LIMIT,
                stop_visible_at: Some(completion_prefill_target.max(fallback_target)),
            })
            .await?;
            completion_statuses.extend(
                self.complete_competition_evidence(&enrichment, &mut ranked, refresh, limit)
                    .await,
            );
            self.apply_feed_ranking(&mut ranked);
            append_discovery_reasons(&mut ranked, &discovery_by_key);
            competition_completion::append_completion_explanations(
                &mut ranked,
                &completion_statuses,
            );

            if competition_limited_display_count(&ranked, limit, false) < hard_pass {
                let fallback = github
                    .discover_global_fallback_candidates(&self.config.profile)
                    .await?;
                self.rank_additional_candidates(AdditionalRankingRequest {
                    enrichment: &enrichment,
                    ranked: &mut ranked,
                    discovery_by_key: &mut discovery_by_key,
                    candidates: fallback,
                    refresh,
                    display_limit: limit,
                    max_budget: POST_COMPLETION_GLOBAL_REFILL_LIMIT,
                    stop_visible_at: Some(completion_prefill_target.max(hard_pass)),
                })
                .await?;
                completion_statuses.extend(
                    self.complete_competition_evidence(&enrichment, &mut ranked, refresh, limit)
                        .await,
                );
                self.apply_feed_ranking(&mut ranked);
                append_discovery_reasons(&mut ranked, &discovery_by_key);
                competition_completion::append_completion_explanations(
                    &mut ranked,
                    &completion_statuses,
                );
            }
        }

        let filtered_count = ranked
            .iter()
            .filter(|item| !displayable(item, options.include_filtered))
            .count();
        let visible = competition_completion::select_display_candidates(
            ranked,
            limit,
            options.include_filtered,
            COMPETITION_COMPLETED_RESULTS_PER_REPO_LIMIT,
        );

        if options.record_exposure {
            self.record_exposure(&visible, options.source)?;
        }

        Ok(ScoutResult {
            ranked: visible,
            discovery_count,
            filtered_count,
        })
    }

    pub async fn daily_candidates(
        &self,
        refresh: bool,
        candidate_limit: usize,
    ) -> Result<ScoutResult> {
        self.scout(
            candidate_limit,
            refresh,
            ScoutOptions {
                include_filtered: false,
                record_exposure: false,
                source: RecommendationEventSource::Daily,
            },
        )
        .await
    }

    pub async fn assess_issue(
        &self,
        issue: GitHubIssue,
        refresh: bool,
        record_read: bool,
        source: RecommendationEventSource,
    ) -> Result<RankedValueIssue> {
        self.paths.ensure_layout()?;
        let enrichment = GitHubEnrichmentClient::new(self.config)?;
        let ranked = self
            .rank_single_issue(&enrichment, issue, refresh, true)
            .await?;
        if record_read {
            record_event_for_issue(
                self.paths,
                &ranked.issue,
                Some(&ranked.enriched_issue),
                RecommendationEventType::Read,
                source,
                serde_json::json!({}),
            )?;
        }
        Ok(ranked)
    }

    pub fn record_exposure(
        &self,
        ranked: &[RankedValueIssue],
        source: RecommendationEventSource,
    ) -> Result<()> {
        for item in ranked {
            record_event_for_issue(
                self.paths,
                &item.issue,
                Some(&item.enriched_issue),
                RecommendationEventType::Shown,
                source,
                serde_json::json!({
                    "finalFeedScore": item.recommendation.final_feed_score,
                    "baseCategory": item.recommendation.base_category.to_string()
                }),
            )?;
        }
        Ok(())
    }

    async fn rank_discovered_candidates(
        &self,
        enrichment: &GitHubEnrichmentClient,
        candidates: Vec<DiscoveryCandidate>,
        limit: usize,
        refresh: bool,
    ) -> (Vec<RankedValueIssue>, HashMap<String, DiscoveryCandidate>) {
        let selected = select_enrichment_candidates(candidates, ENRICHED_SCOUT_CANDIDATE_LIMIT);
        let discovery_by_key = selected
            .iter()
            .map(|candidate| (candidate_key(&candidate.issue), candidate.clone()))
            .collect::<HashMap<_, _>>();
        let mut ranked = Vec::new();

        for (batch_index, batch) in selected.chunks(ENRICHMENT_BATCH_SIZE).enumerate() {
            let ranked_batch = stream::iter(batch.iter().cloned().enumerate().map(
                |(index, candidate)| async move {
                    let absolute_index = batch_index * ENRICHMENT_BATCH_SIZE + index;
                    self.rank_single_issue(
                        enrichment,
                        candidate.issue,
                        refresh,
                        absolute_index < COMPETITION_TIMELINE_CANDIDATE_LIMIT,
                    )
                    .await
                    .ok()
                },
            ))
            .buffer_unordered(ENRICHMENT_CONCURRENCY_LIMIT)
            .filter_map(|item| async move { item })
            .collect::<Vec<_>>()
            .await;

            ranked.extend(ranked_batch);
            self.apply_feed_ranking(&mut ranked);
            append_discovery_reasons(&mut ranked, &discovery_by_key);

            if display_count(&ranked, limit, false) >= completion_prefill_visible_count(limit) {
                break;
            }
        }

        (ranked, discovery_by_key)
    }

    async fn rank_additional_candidates(
        &self,
        request: AdditionalRankingRequest<'_>,
    ) -> Result<usize> {
        let AdditionalRankingRequest {
            enrichment,
            ranked,
            discovery_by_key,
            candidates,
            refresh,
            display_limit,
            max_budget,
            stop_visible_at,
        } = request;

        if candidates.is_empty() || max_budget == 0 {
            return Ok(0);
        }

        let existing = ranked
            .iter()
            .map(|item| candidate_key(&item.issue))
            .collect::<HashSet<_>>();
        let candidates = candidates
            .into_iter()
            .filter(|candidate| !existing.contains(&candidate.key()))
            .collect::<Vec<_>>();
        let selected = select_enrichment_candidates(candidates, max_budget);
        if selected.is_empty() {
            return Ok(0);
        }

        for candidate in &selected {
            discovery_by_key.insert(candidate_key(&candidate.issue), candidate.clone());
        }

        let mut consumed = 0;
        for (batch_index, batch) in selected.chunks(ENRICHMENT_BATCH_SIZE).enumerate() {
            let ranked_batch = stream::iter(batch.iter().cloned().enumerate().map(
                |(index, candidate)| async move {
                    let absolute_index = batch_index * ENRICHMENT_BATCH_SIZE + index;
                    self.rank_single_issue(
                        enrichment,
                        candidate.issue,
                        refresh,
                        absolute_index < COMPETITION_TIMELINE_CANDIDATE_LIMIT,
                    )
                    .await
                    .ok()
                },
            ))
            .buffer_unordered(ENRICHMENT_CONCURRENCY_LIMIT)
            .filter_map(|item| async move { item })
            .collect::<Vec<_>>()
            .await;

            consumed += batch.len();
            ranked.extend(ranked_batch);
            self.apply_feed_ranking(ranked);
            append_discovery_reasons(ranked, discovery_by_key);

            if stop_visible_at
                .is_some_and(|target| display_count(ranked, display_limit, false) >= target)
            {
                break;
            }
        }

        Ok(consumed)
    }

    async fn complete_competition_evidence(
        &self,
        enrichment: &GitHubEnrichmentClient,
        ranked: &mut [RankedValueIssue],
        refresh: bool,
        limit: usize,
    ) -> HashMap<String, CompetitionCompletionStatus> {
        if limit == 0 {
            return HashMap::new();
        }

        self.apply_feed_ranking(ranked);
        let plan = competition_completion::plan_completion(ranked, limit);
        let mut statuses =
            competition_completion::annotate_skipped_by_budget(ranked, &plan.skipped_keys);

        if plan.complete_keys.is_empty() {
            return statuses;
        }

        let requested = plan.complete_keys.into_iter().collect::<HashSet<_>>();
        let requests = ranked
            .iter()
            .filter_map(|item| {
                let key = competition_completion::issue_key(item);
                requested
                    .contains(&key)
                    .then(|| (key, item.issue.clone(), item.enriched_issue.clone()))
            })
            .collect::<Vec<_>>();

        let completed = stream::iter(requests.into_iter().map(
            |(key, issue, current)| async move {
                let enriched = enrichment
                    .complete_competition_timeline(self.paths, &issue, &current, refresh)
                    .await;
                let status = if competition_timeline_missing(&enriched) {
                    CompetitionCompletionStatus::Failed
                } else {
                    CompetitionCompletionStatus::Completed
                };
                (key, enriched, status)
            },
        ))
        .buffer_unordered(COMPETITION_COMPLETION_CONCURRENCY_LIMIT)
        .collect::<Vec<_>>()
        .await;

        let completed_by_key = completed
            .into_iter()
            .map(|(key, enriched, status)| {
                statuses.insert(key.clone(), status);
                (key, enriched)
            })
            .collect::<HashMap<_, _>>();

        for item in ranked {
            let key = competition_completion::issue_key(item);
            let Some(enriched) = completed_by_key.get(&key) else {
                continue;
            };
            item.enriched_issue = enriched.clone();
            item.value_assessment = assess_issue(&item.enriched_issue, &self.config.profile);
            item.score = item.value_assessment.final_rank_score;
            item.explanation = item.value_assessment.explanation.clone();
        }

        statuses
    }

    async fn rank_single_issue(
        &self,
        enrichment: &GitHubEnrichmentClient,
        issue: GitHubIssue,
        refresh: bool,
        include_competition_timeline: bool,
    ) -> Result<RankedValueIssue> {
        let enriched = enrichment
            .enrich_issue_with_options(self.paths, &issue, refresh, include_competition_timeline)
            .await;
        let value_assessment = assess_issue(&enriched, &self.config.profile);
        let mut ranked = RankedValueIssue {
            issue,
            score: value_assessment.final_rank_score,
            value_assessment,
            enriched_issue: enriched,
            explanation: Vec::new(),
            recommendation: Default::default(),
        };
        ranked.explanation = ranked.value_assessment.explanation.clone();
        self.apply_feed_ranking(std::slice::from_mut(&mut ranked));
        Ok(ranked)
    }

    fn apply_feed_ranking(&self, ranked: &mut [RankedValueIssue]) {
        let states = load_state_map(self.paths).unwrap_or_default();
        apply_recommendation_assessments(ranked, &states);
        sort_by_feed(ranked);
    }
}

fn append_discovery_reasons(
    ranked: &mut [RankedValueIssue],
    discovery_by_key: &HashMap<String, DiscoveryCandidate>,
) {
    for item in ranked {
        let key = candidate_key(&item.issue);
        let Some(candidate) = discovery_by_key.get(&key) else {
            continue;
        };
        for reason in candidate.discovery_reasons() {
            if !item.explanation.contains(&reason) {
                item.explanation.push(reason);
            }
        }
    }
}

fn candidate_key(issue: &GitHubIssue) -> String {
    format!("{}#{}", issue.repo_full_name, issue.number)
}

fn display_count(ranked: &[RankedValueIssue], limit: usize, include_filtered: bool) -> usize {
    if limit == 0 {
        return 0;
    }

    let mut selected = 0;
    let mut repo_counts = HashMap::<&str, usize>::new();

    for item in ranked {
        if !displayable(item, include_filtered) {
            continue;
        }

        let repo = item.issue.repo_full_name.as_str();
        let count = *repo_counts.get(repo).unwrap_or(&0);
        if count < PRIMARY_RESULTS_PER_REPO_LIMIT {
            repo_counts.insert(repo, count + 1);
            selected += 1;
            if selected == limit {
                return selected;
            }
        }
    }

    selected
}

fn hard_pass_visible_count(limit: usize) -> usize {
    ceil_percent(limit, 70)
}

fn fallback_target_visible_count(limit: usize) -> usize {
    ceil_percent(limit, 80)
}

fn completion_prefill_visible_count(limit: usize) -> usize {
    limit.saturating_mul(2).min(limit.saturating_add(15))
}

fn ceil_percent(value: usize, percent: usize) -> usize {
    if value == 0 {
        return 0;
    }
    (value * percent).div_ceil(100)
}

fn competition_limited_display_count(
    ranked: &[RankedValueIssue],
    limit: usize,
    include_filtered: bool,
) -> usize {
    competition_completion::select_display_candidates(
        ranked.to_vec(),
        limit,
        include_filtered,
        COMPETITION_COMPLETED_RESULTS_PER_REPO_LIMIT,
    )
    .len()
}

pub fn select_display_candidates(
    ranked: Vec<RankedValueIssue>,
    limit: usize,
    include_filtered: bool,
) -> Vec<RankedValueIssue> {
    if limit == 0 {
        return Vec::new();
    }

    let mut selected = Vec::new();
    let mut repo_counts = HashMap::<String, usize>::new();

    for item in ranked {
        if !displayable(&item, include_filtered) {
            continue;
        }

        let repo = item.issue.repo_full_name.clone();
        let count = *repo_counts.get(&repo).unwrap_or(&0);
        if count < PRIMARY_RESULTS_PER_REPO_LIMIT {
            repo_counts.insert(repo, count + 1);
            selected.push(item);
            if selected.len() == limit {
                return selected;
            }
        }
    }

    selected
}
