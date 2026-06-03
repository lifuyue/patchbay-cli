use chrono::{Duration, Utc};
use patchbay_cli::config::ProfileConfig;
use patchbay_cli::github::GitHubIssue;
use patchbay_cli::github_enrichment::{EnrichedIssue, TimestampedSample};
use patchbay_cli::value_scoring::{
    aggregate_signals, assess_issue, GrowthConfidence, OpportunityType, Recommendation,
};
use patchbay_cli::value_signals::{ValueSignal, ValueSignalKind};

#[test]
fn classifies_established_project_from_visible_impact() {
    let enriched = fixture()
        .stars(2_500)
        .forks(220)
        .recent_repo_activity()
        .good_first_actionable_issue()
        .build();

    let assessment = assess_issue(&enriched, &matching_profile());

    assert_eq!(
        assessment.opportunity_type,
        OpportunityType::EstablishedProject
    );
    assert!(is_at_least_candidate(&assessment.recommendation));
    assert_has_signal(&assessment.signals, ValueSignalKind::EstablishedImpact);
}

#[test]
fn classifies_growth_project_from_recent_attention_sample() {
    let enriched = fixture()
        .stars(80)
        .recent_repo_activity()
        .good_first_actionable_issue()
        .recent_stargazers(16)
        .build();

    let assessment = assess_issue(&enriched, &non_matching_profile());

    assert_eq!(assessment.opportunity_type, OpportunityType::GrowthProject);
    assert_has_signal(&assessment.signals, ValueSignalKind::GrowthMomentum);
    assert_ne!(assessment.growth_confidence, GrowthConfidence::Low);
}

#[test]
fn classifies_balanced_project_when_impact_and_growth_are_strong() {
    let enriched = fixture()
        .stars(2_500)
        .forks(220)
        .recent_repo_activity()
        .good_first_actionable_issue()
        .recent_stargazers(16)
        .build();

    let assessment = assess_issue(&enriched, &matching_profile());

    assert_eq!(assessment.opportunity_type, OpportunityType::Balanced);
    assert_has_signal(&assessment.signals, ValueSignalKind::EstablishedImpact);
    assert_has_signal(&assessment.signals, ValueSignalKind::GrowthMomentum);
}

#[test]
fn classifies_niche_but_actionable_when_fit_and_gate_are_strong() {
    let enriched = fixture()
        .recent_repo_activity()
        .good_first_actionable_issue()
        .topics(["rust", "cli", "parser"])
        .build();

    let assessment = assess_issue(&enriched, &matching_profile());

    assert_eq!(
        assessment.opportunity_type,
        OpportunityType::NicheButActionable
    );
    assert!(assessment.execution_gate_score >= 60);
    assert!(is_at_least_candidate(&assessment.recommendation));
}

#[test]
fn classifies_low_signal_when_value_evidence_is_missing() {
    let enriched = fixture().stale_issue().stale_repo().vague_issue().build();

    let assessment = assess_issue(&enriched, &non_matching_profile());

    assert_eq!(assessment.opportunity_type, OpportunityType::LowSignal);
    assert!(matches!(
        assessment.recommendation,
        Recommendation::WeakCandidate | Recommendation::Avoid
    ));
    assert!(assessment
        .missing_evidence
        .iter()
        .any(|item| item.contains("stargazer sample")));
    assert!(assessment
        .missing_evidence
        .iter()
        .any(|item| item.contains("fork sample")));
}

#[test]
fn avoids_high_impact_issue_when_execution_gate_is_low() {
    let enriched = fixture()
        .stars(12_000)
        .forks(1_100)
        .recent_repo_activity()
        .vague_issue()
        .build();

    let assessment = assess_issue(&enriched, &non_matching_profile());

    assert!(assessment.execution_gate_score < 40);
    assert!(matches!(
        assessment.recommendation,
        Recommendation::Avoid | Recommendation::WeakCandidate
    ));
    assert_eq!(
        assessment.opportunity_type,
        OpportunityType::EstablishedProject
    );
}

#[test]
fn exposes_staleness_and_noise_risks() {
    let enriched = fixture()
        .stars(2_500)
        .good_first_actionable_issue()
        .stale_issue()
        .stale_repo()
        .comments_count(60)
        .open_issues(1_200)
        .build();

    let assessment = assess_issue(&enriched, &matching_profile());

    assert_has_signal(&assessment.signals, ValueSignalKind::StalenessRisk);
    assert_has_signal(&assessment.signals, ValueSignalKind::NoiseRisk);
    assert!(assessment
        .risks
        .iter()
        .any(|risk| risk.contains("not recent")));
    assert!(assessment
        .risks
        .iter()
        .any(|risk| risk.contains("extra triage")));
}

#[test]
fn profile_fit_improves_score_and_explanation() {
    let enriched = fixture()
        .recent_repo_activity()
        .good_first_actionable_issue()
        .topics(["rust", "cli", "parser"])
        .build();

    let matching = assess_issue(&enriched, &matching_profile());
    let non_matching = assess_issue(&enriched, &non_matching_profile());

    assert_has_signal(&matching.signals, ValueSignalKind::IssueFit);
    assert!(!non_matching
        .signals
        .iter()
        .any(|signal| signal.kind == ValueSignalKind::IssueFit));
    assert!(matching.value_score > non_matching.value_score);
    assert!(matching
        .explanation
        .iter()
        .any(|item| item.contains("Matched profile terms")));
}

#[test]
fn covers_recommendation_threshold_behaviors() {
    let strong = aggregate_signals(
        vec![
            signal(ValueSignalKind::EstablishedImpact, 24),
            signal(ValueSignalKind::GrowthMomentum, 22),
            signal(ValueSignalKind::ExecutionReadiness, 16),
            signal(ValueSignalKind::IssueClarity, 14),
        ],
        &fixture().build(),
    );
    assert_eq!(strong.recommendation, Recommendation::StrongCandidate);

    let candidate = aggregate_signals(
        vec![
            signal(ValueSignalKind::EstablishedImpact, 24),
            signal(ValueSignalKind::IssueFit, 15),
            signal(ValueSignalKind::ExecutionReadiness, 10),
        ],
        &fixture().build(),
    );
    assert_eq!(candidate.recommendation, Recommendation::Candidate);

    let weak = aggregate_signals(
        vec![
            signal(ValueSignalKind::EstablishedImpact, 24),
            signal(ValueSignalKind::IssueClarity, 10),
        ],
        &fixture().build(),
    );
    assert_eq!(weak.recommendation, Recommendation::WeakCandidate);

    let avoid = aggregate_signals(
        vec![signal(ValueSignalKind::EstablishedImpact, 24)],
        &fixture().build(),
    );
    assert_eq!(avoid.recommendation, Recommendation::Avoid);
}

fn fixture() -> EnrichedIssueFixture {
    EnrichedIssueFixture::default()
}

#[derive(Debug, Clone)]
struct EnrichedIssueFixture {
    title: String,
    body: String,
    labels: Vec<String>,
    repo_description: String,
    repo_stars: u64,
    forks: u64,
    subscribers: Option<u64>,
    open_issues: Option<u64>,
    issue_updated_at: String,
    repo_pushed_at: Option<String>,
    topics: Vec<String>,
    comments_count: u64,
    recent_stargazers: usize,
    recent_forks: usize,
}

impl Default for EnrichedIssueFixture {
    fn default() -> Self {
        Self {
            title: "Fix Rust CLI parser".to_string(),
            body: actionable_body(),
            labels: vec!["good first issue".to_string()],
            repo_description: "Rust CLI developer tools".to_string(),
            repo_stars: 0,
            forks: 0,
            subscribers: None,
            open_issues: Some(12),
            issue_updated_at: recent_timestamp(),
            repo_pushed_at: Some(recent_timestamp()),
            topics: vec!["rust".to_string(), "cli".to_string()],
            comments_count: 1,
            recent_stargazers: 0,
            recent_forks: 0,
        }
    }
}

impl EnrichedIssueFixture {
    fn stars(mut self, value: u64) -> Self {
        self.repo_stars = value;
        self
    }

    fn forks(mut self, value: u64) -> Self {
        self.forks = value;
        self
    }

    fn open_issues(mut self, value: u64) -> Self {
        self.open_issues = Some(value);
        self
    }

    fn comments_count(mut self, value: u64) -> Self {
        self.comments_count = value;
        self
    }

    fn topics<const N: usize>(mut self, values: [&str; N]) -> Self {
        self.topics = values.into_iter().map(ToOwned::to_owned).collect();
        self
    }

    fn recent_repo_activity(mut self) -> Self {
        self.repo_pushed_at = Some(recent_timestamp());
        self
    }

    fn stale_repo(mut self) -> Self {
        self.repo_pushed_at = Some(stale_timestamp());
        self
    }

    fn stale_issue(mut self) -> Self {
        self.issue_updated_at = stale_timestamp();
        self
    }

    fn good_first_actionable_issue(mut self) -> Self {
        self.title = "Fix Rust CLI parser regression".to_string();
        self.body = actionable_body();
        self.labels = vec!["good first issue".to_string()];
        self
    }

    fn vague_issue(mut self) -> Self {
        self.title = "Needs investigation".to_string();
        self.body = "Something is wrong.".to_string();
        self.labels = Vec::new();
        self.topics = Vec::new();
        self
    }

    fn recent_stargazers(mut self, count: usize) -> Self {
        self.recent_stargazers = count;
        self
    }

    fn build(self) -> EnrichedIssue {
        let issue = GitHubIssue {
            id: 1,
            number: 1,
            title: self.title,
            body: self.body,
            labels: self.labels,
            url: "https://github.com/owner/repo/issues/1".to_string(),
            repo_full_name: "owner/repo".to_string(),
            repo_name: "repo".to_string(),
            repo_description: self.repo_description,
            repo_stars: self.repo_stars,
            created_at: self.issue_updated_at.clone(),
            updated_at: self.issue_updated_at,
        };
        let mut enriched = EnrichedIssue::from_issue(&issue);
        enriched.repository.stars = issue.repo_stars;
        enriched.repository.forks = self.forks;
        enriched.repository.subscribers = self.subscribers;
        enriched.repository.open_issues = self.open_issues;
        enriched.repository.pushed_at = self.repo_pushed_at.clone();
        enriched.repository.topics = self.topics;
        enriched.issue.comments_count = self.comments_count;
        enriched.activity.recent_issue_activity = is_recent_timestamp(&enriched.issue.updated_at);
        enriched.activity.recent_repo_activity = self
            .repo_pushed_at
            .as_deref()
            .map(is_recent_timestamp)
            .unwrap_or(false);
        enriched.activity.maintainer_recent_response = false;
        enriched.growth.recent_stargazer_sample =
            timestamp_samples("repo:stargazers.sample_recent_100", self.recent_stargazers);
        enriched.growth.newest_fork_sample =
            timestamp_samples("repo:forks.sample_newest_100", self.recent_forks);
        enriched
    }
}

fn matching_profile() -> ProfileConfig {
    ProfileConfig {
        tech_stack: vec!["Rust".to_string()],
        keywords: vec!["cli".to_string(), "parser".to_string()],
    }
}

fn non_matching_profile() -> ProfileConfig {
    ProfileConfig {
        tech_stack: vec!["Python".to_string()],
        keywords: vec!["database".to_string()],
    }
}

fn actionable_body() -> String {
    "The parser currently panics when a subcommand contains repeated flags. Expected behavior is a graceful error in src/main.rs, actual behavior is a panic with a short stack trace. Steps to reproduce are included and the fix should be small.".to_string()
}

fn recent_timestamp() -> String {
    Utc::now().to_rfc3339()
}

fn stale_timestamp() -> String {
    (Utc::now() - Duration::days(90)).to_rfc3339()
}

fn is_recent_timestamp(value: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| Utc::now() - timestamp.with_timezone(&Utc) <= Duration::days(14))
        .unwrap_or(false)
}

fn timestamp_samples(prefix: &str, count: usize) -> Vec<TimestampedSample> {
    (0..count)
        .map(|index| TimestampedSample {
            source_ref: format!("{prefix}.{index}"),
            actor: Some(format!("actor-{index}")),
            timestamp: Some(recent_timestamp()),
        })
        .collect()
}

fn signal(kind: ValueSignalKind, delta: i32) -> ValueSignal {
    ValueSignal {
        kind,
        score_delta: delta,
        confidence: patchbay_cli::value_signals::SignalConfidence::High,
        summary: "summary".to_string(),
        evidence_refs: vec!["issue:body".to_string()],
    }
}

fn assert_has_signal(signals: &[ValueSignal], kind: ValueSignalKind) {
    assert!(
        signals.iter().any(|signal| signal.kind == kind),
        "expected signal {kind:?}, got {signals:?}"
    );
}

fn is_at_least_candidate(recommendation: &Recommendation) -> bool {
    matches!(
        recommendation,
        Recommendation::StrongCandidate | Recommendation::Candidate
    )
}
