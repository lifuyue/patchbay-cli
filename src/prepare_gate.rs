use crate::value_scoring::{GateStatus, GateVerdict, RecommendationCategory, ValueAssessment};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareGateDecision {
    Allowed,
    Blocked {
        category: RecommendationCategory,
        reasons: Vec<String>,
        bypass_available: bool,
    },
    Bypassed {
        category: RecommendationCategory,
        reason: String,
    },
}

pub fn default_prepare_allowed(category: RecommendationCategory) -> bool {
    matches!(
        category,
        RecommendationCategory::HighValueReady | RecommendationCategory::HighValueNeedsScoping
    )
}

pub fn allowed_prepare_categories() -> [RecommendationCategory; 2] {
    [
        RecommendationCategory::HighValueReady,
        RecommendationCategory::HighValueNeedsScoping,
    ]
}

pub fn prepare_gate_decision(
    assessment: &ValueAssessment,
    bypass_reason: Option<&str>,
) -> PrepareGateDecision {
    let category = assessment.recommendation_category;
    if default_prepare_allowed(category) {
        return PrepareGateDecision::Allowed;
    }

    if let Some(reason) = bypass_reason
        .map(str::trim)
        .filter(|reason| !reason.is_empty())
    {
        return PrepareGateDecision::Bypassed {
            category,
            reason: reason.to_string(),
        };
    }

    PrepareGateDecision::Blocked {
        category,
        reasons: prepare_gate_reasons(assessment),
        bypass_available: true,
    }
}

pub fn prepare_gate_reasons(assessment: &ValueAssessment) -> Vec<String> {
    let mut reasons = Vec::new();
    collect_gate_reasons(&mut reasons, &assessment.gates.low_depth);
    collect_gate_reasons(&mut reasons, &assessment.gates.repo_influence);
    collect_gate_reasons(&mut reasons, &assessment.gates.competition);
    collect_gate_reasons(&mut reasons, &assessment.gates.profile_fit);
    if assessment.execution_score < 50 {
        reasons.push(format!(
            "Execution score is below prepare threshold ({})",
            assessment.execution_score
        ));
    }
    for tag in &assessment.risk_tags {
        reasons.push(format!("Risk tag: {tag}"));
    }
    for item in &assessment.missing_evidence {
        reasons.push(format!("Missing evidence: {item}"));
    }
    if reasons.is_empty() {
        reasons.push(format!(
            "Category {} is outside the default prepare gate",
            assessment.recommendation_category
        ));
    }
    reasons.sort();
    reasons.dedup();
    reasons
}

fn collect_gate_reasons(reasons: &mut Vec<String>, gate: &GateVerdict) {
    if gate.status != GateStatus::Pass {
        reasons.extend(gate.reasons.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::{default_prepare_allowed, prepare_gate_decision, PrepareGateDecision};
    use crate::value_scoring::{RecommendationCategory, ValueAssessment};

    #[test]
    fn default_prepare_policy_allows_only_high_value_categories() {
        assert!(default_prepare_allowed(
            RecommendationCategory::HighValueReady
        ));
        assert!(default_prepare_allowed(
            RecommendationCategory::HighValueNeedsScoping
        ));
        assert!(!default_prepare_allowed(
            RecommendationCategory::NicheButActionable
        ));
        assert!(!default_prepare_allowed(
            RecommendationCategory::ContestedOrLowTrust
        ));
        assert!(!default_prepare_allowed(
            RecommendationCategory::FilteredLowDepth
        ));
        assert!(!default_prepare_allowed(
            RecommendationCategory::NeedsTriage
        ));
    }

    #[test]
    fn prepare_gate_requires_reason_for_blocked_categories() {
        let assessment = ValueAssessment {
            recommendation_category: RecommendationCategory::NicheButActionable,
            category: RecommendationCategory::NicheButActionable,
            execution_score: 70,
            ..ValueAssessment::default()
        };

        assert!(matches!(
            prepare_gate_decision(&assessment, None),
            PrepareGateDecision::Blocked { .. }
        ));
        assert!(matches!(
            prepare_gate_decision(&assessment, Some("explicit user request")),
            PrepareGateDecision::Bypassed { .. }
        ));
    }
}
