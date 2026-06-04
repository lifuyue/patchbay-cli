use chrono::{DateTime, Utc};

use crate::competition::{CompetitionBand, CompetitionFacts};
use crate::github_enrichment::EnrichedIssue;
use crate::scoring::normalize;
use crate::value_model::{GateBand, GateStatus, GateVerdict};
use crate::value_scores::ProfileFitAssessment;

pub fn low_depth_gate(enriched: &EnrichedIssue) -> GateVerdict {
    let matches = low_depth_matches(enriched);
    if matches.is_empty() {
        return GateVerdict::pass(
            "No low-depth content-fill signals matched",
            vec!["issue:body".to_string(), "issue:labels".to_string()],
        );
    }

    GateVerdict::new(
        GateStatus::HardFail,
        GateBand::Suspicious,
        vec![format!(
            "Low-depth preclassification matched: {}",
            matches.join(", ")
        )],
        vec!["issue:body".to_string(), "issue:labels".to_string()],
    )
}

pub fn repo_influence_gate(enriched: &EnrichedIssue) -> GateVerdict {
    let stars = enriched.repository.stars;
    let forks = enriched.repository.forks;
    let watchers = enriched.repository.subscribers.unwrap_or(0);
    let fork_star_ratio = fork_star_ratio(stars, forks);
    let marketplace_terms = marketplace_terms(enriched);
    let repo_age_days = repo_age_days(enriched.repository.created_at.as_deref());
    let open_issues = enriched.repository.open_issues.unwrap_or(0);

    if enriched.repository.archived {
        return GateVerdict::new(
            GateStatus::HardFail,
            GateBand::Suspicious,
            vec!["Repository is archived".to_string()],
            vec!["repo:archived".to_string()],
        );
    }

    if !marketplace_terms.is_empty() {
        return GateVerdict::new(
            GateStatus::HardFail,
            GateBand::Suspicious,
            vec![format!(
                "Marketplace or bounty-farm signals matched: {}",
                marketplace_terms.join(", ")
            )],
            vec!["repo:description".to_string(), "issue:labels".to_string()],
        );
    }

    if stars < 100 && watchers == 0 && forks > stars.saturating_mul(3).max(25) {
        return GateVerdict::new(
            GateStatus::HardFail,
            GateBand::Suspicious,
            vec![format!(
                "Repository has low stars ({stars}), zero watchers, and fork/star anomaly ({forks} forks)"
            )],
            vec![
                "repo:stargazers_count".to_string(),
                "repo:subscribers_count".to_string(),
                "repo:forks_count".to_string(),
            ],
        );
    }

    if fork_star_ratio > 3.0 && stars < 500 {
        return GateVerdict::new(
            GateStatus::HardFail,
            GateBand::Suspicious,
            vec![format!(
                "Fork/star ratio is suspicious for a low-star repository ({fork_star_ratio:.1})"
            )],
            vec![
                "repo:forks_count".to_string(),
                "repo:stargazers_count".to_string(),
            ],
        );
    }

    if repo_age_days.is_some_and(|days| days < 90) && open_issues >= 100 {
        return GateVerdict::new(
            GateStatus::HardFail,
            GateBand::Suspicious,
            vec![format!(
                "Repository is very young and already has {open_issues} open issues"
            )],
            vec![
                "repo:created_at".to_string(),
                "repo:open_issues_count".to_string(),
            ],
        );
    }

    if stars >= 1_000 || (stars >= 500 && watchers >= 20) || (forks >= 200 && stars >= 500) {
        return GateVerdict::new(
            GateStatus::Pass,
            GateBand::Strong,
            vec![format!(
                "Repository influence passes high-value threshold ({stars} stars, {forks} forks, {watchers} watchers)"
            )],
            vec![
                "repo:stargazers_count".to_string(),
                "repo:forks_count".to_string(),
                "repo:subscribers_count".to_string(),
            ],
        );
    }

    if stars >= 100 || watchers >= 10 || (forks >= 50 && fork_star_ratio <= 3.0) {
        return GateVerdict::new(
            GateStatus::SoftFail,
            GateBand::Acceptable,
            vec![format!(
                "Repository is below high-value influence but credible enough for niche consideration ({stars} stars, {forks} forks, {watchers} watchers)"
            )],
            vec![
                "repo:stargazers_count".to_string(),
                "repo:forks_count".to_string(),
                "repo:subscribers_count".to_string(),
            ],
        );
    }

    GateVerdict::new(
        GateStatus::SoftFail,
        GateBand::Weak,
        vec![format!(
            "Repository influence is weak ({stars} stars, {forks} forks, {watchers} watchers)"
        )],
        vec![
            "repo:stargazers_count".to_string(),
            "repo:forks_count".to_string(),
            "repo:subscribers_count".to_string(),
        ],
    )
}

pub fn competition_gate(facts: &CompetitionFacts) -> GateVerdict {
    if !facts.warnings.is_empty() {
        return GateVerdict::new(
            GateStatus::SoftFail,
            GateBand::Weak,
            facts.warnings.clone(),
            vec!["issue:timeline".to_string()],
        );
    }

    match facts.competition_band {
        CompetitionBand::Clear => GateVerdict::new(
            GateStatus::Pass,
            GateBand::Strong,
            vec!["Competition evidence is clear (0-1 points)".to_string()],
            vec!["issue:timeline".to_string(), "issue:comments".to_string()],
        ),
        CompetitionBand::Light => GateVerdict::new(
            GateStatus::Pass,
            GateBand::Acceptable,
            vec![format!(
                "Competition evidence is light ({} points)",
                facts.competition_points
            )],
            vec!["issue:timeline".to_string(), "issue:comments".to_string()],
        ),
        CompetitionBand::Contested => GateVerdict::new(
            GateStatus::SoftFail,
            GateBand::Contested,
            vec![format!(
                "Competition evidence is contested ({} points: {} open PR refs, {} closed PR refs, {} attempt/claim/working comments)",
                facts.competition_points,
                facts.open_pr_refs,
                facts.closed_pr_refs,
                facts.attempt_comments + facts.claim_comments + facts.working_comments + facts.fix_submitted_comments
            )],
            vec!["issue:timeline".to_string(), "issue:comments".to_string()],
        ),
        CompetitionBand::Saturated => GateVerdict::new(
            GateStatus::HardFail,
            GateBand::Saturated,
            vec![format!(
                "Competition evidence is saturated ({} points: {} open PR refs, {} closed PR refs, {} attempt/claim/working comments)",
                facts.competition_points,
                facts.open_pr_refs,
                facts.closed_pr_refs,
                facts.attempt_comments + facts.claim_comments + facts.working_comments + facts.fix_submitted_comments
            )],
            vec!["issue:timeline".to_string(), "issue:comments".to_string()],
        ),
    }
}

pub fn profile_fit_gate(fit: &ProfileFitAssessment) -> GateVerdict {
    if fit.score >= 75 {
        GateVerdict::new(
            GateStatus::Pass,
            GateBand::Strong,
            fit.reasons.clone(),
            fit.evidence_refs.clone(),
        )
    } else if fit.score >= 60 {
        GateVerdict::new(
            GateStatus::Pass,
            GateBand::Acceptable,
            fit.reasons.clone(),
            fit.evidence_refs.clone(),
        )
    } else if fit.score >= 40 {
        GateVerdict::new(
            GateStatus::SoftFail,
            GateBand::Weak,
            fit.reasons.clone(),
            fit.evidence_refs.clone(),
        )
    } else {
        GateVerdict::new(
            GateStatus::HardFail,
            GateBand::Weak,
            fit.reasons.clone(),
            fit.evidence_refs.clone(),
        )
    }
}

pub fn low_depth_matches(enriched: &EnrichedIssue) -> Vec<String> {
    let text = normalized_issue_and_repo_text(enriched);
    let mut matches = Vec::new();
    for term in [
        "no code required",
        "no coding required",
        "no prerequisites needed",
        "do not need to clone",
        "don t need to clone",
        "browser in under",
        "under 60 seconds",
        "under 1 minute",
        "less than 1 minute",
        "phone only",
        "mobile only",
        "add json content",
        "json content",
        "add japanese proverb",
        "japanese proverb",
        "add new trivia",
        "trivia question",
        "grammar point",
        "glossary",
        "related terms",
        "content contribution",
        "content only",
        "add jsdoc",
        "jsdoc comments",
        "add concise comments",
        "add comments",
        "short summary",
    ] {
        if text.contains(term) {
            matches.push(term.to_string());
        }
    }
    matches.sort();
    matches.dedup();
    matches
}

pub fn marketplace_terms(enriched: &EnrichedIssue) -> Vec<String> {
    let text = normalized_issue_and_repo_text(enriched);
    let mut matches = Vec::new();
    for term in [
        "bounty hunters",
        "bounty hunter",
        "ai agent friendly",
        "agent friendly",
        "ai only",
        "agent only",
        "bounty farm",
        "marketplace queue",
        "claim reward",
        "reward pool",
    ] {
        if text.contains(term) {
            matches.push(term.to_string());
        }
    }
    matches.sort();
    matches.dedup();
    matches
}

pub fn fork_star_ratio(stars: u64, forks: u64) -> f64 {
    forks as f64 / stars.max(1) as f64
}

pub fn repo_age_days(created_at: Option<&str>) -> Option<i64> {
    let created_at = created_at?;
    let parsed = DateTime::parse_from_rfc3339(created_at).ok()?;
    Some((Utc::now() - parsed.with_timezone(&Utc)).num_days())
}

fn normalized_issue_and_repo_text(enriched: &EnrichedIssue) -> String {
    normalize(&format!(
        "{} {} {} {} {} {}",
        enriched.issue.title,
        enriched.issue.body,
        enriched.issue.labels.join(" "),
        enriched.repository.full_name,
        enriched.repository.description,
        enriched.repository.topics.join(" ")
    ))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{low_depth_gate, repo_influence_gate};
    use crate::github::GitHubIssue;
    use crate::github_enrichment::EnrichedIssue;
    use crate::value_model::{GateBand, GateStatus};

    fn enriched(stars: u64, forks: u64, body: &str) -> EnrichedIssue {
        let issue = GitHubIssue {
            id: 1,
            number: 1,
            title: "Issue".to_string(),
            body: body.to_string(),
            labels: vec![],
            url: "https://github.com/owner/repo/issues/1".to_string(),
            repo_full_name: "owner/repo".to_string(),
            repo_name: "repo".to_string(),
            repo_description: "Rust CLI".to_string(),
            repo_stars: stars,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };
        let mut enriched = EnrichedIssue::from_issue(&issue);
        enriched.repository.stars = stars;
        enriched.repository.forks = forks;
        enriched.repository.created_at = Some("2020-01-01T00:00:00Z".to_string());
        enriched
    }

    #[test]
    fn low_depth_task_hard_fails_gate() {
        let gate = low_depth_gate(&enriched(
            2_000,
            100,
            "No code required. Browser in under 60 seconds. Add JSON content.",
        ));
        assert_eq!(gate.status, GateStatus::HardFail);
    }

    #[test]
    fn fork_star_anomaly_hard_fails_repo_gate() {
        let gate = repo_influence_gate(&enriched(50, 400, "Fix parser"));
        assert_eq!(gate.status, GateStatus::HardFail);
        assert_eq!(gate.band, GateBand::Suspicious);
    }
}
