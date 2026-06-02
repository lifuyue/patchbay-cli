use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::config::ProfileConfig;
use crate::github_enrichment::{fork_velocity, star_velocity, EnrichedIssue};
use crate::scoring::{has_actionable_signal, normalize, profile_terms};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValueSignalKind {
    EstablishedImpact,
    GrowthMomentum,
    RepoActivity,
    MaintainerAttention,
    IssueClarity,
    ContributionWindow,
    IssueFit,
    ExecutionReadiness,
    StalenessRisk,
    NoiseRisk,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SignalConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValueSignal {
    pub kind: ValueSignalKind,
    pub score_delta: i32,
    pub confidence: SignalConfidence,
    pub summary: String,
    pub evidence_refs: Vec<String>,
}

pub fn build_value_signals(enriched: &EnrichedIssue, profile: &ProfileConfig) -> Vec<ValueSignal> {
    let mut signals = Vec::new();

    if let Some(signal) = established_impact_signal(enriched) {
        signals.push(signal);
    }
    if let Some(signal) = growth_momentum_signal(enriched) {
        signals.push(signal);
    }
    if let Some(signal) = repo_activity_signal(enriched) {
        signals.push(signal);
    }
    if let Some(signal) = maintainer_attention_signal(enriched) {
        signals.push(signal);
    }
    if let Some(signal) = issue_clarity_signal(enriched) {
        signals.push(signal);
    }
    if let Some(signal) = contribution_window_signal(enriched) {
        signals.push(signal);
    }
    if let Some(signal) = issue_fit_signal(enriched, profile) {
        signals.push(signal);
    }
    if let Some(signal) = execution_readiness_signal(enriched) {
        signals.push(signal);
    }
    if let Some(signal) = staleness_risk_signal(enriched) {
        signals.push(signal);
    }
    if let Some(signal) = noise_risk_signal(enriched) {
        signals.push(signal);
    }

    signals
}

fn established_impact_signal(enriched: &EnrichedIssue) -> Option<ValueSignal> {
    let stars = enriched.repository.stars;
    let forks = enriched.repository.forks;
    let subscribers = enriched.repository.subscribers.unwrap_or(0);
    let delta = if stars >= 10_000 || forks >= 1_000 {
        24
    } else if stars >= 2_000 || forks >= 200 {
        18
    } else if stars >= 500 || forks >= 50 || subscribers >= 100 {
        12
    } else if stars >= 100 {
        7
    } else {
        return None;
    };

    Some(ValueSignal {
        kind: ValueSignalKind::EstablishedImpact,
        score_delta: delta,
        confidence: SignalConfidence::High,
        summary: format!("Repository has visible impact with {stars} stars and {forks} forks."),
        evidence_refs: refs(["repo:stargazers_count", "repo:forks_count"]),
    })
}

fn growth_momentum_signal(enriched: &EnrichedIssue) -> Option<ValueSignal> {
    let now = Utc::now();
    let stars_7d = star_velocity(&enriched.growth.recent_stargazer_sample, 7, now);
    let stars_30d = star_velocity(&enriched.growth.recent_stargazer_sample, 30, now);
    let forks_30d = fork_velocity(&enriched.growth.newest_fork_sample, 30, now);
    let attention_to_size = stars_30d as f64 / (enriched.repository.stars.max(1) as f64);

    let delta = if stars_7d >= 15 || stars_30d >= 40 || forks_30d >= 8 || attention_to_size >= 0.15
    {
        22
    } else if stars_7d >= 5 || stars_30d >= 15 || forks_30d >= 3 || attention_to_size >= 0.05 {
        14
    } else if stars_30d > 0 || forks_30d > 0 {
        7
    } else {
        return None;
    };

    let confidence = if enriched
        .growth
        .recent_stargazer_sample
        .iter()
        .any(|sample| sample.timestamp.is_some())
    {
        SignalConfidence::Medium
    } else {
        SignalConfidence::Low
    };

    Some(ValueSignal {
        kind: ValueSignalKind::GrowthMomentum,
        score_delta: delta,
        confidence,
        summary: format!(
            "Sampled recent attention: {stars_7d} stars in 7d, {stars_30d} stars in 30d, {forks_30d} forks in 30d."
        ),
        evidence_refs: refs([
            "repo:stargazers.sample_recent_100",
            "repo:forks.sample_newest_100",
        ]),
    })
}

fn repo_activity_signal(enriched: &EnrichedIssue) -> Option<ValueSignal> {
    if !enriched.activity.recent_repo_activity {
        return None;
    }
    Some(ValueSignal {
        kind: ValueSignalKind::RepoActivity,
        score_delta: 10,
        confidence: SignalConfidence::High,
        summary: "Repository was pushed recently.".to_string(),
        evidence_refs: refs(["repo:pushed_at"]),
    })
}

fn maintainer_attention_signal(enriched: &EnrichedIssue) -> Option<ValueSignal> {
    if !enriched.activity.maintainer_recent_response {
        return None;
    }
    Some(ValueSignal {
        kind: ValueSignalKind::MaintainerAttention,
        score_delta: 12,
        confidence: SignalConfidence::High,
        summary: "A maintainer recently responded in the issue thread.".to_string(),
        evidence_refs: refs(["issue:comments", "issue:author_association"]),
    })
}

fn issue_clarity_signal(enriched: &EnrichedIssue) -> Option<ValueSignal> {
    let body_len = enriched.issue.body.trim().len();
    let actionable = has_actionable_signal(&enriched.issue.title, &enriched.issue.body);
    let delta = if actionable && body_len >= 120 {
        14
    } else if actionable || body_len >= 120 {
        9
    } else if body_len >= 40 {
        4
    } else {
        return None;
    };
    Some(ValueSignal {
        kind: ValueSignalKind::IssueClarity,
        score_delta: delta,
        confidence: SignalConfidence::High,
        summary: "Issue contains enough detail to start investigation.".to_string(),
        evidence_refs: refs(["issue:title", "issue:body"]),
    })
}

fn contribution_window_signal(enriched: &EnrichedIssue) -> Option<ValueSignal> {
    if !enriched.activity.recent_issue_activity {
        return None;
    }
    Some(ValueSignal {
        kind: ValueSignalKind::ContributionWindow,
        score_delta: 8,
        confidence: SignalConfidence::Medium,
        summary: "Issue activity is recent enough for an active contribution window.".to_string(),
        evidence_refs: refs(["issue:updated_at"]),
    })
}

fn issue_fit_signal(enriched: &EnrichedIssue, profile: &ProfileConfig) -> Option<ValueSignal> {
    let searchable = normalize(&format!(
        "{} {} {} {} {}",
        enriched.issue.title,
        enriched.issue.body,
        enriched.repository.full_name,
        enriched.repository.description,
        enriched.repository.topics.join(" ")
    ));
    let matched = profile_terms(profile)
        .into_iter()
        .filter(|term| searchable.contains(term))
        .collect::<Vec<_>>();
    if matched.is_empty() {
        return None;
    }
    let delta = (matched.len() as i32 * 5).min(15);
    Some(ValueSignal {
        kind: ValueSignalKind::IssueFit,
        score_delta: delta,
        confidence: SignalConfidence::High,
        summary: format!("Matched profile terms: {}.", matched.join(", ")),
        evidence_refs: refs(["profile:terms", "issue:title", "repo:topics"]),
    })
}

fn execution_readiness_signal(enriched: &EnrichedIssue) -> Option<ValueSignal> {
    let has_good_first = enriched
        .issue
        .labels
        .iter()
        .any(|label| normalize(label).contains("good first issue"));
    let actionable = has_actionable_signal(&enriched.issue.title, &enriched.issue.body);
    if !has_good_first && !actionable {
        return None;
    }
    Some(ValueSignal {
        kind: ValueSignalKind::ExecutionReadiness,
        score_delta: if has_good_first && actionable { 16 } else { 10 },
        confidence: SignalConfidence::Medium,
        summary: "Issue has beginner-friendly or actionable execution signals.".to_string(),
        evidence_refs: refs(["issue:labels", "issue:body"]),
    })
}

fn staleness_risk_signal(enriched: &EnrichedIssue) -> Option<ValueSignal> {
    if enriched.activity.recent_issue_activity || enriched.activity.recent_repo_activity {
        return None;
    }
    Some(ValueSignal {
        kind: ValueSignalKind::StalenessRisk,
        score_delta: -18,
        confidence: SignalConfidence::Medium,
        summary: "Issue and repository activity are not recent.".to_string(),
        evidence_refs: refs(["issue:updated_at", "repo:pushed_at"]),
    })
}

fn noise_risk_signal(enriched: &EnrichedIssue) -> Option<ValueSignal> {
    let comment_count = enriched.issue.comments_count;
    let open_issues = enriched.repository.open_issues.unwrap_or(0);
    if comment_count < 20 && open_issues < 500 {
        return None;
    }
    Some(ValueSignal {
        kind: ValueSignalKind::NoiseRisk,
        score_delta: if comment_count >= 50 || open_issues >= 1_000 {
            -12
        } else {
            -6
        },
        confidence: SignalConfidence::Medium,
        summary: "Issue or repository has enough activity to require extra triage.".to_string(),
        evidence_refs: refs(["issue:comments_count", "repo:open_issues_count"]),
    })
}

fn refs<const N: usize>(values: [&str; N]) -> Vec<String> {
    values.into_iter().map(ToOwned::to_owned).collect()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{build_value_signals, ValueSignalKind};
    use crate::config::ProfileConfig;
    use crate::github::GitHubIssue;
    use crate::github_enrichment::{EnrichedIssue, TimestampedSample};

    fn enriched() -> EnrichedIssue {
        let issue = GitHubIssue {
            id: 1,
            number: 1,
            title: "Fix Rust CLI parser".to_string(),
            body: "Expected graceful behavior in src/main.rs".to_string(),
            labels: vec!["good first issue".to_string()],
            url: "https://github.com/owner/repo/issues/1".to_string(),
            repo_full_name: "owner/repo".to_string(),
            repo_name: "repo".to_string(),
            repo_description: "Rust CLI".to_string(),
            repo_stars: 2_500,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };
        let mut enriched = EnrichedIssue::from_issue(&issue);
        enriched.repository.forks = 300;
        enriched.repository.pushed_at = Some(Utc::now().to_rfc3339());
        enriched.growth.recent_stargazer_sample = vec![TimestampedSample {
            source_ref: "repo:stargazers.sample_recent_100.0".to_string(),
            actor: Some("user".to_string()),
            timestamp: Some(Utc::now().to_rfc3339()),
        }];
        enriched
    }

    #[test]
    fn builds_expected_value_signals() {
        let profile = ProfileConfig {
            tech_stack: vec!["Rust".to_string()],
            keywords: vec!["cli".to_string()],
        };
        let signals = build_value_signals(&enriched(), &profile);
        assert!(signals
            .iter()
            .any(|signal| signal.kind == ValueSignalKind::EstablishedImpact));
        assert!(signals
            .iter()
            .any(|signal| signal.kind == ValueSignalKind::ExecutionReadiness));
    }
}
