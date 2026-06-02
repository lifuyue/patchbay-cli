use serde::{Deserialize, Serialize};
use std::fmt;

use crate::config::ProfileConfig;
use crate::github::GitHubIssue;
use crate::github_enrichment::EnrichedIssue;
use crate::value_signals::{build_value_signals, SignalConfidence, ValueSignal, ValueSignalKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RankedValueIssue {
    pub issue: GitHubIssue,
    pub score: i32,
    pub value_assessment: ValueAssessment,
    pub enriched_issue: EnrichedIssue,
    pub explanation: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValueAssessment {
    pub value_score: i32,
    pub execution_gate_score: i32,
    pub recommendation: Recommendation,
    pub opportunity_type: OpportunityType,
    pub growth_confidence: GrowthConfidence,
    pub signals: Vec<ValueSignal>,
    pub risks: Vec<String>,
    pub missing_evidence: Vec<String>,
    pub explanation: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Recommendation {
    StrongCandidate,
    Candidate,
    WeakCandidate,
    Avoid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpportunityType {
    EstablishedProject,
    GrowthProject,
    Balanced,
    NicheButActionable,
    LowSignal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GrowthConfidence {
    High,
    Medium,
    Low,
}

impl fmt::Display for Recommendation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::StrongCandidate => "strong_candidate",
            Self::Candidate => "candidate",
            Self::WeakCandidate => "weak_candidate",
            Self::Avoid => "avoid",
        })
    }
}

impl fmt::Display for OpportunityType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EstablishedProject => "established_project",
            Self::GrowthProject => "growth_project",
            Self::Balanced => "balanced",
            Self::NicheButActionable => "niche_but_actionable",
            Self::LowSignal => "low_signal",
        })
    }
}

impl fmt::Display for GrowthConfidence {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        })
    }
}

pub fn assess_issue(enriched: &EnrichedIssue, profile: &ProfileConfig) -> ValueAssessment {
    aggregate_signals(build_value_signals(enriched, profile), enriched)
}

pub fn aggregate_signals(signals: Vec<ValueSignal>, enriched: &EnrichedIssue) -> ValueAssessment {
    let value_score = signals
        .iter()
        .map(|signal| signal.score_delta)
        .sum::<i32>()
        .clamp(0, 100);
    let execution_gate_score = execution_gate_score(&signals);
    let growth_confidence = growth_confidence(&signals, enriched);
    let opportunity_type = classify_opportunity_type(&signals, value_score, execution_gate_score);
    let recommendation = recommendation(value_score, execution_gate_score);
    let risks = risks(&signals);
    let missing_evidence = missing_evidence(enriched);
    let explanation = top_explanations(&signals);

    ValueAssessment {
        value_score,
        execution_gate_score,
        recommendation,
        opportunity_type,
        growth_confidence,
        signals,
        risks,
        missing_evidence,
        explanation,
    }
}

pub fn execution_gate_score(signals: &[ValueSignal]) -> i32 {
    let mut score = 20;
    for signal in signals {
        match signal.kind {
            ValueSignalKind::ExecutionReadiness => score += signal.score_delta * 2,
            ValueSignalKind::IssueClarity => score += signal.score_delta,
            ValueSignalKind::IssueFit => score += signal.score_delta,
            ValueSignalKind::MaintainerAttention => score += 6,
            ValueSignalKind::StalenessRisk | ValueSignalKind::NoiseRisk => {
                score += signal.score_delta
            }
            _ => {}
        }
    }
    score.clamp(0, 100)
}

pub fn classify_opportunity_type(
    signals: &[ValueSignal],
    value_score: i32,
    execution_gate_score: i32,
) -> OpportunityType {
    let established = signal_delta(signals, ValueSignalKind::EstablishedImpact);
    let growth = signal_delta(signals, ValueSignalKind::GrowthMomentum);
    if established >= 18 && growth >= 14 {
        OpportunityType::Balanced
    } else if established >= 18 {
        OpportunityType::EstablishedProject
    } else if growth >= 14 {
        OpportunityType::GrowthProject
    } else if execution_gate_score >= 60 && value_score >= 35 {
        OpportunityType::NicheButActionable
    } else {
        OpportunityType::LowSignal
    }
}

fn recommendation(value_score: i32, execution_gate_score: i32) -> Recommendation {
    if execution_gate_score < 30 || value_score < 20 {
        Recommendation::Avoid
    } else if value_score >= 75 && execution_gate_score >= 60 {
        Recommendation::StrongCandidate
    } else if value_score >= 45 && execution_gate_score >= 40 {
        Recommendation::Candidate
    } else {
        Recommendation::WeakCandidate
    }
}

fn growth_confidence(signals: &[ValueSignal], enriched: &EnrichedIssue) -> GrowthConfidence {
    let has_growth = signals
        .iter()
        .find(|signal| signal.kind == ValueSignalKind::GrowthMomentum);
    match has_growth.map(|signal| &signal.confidence) {
        Some(SignalConfidence::High) => GrowthConfidence::High,
        Some(SignalConfidence::Medium) => GrowthConfidence::Medium,
        _ if enriched.growth.recent_stargazer_sample.is_empty() => GrowthConfidence::Low,
        _ => GrowthConfidence::Low,
    }
}

fn signal_delta(signals: &[ValueSignal], kind: ValueSignalKind) -> i32 {
    signals
        .iter()
        .filter(|signal| signal.kind == kind)
        .map(|signal| signal.score_delta)
        .sum()
}

fn risks(signals: &[ValueSignal]) -> Vec<String> {
    signals
        .iter()
        .filter(|signal| {
            matches!(
                signal.kind,
                ValueSignalKind::StalenessRisk | ValueSignalKind::NoiseRisk
            )
        })
        .map(|signal| signal.summary.clone())
        .collect()
}

fn missing_evidence(enriched: &EnrichedIssue) -> Vec<String> {
    let mut missing = Vec::new();
    if enriched.growth.recent_stargazer_sample.is_empty() {
        missing.push("Recent stargazer sample was unavailable".to_string());
    }
    if enriched.growth.newest_fork_sample.is_empty() {
        missing.push("Newest fork sample was unavailable".to_string());
    }
    if enriched.comments.is_empty() && enriched.issue.comments_count > 0 {
        missing
            .push("Issue comments count exists but comment excerpts were unavailable".to_string());
    }
    missing.extend(enriched.warnings.iter().cloned());
    missing.sort();
    missing.dedup();
    missing
}

fn top_explanations(signals: &[ValueSignal]) -> Vec<String> {
    let mut ordered = signals.to_vec();
    ordered.sort_by_key(|signal| std::cmp::Reverse(signal.score_delta));
    ordered
        .into_iter()
        .filter(|signal| signal.score_delta > 0)
        .take(4)
        .map(|signal| signal.summary)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_signals, classify_opportunity_type, execution_gate_score, OpportunityType,
        Recommendation,
    };
    use crate::github::GitHubIssue;
    use crate::github_enrichment::EnrichedIssue;
    use crate::value_signals::{SignalConfidence, ValueSignal, ValueSignalKind};

    fn signal(kind: ValueSignalKind, delta: i32) -> ValueSignal {
        ValueSignal {
            kind,
            score_delta: delta,
            confidence: SignalConfidence::High,
            summary: "summary".to_string(),
            evidence_refs: vec!["issue:body".to_string()],
        }
    }

    fn enriched() -> EnrichedIssue {
        let issue = GitHubIssue {
            id: 1,
            number: 1,
            title: "Issue".to_string(),
            body: "Body".to_string(),
            labels: vec![],
            url: "https://github.com/owner/repo/issues/1".to_string(),
            repo_full_name: "owner/repo".to_string(),
            repo_name: "repo".to_string(),
            repo_description: String::new(),
            repo_stars: 0,
            created_at: "2026-06-01T00:00:00Z".to_string(),
            updated_at: "2026-06-01T00:00:00Z".to_string(),
        };
        EnrichedIssue::from_issue(&issue)
    }

    #[test]
    fn classifies_balanced_opportunity() {
        let signals = vec![
            signal(ValueSignalKind::EstablishedImpact, 20),
            signal(ValueSignalKind::GrowthMomentum, 18),
        ];
        assert_eq!(
            classify_opportunity_type(&signals, 70, 60),
            OpportunityType::Balanced
        );
    }

    #[test]
    fn applies_execution_gate_threshold() {
        let signals = vec![
            signal(ValueSignalKind::ExecutionReadiness, 5),
            signal(ValueSignalKind::IssueClarity, 5),
        ];
        assert!(execution_gate_score(&signals) < 40);
    }

    #[test]
    fn aggregates_value_score_and_recommendation() {
        let assessment = aggregate_signals(
            vec![
                signal(ValueSignalKind::EstablishedImpact, 24),
                signal(ValueSignalKind::GrowthMomentum, 22),
                signal(ValueSignalKind::ExecutionReadiness, 16),
                signal(ValueSignalKind::IssueClarity, 14),
            ],
            &enriched(),
        );
        assert!(assessment.value_score >= 70);
        assert_eq!(assessment.recommendation, Recommendation::StrongCandidate);
    }
}
