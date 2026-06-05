use crate::competition::CompetitionBand;
use crate::github_enrichment::EnrichedIssue;
use crate::scoring::normalize;
use crate::value_scoring::{GateStatus, RecommendationCategory, RiskTag, ValueAssessment};

use super::model::RecommendationVisibility;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityPolicyAssessment {
    pub freshness_cap: Option<i32>,
    pub penalty: i32,
    pub visibility: Option<RecommendationVisibility>,
    pub reasons: Vec<String>,
}

pub fn assess_quality_policy(
    value: &ValueAssessment,
    enriched: &EnrichedIssue,
) -> QualityPolicyAssessment {
    let mut assessment = QualityPolicyAssessment {
        freshness_cap: None,
        penalty: 0,
        visibility: None,
        reasons: Vec::new(),
    };

    apply_profile_constraints(value, &mut assessment);
    apply_low_impact_constraints(value, &mut assessment);
    apply_competition_constraints(enriched, &mut assessment);
    apply_low_depth_constraints(value, enriched, &mut assessment);
    apply_scope_constraints(value, enriched, &mut assessment);

    assessment
}

fn apply_profile_constraints(value: &ValueAssessment, assessment: &mut QualityPolicyAssessment) {
    if value.gates.profile_fit.status == GateStatus::HardFail
        || value.risk_tags.contains(&RiskTag::ProfileMismatch)
    {
        cap_freshness(assessment, 10);
        assessment.penalty += 70;
        if value.recommendation_category == RecommendationCategory::NeedsTriage
            || value.recommendation_category == RecommendationCategory::ContestedOrLowTrust
        {
            assessment.visibility = Some(RecommendationVisibility::HiddenQuality);
        }
        assessment.reasons.push(
            "Quality policy: profile mismatch caps freshness and lowers feed priority".to_string(),
        );
    } else if value.profile_fit_score < 50 {
        cap_freshness(assessment, 24);
        assessment.penalty += 25;
        if value.recommendation_category == RecommendationCategory::NeedsTriage {
            assessment.visibility = Some(RecommendationVisibility::HiddenQuality);
        }
        assessment
            .reasons
            .push("Quality policy: weak profile fit limits freshness impact".to_string());
    }
}

fn apply_low_impact_constraints(value: &ValueAssessment, assessment: &mut QualityPolicyAssessment) {
    if value.risk_tags.contains(&RiskTag::LowImpactRepo)
        && value.risk_tags.contains(&RiskTag::WeakValidationPath)
    {
        cap_freshness(assessment, 16);
        assessment.penalty += 45;
        assessment.reasons.push(
            "Quality policy: low-impact repo with weak validation path is capped".to_string(),
        );
    } else if value.risk_tags.contains(&RiskTag::LowImpactRepo) {
        cap_freshness(assessment, 28);
        assessment.penalty += 15;
        assessment
            .reasons
            .push("Quality policy: low-impact repo receives a feed penalty".to_string());
    }
}

fn apply_competition_constraints(
    enriched: &EnrichedIssue,
    assessment: &mut QualityPolicyAssessment,
) {
    let text = normalized_issue_and_comments(enriched);
    let has_open_pr = enriched.competition.open_pr_refs > 0
        || enriched.competition.fix_submitted_comments > 0
        || text.contains("pr opened to address this")
        || text.contains("pull request opened to address this")
        || text.contains("opened pr to address this")
        || text.contains("fix submitted in pr")
        || text.contains("submitted in pr")
        || text.contains("opened a pr")
        || text.contains("pull request submitted")
        || text.contains("fixed by pr")
        || text.contains("fixed by #")
        || text.contains("confirmed fixed")
        || text.contains("no longer reproduce")
        || text.contains("should we close this issue since fixed");
    let claimed = enriched.competition.attempt_comments > 0
        || enriched.competition.claim_comments > 0
        || enriched.competition.working_comments > 0
        || text.contains("i d love to work on this")
        || text.contains("i would love to work on this")
        || text.contains("interested in contributing")
        || text.contains("interested in working on this")
        || text.contains("external contributions be welcome")
        || text.contains("happy to implement")
        || text.contains("i d like to work on this")
        || text.contains("i would like to work on this")
        || text.contains("i d like to take a look")
        || text.contains("i would like to take a look")
        || text.contains("i d like to fix this")
        || text.contains("i would like to fix this")
        || text.contains("can i work on this")
        || text.contains("could i work on this")
        || text.contains("i m working on this")
        || text.contains("i am working on this")
        || text.contains("i will look into it")
        || text.contains("i ll look into it")
        || text.contains("i was able to replicate")
        || text.contains("pick this up")
        || text.contains("picked this up")
        || text.contains("take this up")
        || text.contains("please assign me")
        || text.contains("puedo trabajar en este")
        || text.contains("puedo tomar este")
        || text.contains("feel free to fork")
        || text.contains("looking forward to your contribution");

    if has_open_pr {
        assessment.visibility = Some(RecommendationVisibility::HiddenQuality);
        assessment.penalty += 200;
        assessment
            .reasons
            .push("Quality policy: issue appears to have an open or submitted PR".to_string());
        return;
    }

    if enriched.competition.competition_band == CompetitionBand::Contested
        || enriched.competition.competition_band == CompetitionBand::Saturated
    {
        assessment.penalty += 90;
        assessment
            .reasons
            .push("Quality policy: contested issue is strongly deprioritized".to_string());
    }

    if claimed {
        assessment.visibility = Some(RecommendationVisibility::HiddenQuality);
        assessment.penalty += 140;
        assessment.reasons.push(
            "Quality policy: issue appears claimed or already guided to a contributor".to_string(),
        );
    }
}

fn apply_low_depth_constraints(
    value: &ValueAssessment,
    enriched: &EnrichedIssue,
    assessment: &mut QualityPolicyAssessment,
) {
    let text = normalized_issue_and_repo(enriched);
    if value.recommendation_category == RecommendationCategory::FilteredLowDepth {
        assessment.visibility = Some(RecommendationVisibility::HiddenFiltered);
        assessment.penalty += 160;
        assessment
            .reasons
            .push("Quality policy: value model filtered this low-depth issue".to_string());
        return;
    }

    if is_trivial_docs_task(&text) {
        assessment.visibility = Some(RecommendationVisibility::HiddenQuality);
        assessment.penalty += 160;
        assessment
            .reasons
            .push("Quality policy: low-depth documentation or wording task is hidden".to_string());
        return;
    }

    if is_docs_polish_task(&text) {
        assessment.visibility = Some(RecommendationVisibility::HiddenQuality);
        assessment.penalty += 120;
        assessment
            .reasons
            .push("Quality policy: documentation polish task is hidden".to_string());
    }
}

fn apply_scope_constraints(
    value: &ValueAssessment,
    enriched: &EnrichedIssue,
    assessment: &mut QualityPolicyAssessment,
) {
    let text = normalized_issue_and_repo(enriched);
    let broad_audit_needs_triage = is_large_audit_task(&text)
        && (value.recommendation_category == RecommendationCategory::NeedsTriage
            || value.risk_tags.contains(&RiskTag::HighTriageLoad)
            || value.risk_tags.contains(&RiskTag::WeakValidationPath)
            || value.risk_tags.contains(&RiskTag::ProfileMismatch));
    if broad_audit_needs_triage {
        assessment.visibility = Some(RecommendationVisibility::HiddenQuality);
        assessment.penalty += 180;
        assessment.reasons.push(
            "Quality policy: broad audit or campaign task is hidden from the feed".to_string(),
        );
        return;
    }

    if value.risk_tags.contains(&RiskTag::ScopeRisk) || is_large_audit_task(&text) {
        cap_freshness(assessment, 18);
        assessment.penalty += 75;
        assessment
            .reasons
            .push("Quality policy: broad audit or campaign task needs scoping first".to_string());
    }
}

fn cap_freshness(assessment: &mut QualityPolicyAssessment, cap: i32) {
    assessment.freshness_cap = Some(
        assessment
            .freshness_cap
            .map(|existing| existing.min(cap))
            .unwrap_or(cap),
    );
}

fn is_trivial_docs_task(text: &str) -> bool {
    (contains_any(
        text,
        &[
            "readme",
            "manual",
            "user guide",
            "claude md",
            "contributing md",
        ],
    ) && contains_any(
        text,
        &[
            "english wording",
            "clarity grammar",
            "grammar",
            "natural wording",
            "duplicate api entries",
            "remove duplicate",
            "single line fix",
            "one line fix",
            "incorrect signature",
            "documentation error",
            "crate name fix",
            "line 284",
        ],
    )) || contains_any(
        text,
        &[
            "add documentation",
            "add docs",
            "trivial 5 minutes",
            "trivial 10 minutes",
            "estimated effort trivial",
            "pure docs formatting",
            "documentation formatting error",
            "documentation only no behavior change",
            "doc comments still describe",
            "stale wording in",
        ],
    )
}

fn is_docs_polish_task(text: &str) -> bool {
    contains_any(
        text,
        &[
            "docs should include",
            "please add a further bullet point",
            "review the english",
            "improve english wording",
            "manual and public project description",
            "readme completeness",
            "license footer",
        ],
    )
}

fn is_large_audit_task(text: &str) -> bool {
    contains_any(
        text,
        &[
            "audit",
            "110 gaps",
            "127 publishable crates",
            "2000 print",
            "2 000 print",
            "phase 1 triage",
            "phase 2 remediation",
            "large volume",
            "not a mechanical fix",
            "requires judgment",
            "sampling phase",
            "campaign",
        ],
    )
}

fn normalized_issue_and_repo(enriched: &EnrichedIssue) -> String {
    normalize(&format!(
        "{} {} {} {} {}",
        enriched.issue.title,
        enriched.issue.body,
        enriched.issue.labels.join(" "),
        enriched.repository.description,
        enriched.repository.topics.join(" ")
    ))
}

fn normalized_issue_and_comments(enriched: &EnrichedIssue) -> String {
    normalize(&format!(
        "{} {} {}",
        enriched.issue.title,
        enriched.issue.body,
        enriched
            .comments
            .iter()
            .map(|comment| comment.body_excerpt.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    ))
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}
