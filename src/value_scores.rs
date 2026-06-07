use chrono::{DateTime, Utc};

use crate::config::ProfileConfig;
use crate::github_enrichment::EnrichedIssue;
use crate::scoring::normalize;
use crate::value_model::{GateStatus, RecommendationCategory, RiskTag, ScoreBand, ValueScores};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileFitAssessment {
    pub score: i32,
    pub reasons: Vec<String>,
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionQualityAssessment {
    pub score: i32,
    pub reasons: Vec<String>,
    pub evidence_refs: Vec<String>,
}

pub fn value_scores(
    enriched: &EnrichedIssue,
    profile: &ProfileConfig,
    risk_tags: &[RiskTag],
    repo_gate_status: GateStatus,
) -> (
    ValueScores,
    ProfileFitAssessment,
    ExecutionQualityAssessment,
) {
    let profile_fit = profile_fit_assessment(enriched, profile, risk_tags);
    let execution_quality = execution_quality_assessment(enriched, risk_tags);
    let scores = ValueScores {
        repo_influence_score: repo_influence_score(enriched, repo_gate_status),
        profile_fit_score: profile_fit.score,
        execution_quality_score: execution_quality.score,
        maintainer_signal_score: maintainer_signal_score(enriched),
        freshness_score: freshness_score(&enriched.issue.updated_at),
        risk_score: risk_score(risk_tags),
    };
    (scores, profile_fit, execution_quality)
}

pub fn repo_influence_score(enriched: &EnrichedIssue, repo_gate_status: GateStatus) -> i32 {
    let stars = enriched.repository.stars;
    let forks = enriched.repository.forks;
    let watchers = enriched.repository.subscribers.unwrap_or(0);

    let star_score = match stars {
        value if value >= 10_000 => 100,
        value if value >= 5_000 => 92,
        value if value >= 1_000 => 82,
        value if value >= 500 => 68,
        value if value >= 100 => 45,
        value if value >= 25 => 22,
        _ => 8,
    };
    let fork_score = match forks {
        value if value >= 1_000 => 100,
        value if value >= 500 => 84,
        value if value >= 200 => 70,
        value if value >= 50 => 45,
        value if value >= 10 => 22,
        _ => 6,
    };
    let watcher_score = match watchers {
        value if value >= 200 => 100,
        value if value >= 50 => 78,
        value if value >= 20 => 62,
        value if value >= 10 => 45,
        value if value >= 1 => 18,
        _ => 0,
    };

    let score =
        ((star_score as f64 * 0.65) + (fork_score as f64 * 0.20) + (watcher_score as f64 * 0.15))
            .round() as i32;

    match repo_gate_status {
        GateStatus::Pass => score,
        GateStatus::SoftFail => score.min(55),
        GateStatus::HardFail => score.min(25),
    }
}

pub fn profile_fit_assessment(
    enriched: &EnrichedIssue,
    profile: &ProfileConfig,
    risk_tags: &[RiskTag],
) -> ProfileFitAssessment {
    let issue_text = normalize(&format!(
        "{} {} {}",
        enriched.issue.title,
        enriched.issue.body,
        enriched.issue.labels.join(" ")
    ));
    let repo_text = normalize(&format!(
        "{} {} {} {}",
        enriched.repository.full_name,
        enriched.repository.description,
        enriched.repository.topics.join(" "),
        enriched.repository.language.clone().unwrap_or_default()
    ));
    let profile_terms = profile_terms(profile);
    let mut score = 0;
    let mut reasons = Vec::new();
    let mut evidence_refs = Vec::new();
    let mut strong_issue_evidence = false;

    let issue_matches = matching_terms(&issue_text, &profile_terms);
    if !issue_matches.is_empty() {
        score += (issue_matches.len() as i32 * 22).min(50);
        strong_issue_evidence = true;
        reasons.push(format!(
            "Issue task matches profile term(s): {}",
            issue_matches.join(", ")
        ));
        evidence_refs.push("issue:title".to_string());
        evidence_refs.push("issue:body".to_string());
    }

    let repo_matches = matching_terms(&repo_text, &profile_terms);
    if !repo_matches.is_empty() {
        score += (repo_matches.len() as i32 * 12).min(38);
        reasons.push(format!(
            "Repository context matches profile term(s): {}",
            repo_matches.join(", ")
        ));
        evidence_refs.push("repo:description".to_string());
        evidence_refs.push("repo:topics".to_string());
    }

    if language_matches_profile(enriched, &profile_terms) {
        score += 14;
        reasons.push("Repository language matches the configured profile".to_string());
        evidence_refs.push("repo:language".to_string());
    }

    let domain_matches = matching_terms(&issue_text, &profile_domain_terms(profile));
    if !domain_matches.is_empty() {
        strong_issue_evidence = true;
        score += (domain_matches.len() as i32 * 12).min(28);
        reasons.push(format!(
            "Issue domain signal matches profile: {}",
            domain_matches.join(", ")
        ));
        evidence_refs.push("issue:title".to_string());
        evidence_refs.push("issue:body".to_string());
    }

    if strong_issue_evidence && repo_matches.len() >= 2 {
        score += 16;
        reasons.push("Issue-level evidence is reinforced by repository context".to_string());
        evidence_refs.push("repo:topics".to_string());
    }

    if wants_cli_or_devtools(profile)
        && looks_like_cli_or_devtool(&issue_text, &repo_text)
        && cli_devtool_boost_allowed(enriched, profile, &issue_text, &repo_text)
    {
        score += 45;
        reasons.push("CLI/developer-tool domain fits the configured profile".to_string());
        evidence_refs.push("profile:keywords".to_string());
        evidence_refs.push("repo:description".to_string());
    }

    apply_profile_focus_caps(
        enriched,
        profile,
        &issue_text,
        &repo_text,
        &mut score,
        &mut reasons,
        &mut evidence_refs,
    );

    if lacks_security_profile(profile) && looks_like_crypto_security(&issue_text, &repo_text) {
        score = score.min(35);
        reasons.push("Crypto/security task is outside the configured profile".to_string());
        evidence_refs.push("repo:topics".to_string());
    }

    if risk_tags.iter().any(is_low_depth_tag) {
        score = score.min(25);
        reasons.push("Content-only or no-code task weakens issue-level profile fit".to_string());
        evidence_refs.push("issue:body".to_string());
    }

    if reasons.is_empty() {
        reasons.push("No strong issue-level profile fit evidence found".to_string());
    }

    evidence_refs.sort();
    evidence_refs.dedup();

    ProfileFitAssessment {
        score: score.clamp(0, 100),
        reasons,
        evidence_refs,
    }
}

pub fn execution_quality_assessment(
    enriched: &EnrichedIssue,
    risk_tags: &[RiskTag],
) -> ExecutionQualityAssessment {
    let text = normalize(&format!(
        "{} {} {}",
        enriched.issue.title,
        enriched.issue.body,
        enriched.issue.labels.join(" ")
    ));
    let mut score = 0;
    let mut reasons = Vec::new();
    let mut evidence_refs = Vec::new();
    let body_len = enriched.issue.body.trim().len();

    if body_len >= 700 {
        score += 25;
        reasons.push("Issue body has substantial implementation detail".to_string());
        evidence_refs.push("issue:body".to_string());
    } else if body_len >= 250 {
        score += 20;
        reasons.push("Issue body has enough detail to begin investigation".to_string());
        evidence_refs.push("issue:body".to_string());
    } else if body_len >= 120 {
        score += 14;
        reasons.push("Issue body provides some actionable context".to_string());
        evidence_refs.push("issue:body".to_string());
    }

    if has_file_path_reference(&enriched.issue.body) {
        score += 16;
        reasons.push("Issue references a likely code path or file".to_string());
        evidence_refs.push("issue:body".to_string());
    }
    if contains_any(&text, &["steps to reproduce", "step to reproduce", "repro"]) {
        score += 20;
        reasons.push("Issue includes reproduction guidance".to_string());
        evidence_refs.push("issue:body".to_string());
    }
    if contains_any(
        &text,
        &[
            "expected behavior",
            "expected behaviour",
            "expected",
            "actual behavior",
            "actual behaviour",
            "actual",
        ],
    ) {
        score += 15;
        reasons.push("Issue states expected or actual behavior".to_string());
        evidence_refs.push("issue:body".to_string());
    }
    if contains_any(
        &text,
        &[
            "suggested fix",
            "suggested solution",
            "fix should",
            "proposal",
        ],
    ) {
        score += 12;
        reasons.push("Issue includes implementation direction".to_string());
        evidence_refs.push("issue:body".to_string());
    }
    if has_validation_hint(&text) {
        score += 12;
        reasons.push("Issue includes validation or test guidance".to_string());
        evidence_refs.push("issue:body".to_string());
    }
    if enriched
        .issue
        .labels
        .iter()
        .any(|label| normalize(label).contains("good first issue"))
    {
        score += 6;
        reasons.push("Issue carries a good-first-issue label".to_string());
        evidence_refs.push("issue:labels".to_string());
    }

    if risk_tags.iter().any(is_low_depth_tag) {
        score = score.min(25);
        reasons.push("Low-depth content task caps execution quality".to_string());
        evidence_refs.push("issue:body".to_string());
    }
    if risk_tags.contains(&RiskTag::ThinTask) {
        score = score.min(45);
    }
    if risk_tags.contains(&RiskTag::WeakValidationPath) {
        score = score.min(82);
    }

    if reasons.is_empty() {
        reasons.push("Issue lacks enough concrete execution detail".to_string());
    }

    evidence_refs.sort();
    evidence_refs.dedup();

    ExecutionQualityAssessment {
        score: score.clamp(0, 100),
        reasons,
        evidence_refs,
    }
}

pub fn maintainer_signal_score(enriched: &EnrichedIssue) -> i32 {
    let mut score = 0;
    if is_maintainer_association(&enriched.issue.author_association) {
        score += 60;
    }
    if enriched.activity.maintainer_recent_response {
        score += 35;
    } else if !enriched.participants.maintainer_commenters.is_empty() {
        score += 25;
    }
    score.clamp(0, 100)
}

pub fn freshness_score(updated_at: &str) -> i32 {
    let Ok(updated_at) = DateTime::parse_from_rfc3339(updated_at) else {
        return 20;
    };
    let age_days = (Utc::now() - updated_at.with_timezone(&Utc)).num_days();
    match age_days {
        value if value <= 7 => 100,
        value if value <= 30 => 75,
        value if value <= 90 => 45,
        value if value <= 180 => 25,
        _ => 10,
    }
}

pub fn risk_score(tags: &[RiskTag]) -> i32 {
    tags.iter()
        .map(|tag| match tag {
            RiskTag::NoCodeRequired => 35,
            RiskTag::MicroContribution => 30,
            RiskTag::ContentFill => 30,
            RiskTag::TemplateLike => 22,
            RiskTag::EventNoise => 18,
            RiskTag::ThinTask => 22,
            RiskTag::HighTriageLoad => 25,
            RiskTag::MissingMaintainerSignal => 8,
            RiskTag::WeakValidationPath => 10,
            RiskTag::LowTrustRepo => 45,
            RiskTag::LowImpactRepo => 20,
            RiskTag::ForkStarAnomaly => 35,
            RiskTag::MarketplaceNoise => 42,
            RiskTag::CompetitionContested => 25,
            RiskTag::CompetitionSaturated => 55,
            RiskTag::CompetitionEvidenceMissing => 18,
            RiskTag::ProfileMismatch => 25,
            RiskTag::ScopeRisk => 15,
        })
        .sum::<i32>()
        .clamp(0, 100)
}

pub fn rank_score(category: RecommendationCategory, scores: &ValueScores) -> i32 {
    let value = match category {
        RecommendationCategory::HighValueReady | RecommendationCategory::HighValueNeedsScoping => {
            scores.profile_fit_score as f64 * 0.35
                + scores.execution_quality_score as f64 * 0.25
                + scores.repo_influence_score as f64 * 0.20
                + scores.maintainer_signal_score as f64 * 0.10
                + scores.freshness_score as f64 * 0.05
                - scores.risk_score as f64 * 0.25
        }
        RecommendationCategory::NicheButActionable => {
            scores.profile_fit_score as f64 * 0.40
                + scores.execution_quality_score as f64 * 0.35
                + scores.maintainer_signal_score as f64 * 0.10
                + scores.repo_influence_score as f64 * 0.10
                + scores.freshness_score as f64 * 0.05
                - scores.risk_score as f64 * 0.20
        }
        RecommendationCategory::ContestedOrLowTrust => {
            scores.repo_influence_score as f64 * 0.20
                + scores.profile_fit_score as f64 * 0.25
                + scores.execution_quality_score as f64 * 0.25
                + scores.freshness_score as f64 * 0.05
                - scores.risk_score as f64 * 0.35
        }
        RecommendationCategory::NeedsTriage => {
            scores.repo_influence_score as f64 * 0.20
                + scores.profile_fit_score as f64 * 0.20
                + scores.execution_quality_score as f64 * 0.25
                + scores.freshness_score as f64 * 0.05
                - scores.risk_score as f64 * 0.20
        }
        RecommendationCategory::FilteredLowDepth => 0.0,
    };
    value.round().clamp(0.0, 100.0) as i32
}

pub fn score_band(score: i32) -> ScoreBand {
    if score >= 70 {
        ScoreBand::High
    } else if score >= 30 {
        ScoreBand::Medium
    } else {
        ScoreBand::Low
    }
}

pub fn has_validation_hint(text: &str) -> bool {
    contains_any(
        text,
        &[
            "test",
            "tests",
            "testing",
            "verify",
            "validation",
            "coverage",
            "chrome devtools",
            "emulate",
            "reproduce",
            "reproduction",
        ],
    )
}

pub fn has_file_path_reference(text: &str) -> bool {
    text.split_whitespace()
        .map(|token| {
            token
                .trim_matches(|ch: char| {
                    !ch.is_ascii_alphanumeric() && ch != '/' && ch != '.' && ch != '-' && ch != '_'
                })
                .trim_end_matches(['.', ',', ';', ':', ')', ']'])
        })
        .any(|token| {
            token.contains('/')
                && [
                    ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".md", ".json", ".css",
                    ".scss", ".html", ".sql", ".toml", ".yaml", ".yml",
                ]
                .iter()
                .any(|suffix| token.ends_with(suffix))
        })
}

pub fn is_low_depth_tag(tag: &RiskTag) -> bool {
    matches!(
        tag,
        RiskTag::NoCodeRequired
            | RiskTag::MicroContribution
            | RiskTag::ContentFill
            | RiskTag::ThinTask
    )
}

fn matching_terms(searchable: &str, terms: &[String]) -> Vec<String> {
    let tokens = searchable.split_whitespace().collect::<Vec<_>>();
    let mut matched = Vec::new();
    for term in terms {
        let term_tokens = term.split_whitespace().collect::<Vec<_>>();
        let is_match = if term.len() < 3 {
            tokens.iter().any(|token| *token == term)
        } else if term_tokens.len() == 1 {
            tokens.iter().any(|token| *token == term)
        } else {
            searchable.contains(term)
        };
        if is_match && !matched.contains(term) {
            matched.push(term.clone());
        }
    }
    matched.sort();
    matched
}

fn profile_terms(profile: &ProfileConfig) -> Vec<String> {
    let mut terms = Vec::new();
    for item in profile.tech_stack.iter().chain(profile.keywords.iter()) {
        let normalized = normalize(item);
        if !normalized.is_empty() {
            terms.push(normalized.clone());
        }
        terms.extend(aliases(&normalized).iter().map(|alias| alias.to_string()));
    }
    terms.sort();
    terms.dedup();
    terms
}

fn profile_domain_terms(profile: &ProfileConfig) -> Vec<String> {
    let mut terms = Vec::new();
    for item in profile.tech_stack.iter().chain(profile.keywords.iter()) {
        let normalized = normalize(item);
        terms.extend(
            domain_aliases(&normalized)
                .iter()
                .map(|alias| alias.to_string()),
        );
    }
    terms.sort();
    terms.dedup();
    terms
}

fn aliases(term: &str) -> &'static [&'static str] {
    match term {
        "typescript" => &["ts", "tsx"],
        "javascript" => &["js", "jsx"],
        "node js" => &["node", "nodejs", "npm"],
        "frontend" => &["browser", "component", "components", "form", "forms", "ui"],
        "react" => &[
            "jsx",
            "tsx",
            "component",
            "components",
            "hooks",
            "form",
            "forms",
        ],
        "ui" => &["component", "components", "browser", "form", "forms"],
        "browser" => &["dom", "frontend", "ui"],
        "python" => &["py", "pytest"],
        "go" => &["golang"],
        "rust" => &["cargo", "rs"],
        "compiler" => &["diagnostic", "lint", "lints"],
        "backend" => &[
            "api",
            "async",
            "concurrency",
            "runtime",
            "service",
            "server",
        ],
        "performance" => &["benchmark", "profiler", "profiling"],
        "data" => &["dataset", "profiler", "training"],
        "cli" => &["command line", "terminal", "subcommand", "base command"],
        "developer tools" | "developer tool" => &[
            "developer tools",
            "developer tool",
            "devtools",
            "dev tools",
            "tooling",
            "sdk",
            "build tool",
            "packaging",
            "installer",
            "command line",
            "terminal",
            "subcommand",
            "base command",
        ],
        "ai" | "llm" | "agent" => &["mcp", "model context protocol"],
        _ => &[],
    }
}

fn domain_aliases(term: &str) -> &'static [&'static str] {
    match term {
        "frontend" => &[
            "browser",
            "component",
            "components",
            "dom",
            "form",
            "forms",
            "render",
            "ui",
        ],
        "react" => &[
            "component",
            "components",
            "form",
            "forms",
            "hooks",
            "render",
        ],
        "ui" => &[
            "button",
            "component",
            "components",
            "form",
            "forms",
            "render",
        ],
        "browser" => &["dom", "render", "status", "ui"],
        "rust" => &["diagnostic", "lint", "lints", "metadata", "path", "syscall"],
        "go" => &["api", "docker", "service", "server"],
        "backend" => &[
            "api",
            "async",
            "backpressure",
            "concurrency",
            "docker",
            "mpsc",
            "runtime",
            "service",
            "server",
        ],
        "compiler" => &["diagnostic", "lint", "lints", "metadata"],
        "performance" => &["benchmark", "profiler", "profiling"],
        "python" => &["profiler", "pytest", "training", "validation"],
        "data" => &["dataset", "profiler", "training", "validation"],
        "testing" => &["regression", "test", "tests", "validation"],
        "cli" => &["command", "completion", "terminal"],
        "developer tools" | "developer tool" => &["sdk", "tooling"],
        "ai" | "llm" | "agent" => &[
            "eval",
            "evaluation",
            "mcp",
            "memory",
            "model context protocol",
            "prompt",
            "sampling",
        ],
        "kubernetes" | "docker" | "ci" | "infrastructure" => &[
            "container",
            "deploy",
            "dockerfile",
            "helm",
            "manifest",
            "sync",
        ],
        _ => &[],
    }
}

fn language_matches_profile(enriched: &EnrichedIssue, profile_terms: &[String]) -> bool {
    enriched
        .repository
        .language
        .as_deref()
        .map(normalize)
        .is_some_and(|language| matching_terms(&language, profile_terms).len() == 1)
}

fn wants_cli_or_devtools(profile: &ProfileConfig) -> bool {
    let terms = profile_terms(profile);
    terms.iter().any(|term| {
        matches!(
            term.as_str(),
            "cli"
                | "command line"
                | "terminal"
                | "subcommand"
                | "base command"
                | "developer tools"
                | "developer tool"
                | "devtools"
                | "tooling"
        )
    })
}

fn looks_like_cli_or_devtool(issue_text: &str, repo_text: &str) -> bool {
    let combined = format!("{issue_text} {repo_text}");
    contains_any(
        &combined,
        &[
            "cli",
            "command line",
            "terminal",
            "subcommand",
            "developer tool",
            "developer tools",
            "devtools",
            "tooling",
            "sdk",
            "build tool",
            "packaging",
            "installer",
            "briefcase",
            "base command",
            "msi",
        ],
    )
}

fn cli_devtool_boost_allowed(
    enriched: &EnrichedIssue,
    profile: &ProfileConfig,
    issue_text: &str,
    repo_text: &str,
) -> bool {
    let tech_stack = profile
        .tech_stack
        .iter()
        .map(|item| normalize(item))
        .collect::<Vec<_>>();
    let language = enriched
        .repository
        .language
        .as_deref()
        .map(normalize)
        .unwrap_or_default();
    if !language.is_empty() && tech_stack.iter().any(|term| term == &language) {
        return true;
    }

    if tech_stack.iter().any(|term| term == "python") {
        let text = format!("{issue_text} {repo_text}");
        return contains_any(
            &text,
            &[
                "python",
                "pandas",
                "dataframe",
                "dataset",
                "machine learning",
                "package manager",
                "profiler",
                "training",
            ],
        );
    }

    true
}

fn apply_profile_focus_caps(
    enriched: &EnrichedIssue,
    profile: &ProfileConfig,
    issue_text: &str,
    repo_text: &str,
    score: &mut i32,
    reasons: &mut Vec<String>,
    evidence_refs: &mut Vec<String>,
) {
    let profile_terms = profile_terms(profile);

    if profile_terms
        .iter()
        .any(|term| matches!(term.as_str(), "ai" | "llm" | "agent"))
    {
        let issue_matches = focus_matches(issue_text, AI_FOCUS_TERMS);
        let repo_matches = focus_matches(repo_text, AI_FOCUS_TERMS);
        let only_generic_agent =
            issue_matches.len() == 1 && issue_matches[0] == "agent" && repo_matches.is_empty();
        if issue_matches.is_empty() || only_generic_agent {
            *score = (*score).min(55);
            reasons.push(
                "AI/agent profile requires issue-level AI, LLM, agent, MCP, prompt, eval, or model evidence"
                    .to_string(),
            );
            evidence_refs.push("profile:keywords".to_string());
        }
    }

    if profile_terms.iter().any(|term| {
        matches!(
            term.as_str(),
            "kubernetes" | "docker" | "ci" | "infrastructure"
        )
    }) && !has_focus_signal(issue_text, DEVOPS_FOCUS_TERMS)
    {
        *score = (*score).min(55);
        reasons.push(
            "DevOps/infra profile requires issue-level container, deploy, CI, Kubernetes, Docker, or infrastructure evidence"
                .to_string(),
        );
        evidence_refs.push("profile:keywords".to_string());
    }

    let language = enriched
        .repository
        .language
        .as_deref()
        .map(normalize)
        .unwrap_or_default();
    if is_backend_systems_profile(profile)
        && !matches!(language.as_str(), "rust" | "go")
        && !has_focus_signal(issue_text, BACKEND_SYSTEMS_FOCUS_TERMS)
    {
        *score = (*score).min(55);
        reasons.push(
            "Rust/Go backend profile requires Rust, Go, compiler, runtime, parser, concurrency, or backend evidence"
                .to_string(),
        );
        evidence_refs.push("profile:keywords".to_string());
    }

    if profile_terms.iter().any(|term| {
        matches!(
            term.as_str(),
            "frontend" | "react" | "ui" | "browser" | "dom"
        )
    }) && !has_frontend_focus_signal(issue_text)
    {
        *score = (*score).min(58);
        reasons.push(
            "Frontend profile requires issue-level UI, React, browser, component-with-UI-context, or form evidence"
                .to_string(),
        );
        evidence_refs.push("profile:keywords".to_string());
    }

    let python_data_cli_profile = profile
        .tech_stack
        .iter()
        .map(|item| normalize(item))
        .any(|term| term == "python")
        && profile
            .keywords
            .iter()
            .map(|item| normalize(item))
            .any(|term| matches!(term.as_str(), "cli" | "data" | "testing" | "pandas"));
    if python_data_cli_profile && !has_focus_signal(issue_text, PYTHON_DATA_CLI_FOCUS_TERMS) {
        *score = (*score).min(58);
        reasons.push(
            "Python data/CLI profile requires issue-level CLI, data, notebook, package, testing, or traceback evidence"
                .to_string(),
        );
        evidence_refs.push("profile:tech_stack".to_string());
    } else if python_data_cli_profile {
        let python_context = language == "python"
            || has_focus_signal(issue_text, PYTHON_CONTEXT_TERMS)
            || has_focus_signal(repo_text, PYTHON_CONTEXT_TERMS);
        if !python_context {
            *score = (*score).min(55);
            reasons.push(
                "Python data/CLI profile requires Python, data, notebook, package, or testing context beyond generic command-line evidence"
                    .to_string(),
            );
            evidence_refs.push("profile:tech_stack".to_string());
        }
    }
}

fn is_backend_systems_profile(profile: &ProfileConfig) -> bool {
    let tech_stack = profile
        .tech_stack
        .iter()
        .map(|item| normalize(item))
        .collect::<Vec<_>>();
    let keywords = profile
        .keywords
        .iter()
        .map(|item| normalize(item))
        .collect::<Vec<_>>();
    let has_backend_keyword = keywords.iter().any(|term| {
        matches!(
            term.as_str(),
            "backend" | "compiler" | "performance" | "cargo"
        )
    });
    let has_rust_and_go =
        tech_stack.iter().any(|term| term == "rust") && tech_stack.iter().any(|term| term == "go");
    has_backend_keyword || has_rust_and_go
}

fn has_focus_signal(searchable: &str, terms: &[&str]) -> bool {
    !focus_matches(searchable, terms).is_empty()
}

fn has_frontend_focus_signal(issue_text: &str) -> bool {
    if has_focus_signal(issue_text, FRONTEND_STRONG_FOCUS_TERMS) {
        return true;
    }

    let component_match = has_focus_signal(issue_text, &["component", "components"]);
    component_match && has_focus_signal(issue_text, FRONTEND_COMPONENT_CONTEXT_TERMS)
}

fn focus_matches(searchable: &str, terms: &[&str]) -> Vec<String> {
    let terms = terms.iter().map(|term| normalize(term)).collect::<Vec<_>>();
    matching_terms(searchable, &terms)
}

const AI_FOCUS_TERMS: &[&str] = &[
    "ai",
    "llm",
    "agent",
    "anthropic",
    "claude",
    "eval",
    "evaluation",
    "inference",
    "machine learning",
    "mcp",
    "memory",
    "model",
    "model context protocol",
    "openai",
    "prompt",
    "pytorch",
    "rag",
    "sampling",
    "tensorflow",
    "training",
    "vllm",
];

const DEVOPS_FOCUS_TERMS: &[&str] = &[
    "ci",
    "cloud",
    "compose",
    "container",
    "containers",
    "deploy",
    "deployment",
    "docker",
    "dockerfile",
    "gitops",
    "helm",
    "infra",
    "infrastructure",
    "kubernetes",
    "manifest",
    "pipeline",
    "runner",
    "runners",
    "terraform",
];

const BACKEND_SYSTEMS_FOCUS_TERMS: &[&str] = &[
    "asm",
    "assembly",
    "backend",
    "backpressure",
    "cargo",
    "compiler",
    "concurrency",
    "go",
    "golang",
    "mpsc",
    "parser",
    "runtime",
    "rust",
    "syscall",
];

const FRONTEND_STRONG_FOCUS_TERMS: &[&str] = &[
    "accessibility",
    "browser",
    "button",
    "css",
    "dom",
    "focus",
    "form",
    "forms",
    "frontend",
    "hook",
    "hooks",
    "modal",
    "page",
    "react",
    "screen reader",
    "theme",
    "ui",
    "websocket",
];

const FRONTEND_COMPONENT_CONTEXT_TERMS: &[&str] = &[
    "accessibility",
    "browser",
    "button",
    "css",
    "dom",
    "focus",
    "form",
    "forms",
    "frontend",
    "hook",
    "hooks",
    "modal",
    "page",
    "react",
    "screen reader",
    "theme",
    "ui",
];

const PYTHON_DATA_CLI_FOCUS_TERMS: &[&str] = &[
    "cli",
    "command",
    "completion",
    "data",
    "dataframe",
    "dataset",
    "importerror",
    "jupyter",
    "notebook",
    "package manager",
    "pandas",
    "pip",
    "profiler",
    "pyproject",
    "pytest",
    "regression",
    "subcommand",
    "terminal",
    "test",
    "testing",
    "tests",
    "traceback",
    "training",
    "venv",
];

const PYTHON_CONTEXT_TERMS: &[&str] = &[
    "data",
    "dataframe",
    "dataset",
    "importerror",
    "jupyter",
    "machine learning",
    "notebook",
    "package manager",
    "pandas",
    "pip",
    "profiler",
    "py",
    "pyproject",
    "pytest",
    "python",
    "traceback",
    "training",
    "uv",
    "venv",
];

fn lacks_security_profile(profile: &ProfileConfig) -> bool {
    let terms = profile_terms(profile);
    !terms.iter().any(|term| {
        matches!(
            term.as_str(),
            "security" | "crypto" | "cryptography" | "solidity" | "ethereum" | "web3"
        )
    })
}

fn looks_like_crypto_security(issue_text: &str, repo_text: &str) -> bool {
    let combined = format!("{issue_text} {repo_text}");
    contains_any(
        &combined,
        &[
            "solidity",
            "web3",
            "ethereum",
            "smart contract",
            "bounty",
            "audit",
            "exploit",
            "vulnerability",
        ],
    )
}

fn is_maintainer_association(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "owner" | "member" | "collaborator"
    )
}

fn contains_any(text: &str, values: &[&str]) -> bool {
    values.iter().any(|value| text.contains(value))
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::{execution_quality_assessment, profile_fit_assessment, score_band};
    use crate::config::ProfileConfig;
    use crate::github::GitHubIssue;
    use crate::github_enrichment::EnrichedIssue;
    use crate::value_model::{RiskTag, ScoreBand};

    fn enriched(title: &str, body: &str) -> EnrichedIssue {
        let issue = GitHubIssue {
            id: 1,
            number: 1,
            title: title.to_string(),
            body: body.to_string(),
            labels: vec!["good first issue".to_string()],
            url: "https://github.com/owner/repo/issues/1".to_string(),
            repo_full_name: "beeware/briefcase".to_string(),
            repo_name: "briefcase".to_string(),
            repo_description: "A command line tool for packaging Python apps".to_string(),
            repo_stars: 3_000,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };
        EnrichedIssue::from_issue(&issue)
    }

    #[test]
    fn python_cli_issue_fits_default_cli_devtool_profile() {
        let enriched = enriched(
            "Briefcase validates base command paths incorrectly",
            "Steps to reproduce with briefcase create. Expected the Base command to reject invalid paths.",
        );
        let fit = profile_fit_assessment(
            &enriched,
            &ProfileConfig {
                tech_stack: vec!["Rust".to_string(), "TypeScript".to_string()],
                keywords: vec!["cli".to_string(), "developer-tools".to_string()],
            },
            &[],
        );
        assert!(fit.score >= 60, "{fit:?}");
    }

    #[test]
    fn low_depth_caps_profile_and_execution() {
        let enriched = enriched(
            "Add new Japanese proverb",
            "No code required. Browser in under 60 seconds. Add JSON content.",
        );
        let tags = vec![RiskTag::NoCodeRequired, RiskTag::ContentFill];
        let fit = profile_fit_assessment(
            &enriched,
            &ProfileConfig {
                tech_stack: vec!["Rust".to_string(), "TypeScript".to_string()],
                keywords: vec!["cli".to_string(), "developer-tools".to_string()],
            },
            &tags,
        );
        let execution = execution_quality_assessment(&enriched, &tags);
        assert!(fit.score <= 25);
        assert!(execution.score <= 25);
    }

    #[test]
    fn score_band_matches_thresholds() {
        assert_eq!(score_band(70), ScoreBand::High);
        assert_eq!(score_band(30), ScoreBand::Medium);
        assert_eq!(score_band(29), ScoreBand::Low);
    }
}
