use anyhow::Result;
use futures::stream::{self, StreamExt};
use std::collections::HashMap;

use crate::config::Config;
use crate::discovery::{select_enrichment_candidates, DiscoveryCandidate};
use crate::github::{GitHubClient, GitHubIssue};
use crate::github_enrichment::GitHubEnrichmentClient;
use crate::paths::IssueFinderPaths;
use crate::value_scoring::{assess_issue, RankedValueIssue};

use super::events::{record_event_for_issue, RecommendationEventSource, RecommendationEventType};
use super::feed_ranker::{apply_recommendation_assessments, displayable, sort_by_feed};
use super::state::load_state_map;

const ENRICHED_SCOUT_CANDIDATE_LIMIT: usize = 100;
const ENRICHMENT_BATCH_SIZE: usize = 25;
const COMPETITION_TIMELINE_CANDIDATE_LIMIT: usize = 20;
const ENRICHMENT_CONCURRENCY_LIMIT: usize = 4;
const PRIMARY_RESULTS_PER_REPO_LIMIT: usize = 2;

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
        let ranked = self
            .rank_discovered_candidates(&enrichment, candidates, limit, refresh)
            .await;
        let filtered_count = ranked
            .iter()
            .filter(|item| !displayable(item, options.include_filtered))
            .count();
        let visible = select_display_candidates(ranked, limit, options.include_filtered);

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
    ) -> Vec<RankedValueIssue> {
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

            if ranked
                .iter()
                .filter(|item| displayable(item, false))
                .count()
                >= limit
            {
                break;
            }
        }

        ranked
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
