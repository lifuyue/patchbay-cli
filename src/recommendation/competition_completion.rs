use std::collections::{HashMap, HashSet};

use crate::github_enrichment::{competition_timeline_missing, competition_timeline_not_fetched};
use crate::value_scoring::{RankedValueIssue, RecommendationCategory};

use super::feed_ranker::displayable;

pub const TOP10_MISSING_TIMELINE_LIMIT: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompetitionCompletionStatus {
    Completed,
    Failed,
    SkippedByBudget,
}

impl CompetitionCompletionStatus {
    pub fn explanation(self) -> &'static str {
        match self {
            Self::Completed => "Competition timeline completed before final ranking",
            Self::Failed => {
                "Competition timeline completion failed; candidate is limited by missing evidence"
            }
            Self::SkippedByBudget => {
                "Competition timeline completion skipped by budget; candidate is limited by missing evidence"
            }
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CompetitionCompletionPlan {
    pub complete_keys: Vec<String>,
    pub skipped_keys: Vec<String>,
}

pub fn completion_budget(limit: usize) -> usize {
    limit.saturating_mul(2).min(30)
}

pub fn plan_completion(ranked: &[RankedValueIssue], limit: usize) -> CompetitionCompletionPlan {
    if limit == 0 {
        return CompetitionCompletionPlan::default();
    }

    let budget = completion_budget(limit);
    let mut visible_seen = 0usize;
    let mut candidates = Vec::new();
    let max_scan = limit.saturating_mul(4).max(10);

    for (index, item) in ranked.iter().enumerate() {
        let is_displayable = displayable(item, false);
        if is_displayable {
            visible_seen += 1;
        }

        if !competition_timeline_not_fetched(&item.enriched_issue) {
            continue;
        }

        if is_visible_ish(item, index, visible_seen, is_displayable, limit, max_scan) {
            candidates.push(issue_key(item));
        }
    }

    let complete_keys = candidates.iter().take(budget).cloned().collect::<Vec<_>>();
    let skipped_keys = candidates.into_iter().skip(budget).collect::<Vec<_>>();
    CompetitionCompletionPlan {
        complete_keys,
        skipped_keys,
    }
}

pub fn annotate_skipped_by_budget(
    ranked: &mut [RankedValueIssue],
    skipped_keys: &[String],
) -> HashMap<String, CompetitionCompletionStatus> {
    let skipped = skipped_keys.iter().cloned().collect::<HashSet<_>>();
    let mut statuses = HashMap::new();
    for item in ranked {
        let key = issue_key(item);
        if !skipped.contains(&key) {
            continue;
        }
        push_unique_warning(
            &mut item.enriched_issue.competition.warnings,
            "Competition timeline completion skipped by budget",
        );
        statuses.insert(key, CompetitionCompletionStatus::SkippedByBudget);
    }
    statuses
}

pub fn select_display_candidates(
    ranked: Vec<RankedValueIssue>,
    limit: usize,
    include_filtered: bool,
    per_repo_limit: usize,
) -> Vec<RankedValueIssue> {
    if limit == 0 {
        return Vec::new();
    }

    let mut selected = Vec::new();
    let mut deferred_missing = Vec::new();
    let mut repo_counts = HashMap::<String, usize>::new();
    let mut top10_missing_count = 0usize;

    for item in ranked {
        if !displayable(&item, include_filtered) {
            continue;
        }

        if repo_count(&repo_counts, &item) >= per_repo_limit {
            continue;
        }

        if competition_timeline_missing(&item.enriched_issue)
            && !can_place_missing_candidate(selected.len() + 1, top10_missing_count)
        {
            deferred_missing.push(item);
            continue;
        }

        if competition_timeline_missing(&item.enriched_issue) && selected.len() < 10 {
            top10_missing_count += 1;
        }
        increment_repo(&mut repo_counts, &item);
        selected.push(item);
        if selected.len() == limit {
            return selected;
        }
    }

    for item in deferred_missing {
        if selected.len() == limit {
            break;
        }
        if selected.len() < 5 {
            continue;
        }
        if repo_count(&repo_counts, &item) >= per_repo_limit {
            continue;
        }
        if competition_timeline_missing(&item.enriched_issue)
            && !can_place_missing_candidate(selected.len() + 1, top10_missing_count)
        {
            continue;
        }
        if competition_timeline_missing(&item.enriched_issue) && selected.len() < 10 {
            top10_missing_count += 1;
        }
        increment_repo(&mut repo_counts, &item);
        selected.push(item);
    }

    selected
}

pub fn append_completion_explanations(
    ranked: &mut [RankedValueIssue],
    statuses: &HashMap<String, CompetitionCompletionStatus>,
) {
    for item in ranked {
        let Some(status) = statuses.get(&issue_key(item)).copied() else {
            continue;
        };
        let reason = status.explanation().to_string();
        if !item.explanation.contains(&reason) {
            item.explanation.push(reason);
        }
    }
}

pub fn issue_key(item: &RankedValueIssue) -> String {
    format!("{}#{}", item.issue.repo_full_name, item.issue.number)
}

fn is_visible_ish(
    item: &RankedValueIssue,
    index: usize,
    visible_seen: usize,
    is_displayable: bool,
    limit: usize,
    max_scan: usize,
) -> bool {
    if is_displayable && visible_seen <= limit.saturating_mul(2) {
        return true;
    }
    if index >= max_scan {
        return false;
    }
    matches!(
        item.value_assessment.recommendation_category,
        RecommendationCategory::HighValueReady
            | RecommendationCategory::HighValueNeedsScoping
            | RecommendationCategory::NicheButActionable
    ) || item.recommendation.final_feed_score >= 300
}

fn can_place_missing_candidate(position: usize, top10_missing_count: usize) -> bool {
    if position <= 5 {
        return false;
    }
    if position <= 10 {
        return top10_missing_count < TOP10_MISSING_TIMELINE_LIMIT;
    }
    true
}

fn repo_count(repo_counts: &HashMap<String, usize>, item: &RankedValueIssue) -> usize {
    repo_counts
        .get(&item.issue.repo_full_name)
        .copied()
        .unwrap_or(0)
}

fn increment_repo(repo_counts: &mut HashMap<String, usize>, item: &RankedValueIssue) {
    *repo_counts
        .entry(item.issue.repo_full_name.clone())
        .or_insert(0) += 1;
}

fn push_unique_warning(warnings: &mut Vec<String>, warning: &str) {
    if !warnings.iter().any(|item| item == warning) {
        warnings.push(warning.to_string());
    }
}

#[cfg(test)]
mod tests {
    use crate::competition::CompetitionFacts;
    use crate::github::GitHubIssue;
    use crate::github_enrichment::EnrichedIssue;
    use crate::recommendation::RecommendationAssessment;
    use crate::value_scoring::{
        GateBand, GateStatus, GateVerdict, RankedValueIssue, RecommendationCategory, ScoreBand,
        ValueAssessment, ValueGates,
    };

    use super::{
        annotate_skipped_by_budget, plan_completion, select_display_candidates,
        CompetitionCompletionStatus,
    };

    #[test]
    fn plan_completion_selects_visible_ish_missing_timeline_candidates_with_budget() {
        let ranked = (0..20)
            .map(|index| {
                let mut item = ranked_issue(index, RecommendationCategory::HighValueNeedsScoping);
                if index % 3 == 0 {
                    item.enriched_issue.competition = CompetitionFacts::default();
                }
                item
            })
            .collect::<Vec<_>>();

        let plan = plan_completion(&ranked, 5);

        assert_eq!(plan.complete_keys.len(), 10);
        assert!(!plan.skipped_keys.is_empty());
        assert!(!plan.complete_keys.iter().any(|key| key.ends_with("/0#1")));
    }

    #[test]
    fn skipped_missing_timeline_candidates_do_not_enter_top5_and_are_limited_in_top10() {
        let ranked = (0..12)
            .map(|index| {
                let mut item = ranked_issue(index, RecommendationCategory::HighValueReady);
                item.recommendation.final_feed_score = 1_000 - index as i32;
                if index >= 6 {
                    item.enriched_issue.competition = CompetitionFacts::default();
                }
                item
            })
            .collect::<Vec<_>>();

        let selected = select_display_candidates(ranked, 10, false, 2);
        let top5_missing = selected
            .iter()
            .take(5)
            .filter(|item| !item.enriched_issue.competition.warnings.is_empty())
            .count();
        let top10_missing = selected
            .iter()
            .take(10)
            .filter(|item| !item.enriched_issue.competition.warnings.is_empty())
            .count();

        assert_eq!(top5_missing, 0);
        assert_eq!(top10_missing, 2);
    }

    #[test]
    fn failed_timeline_completion_is_limited_like_missing_evidence() {
        let ranked = (0..12)
            .map(|index| {
                let mut item = ranked_issue(index, RecommendationCategory::HighValueReady);
                item.recommendation.final_feed_score = 1_000 - index as i32;
                if index < 6 {
                    item.enriched_issue.competition.warnings =
                        vec!["Competition timeline enrichment failed: rate limit".to_string()];
                } else {
                    item.enriched_issue.competition = CompetitionFacts::default();
                }
                item
            })
            .collect::<Vec<_>>();

        let selected = select_display_candidates(ranked, 10, false, 2);
        let top5_failed = selected
            .iter()
            .take(5)
            .filter(|item| {
                item.enriched_issue
                    .competition
                    .warnings
                    .iter()
                    .any(|warning| warning.contains("timeline enrichment failed"))
            })
            .count();
        let top10_failed = selected
            .iter()
            .take(10)
            .filter(|item| {
                item.enriched_issue
                    .competition
                    .warnings
                    .iter()
                    .any(|warning| warning.contains("timeline enrichment failed"))
            })
            .count();

        assert_eq!(top5_failed, 0);
        assert_eq!(top10_failed, 2);
    }

    #[test]
    fn annotate_skipped_by_budget_records_status_and_warning() {
        let mut ranked = vec![ranked_issue(1, RecommendationCategory::HighValueReady)];
        let key = super::issue_key(&ranked[0]);

        let statuses = annotate_skipped_by_budget(&mut ranked, std::slice::from_ref(&key));

        assert_eq!(
            statuses.get(&key),
            Some(&CompetitionCompletionStatus::SkippedByBudget)
        );
        assert!(ranked[0]
            .enriched_issue
            .competition
            .warnings
            .iter()
            .any(|warning| warning.contains("skipped by budget")));
    }

    fn ranked_issue(index: usize, category: RecommendationCategory) -> RankedValueIssue {
        let issue = GitHubIssue {
            id: index as u64,
            number: 1,
            title: format!("Issue {index}"),
            body: "Expected behavior in src/lib.rs with clear reproduction.".to_string(),
            labels: vec!["good first issue".to_string()],
            url: format!("https://github.com/owner/repo-{index}/issues/1"),
            repo_full_name: format!("owner/repo-{index}"),
            repo_name: format!("repo-{index}"),
            repo_description: "Rust CLI developer tools".to_string(),
            repo_stars: 1_000,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-06-01T00:00:00Z".to_string(),
        };
        let enriched_issue = EnrichedIssue::from_issue(&issue);
        let value_assessment = ValueAssessment {
            final_rank_score: 80,
            category,
            recommendation_category: category,
            gates: passing_gates(),
            attention_score: 80,
            execution_score: 80,
            profile_fit_score: 80,
            attention_band: ScoreBand::High,
            execution_band: ScoreBand::High,
            explanation: vec!["test assessment".to_string()],
            ..ValueAssessment::default()
        };
        RankedValueIssue {
            issue,
            score: value_assessment.final_rank_score,
            value_assessment: value_assessment.clone(),
            enriched_issue,
            explanation: value_assessment.explanation.clone(),
            recommendation: RecommendationAssessment::from_value_assessment(&value_assessment),
        }
    }

    fn passing_gates() -> ValueGates {
        ValueGates {
            low_depth: pass_gate(),
            repo_influence: pass_gate(),
            competition: pass_gate(),
            profile_fit: pass_gate(),
        }
    }

    fn pass_gate() -> GateVerdict {
        GateVerdict {
            status: GateStatus::Pass,
            band: GateBand::Strong,
            reasons: vec!["pass".to_string()],
            evidence_refs: Vec::new(),
        }
    }
}
