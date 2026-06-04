use serde::{Deserialize, Serialize};
use std::fmt;

use crate::scoring::normalize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CompetitionFacts {
    #[serde(default)]
    pub open_pr_refs: usize,
    #[serde(default)]
    pub closed_pr_refs: usize,
    #[serde(default)]
    pub attempt_comments: usize,
    #[serde(default)]
    pub claim_comments: usize,
    #[serde(default)]
    pub working_comments: usize,
    #[serde(default)]
    pub fix_submitted_comments: usize,
    #[serde(default)]
    pub latest_competition_at: Option<String>,
    #[serde(default)]
    pub competition_points: i32,
    #[serde(default)]
    pub competition_band: CompetitionBand,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl CompetitionFacts {
    pub fn missing_timeline() -> Self {
        Self {
            warnings: vec!["Competition timeline evidence was not fetched".to_string()],
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum CompetitionBand {
    #[default]
    Clear,
    Light,
    Contested,
    Saturated,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimelineIssueReference {
    pub source_ref: String,
    pub state: Option<String>,
    pub is_pull_request: bool,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommentCompetitionMarkers {
    pub attempt: bool,
    pub claim: bool,
    pub working: bool,
    pub fix_submitted: bool,
}

pub fn assess_competition(
    timeline_refs: &[TimelineIssueReference],
    comment_bodies: &[String],
    warnings: Vec<String>,
) -> CompetitionFacts {
    let open_pr_refs = timeline_refs
        .iter()
        .filter(|item| item.is_pull_request && is_open(item.state.as_deref()))
        .count();
    let closed_pr_refs = timeline_refs
        .iter()
        .filter(|item| item.is_pull_request && !is_open(item.state.as_deref()))
        .count();

    let mut attempt_comments = 0usize;
    let mut claim_comments = 0usize;
    let mut working_comments = 0usize;
    let mut fix_submitted_comments = 0usize;
    for body in comment_bodies {
        let markers = detect_comment_competition_markers(body);
        attempt_comments += usize::from(markers.attempt);
        claim_comments += usize::from(markers.claim);
        working_comments += usize::from(markers.working);
        fix_submitted_comments += usize::from(markers.fix_submitted);
    }

    let competition_points = (open_pr_refs * 3
        + closed_pr_refs
        + attempt_comments
        + claim_comments
        + working_comments
        + fix_submitted_comments) as i32;

    CompetitionFacts {
        open_pr_refs,
        closed_pr_refs,
        attempt_comments,
        claim_comments,
        working_comments,
        fix_submitted_comments,
        latest_competition_at: timeline_refs
            .iter()
            .filter_map(|item| item.created_at.clone())
            .max(),
        competition_points,
        competition_band: competition_band(competition_points),
        warnings,
    }
}

pub fn detect_comment_competition_markers(body: &str) -> CommentCompetitionMarkers {
    let normalized = normalize(body);
    CommentCompetitionMarkers {
        attempt: body.contains("/attempt") || normalized.contains(" attempt "),
        claim: body.contains("/claim") || normalized.contains(" claim "),
        working: normalized.contains("working on this")
            || normalized.contains("i am working on this")
            || normalized.contains("i m working on this"),
        fix_submitted: normalized.contains("fix submitted in pr")
            || normalized.contains("submitted in pr")
            || normalized.contains("opened a pr")
            || normalized.contains("pull request submitted"),
    }
}

pub fn competition_band(points: i32) -> CompetitionBand {
    match points {
        value if value <= 1 => CompetitionBand::Clear,
        value if value <= 3 => CompetitionBand::Light,
        value if value <= 7 => CompetitionBand::Contested,
        _ => CompetitionBand::Saturated,
    }
}

fn is_open(state: Option<&str>) -> bool {
    state
        .map(|value| value.eq_ignore_ascii_case("open"))
        .unwrap_or(false)
}

impl fmt::Display for CompetitionBand {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Clear => "clear",
            Self::Light => "light",
            Self::Contested => "contested",
            Self::Saturated => "saturated",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        assess_competition, detect_comment_competition_markers, CompetitionBand,
        TimelineIssueReference,
    };

    #[test]
    fn bands_competition_points() {
        let refs = vec![
            TimelineIssueReference {
                source_ref: "issue:timeline.0".to_string(),
                state: Some("open".to_string()),
                is_pull_request: true,
                created_at: Some("2026-01-01T00:00:00Z".to_string()),
            },
            TimelineIssueReference {
                source_ref: "issue:timeline.1".to_string(),
                state: Some("closed".to_string()),
                is_pull_request: true,
                created_at: Some("2026-01-02T00:00:00Z".to_string()),
            },
        ];
        let comments = vec!["/attempt\nWorking on this".to_string()];
        let facts = assess_competition(&refs, &comments, Vec::new());
        assert_eq!(facts.competition_points, 6);
        assert_eq!(facts.competition_band, CompetitionBand::Contested);
        assert_eq!(
            facts.latest_competition_at,
            Some("2026-01-02T00:00:00Z".to_string())
        );
    }

    #[test]
    fn detects_attempt_and_claim_markers() {
        let markers = detect_comment_competition_markers("/claim\nFix submitted in PR #12");
        assert!(markers.claim);
        assert!(markers.fix_submitted);
        assert!(!markers.working);
    }
}
