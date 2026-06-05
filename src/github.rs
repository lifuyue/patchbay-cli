use std::collections::{HashMap, HashSet};
use std::fs;

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use futures::stream::{self, StreamExt};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::config::{Config, ProfileConfig};
use crate::discovery::{
    gfi_repositories, merge_candidates, overlay_repositories, DiscoveryCandidate, RepoTrustTier,
    TrustedRepository,
};
use crate::errors::IssueFinderError;
use crate::paths::{atomic_write, IssueFinderPaths};

const SEARCH_CACHE_TTL_MINUTES: i64 = 10;
const TRUSTED_OVERLAY_REPO_REQUEST_LIMIT: usize = 15;
const GFI_REPO_REQUEST_LIMIT: usize = 30;
const GLOBAL_SEARCH_REQUEST_LIMIT: usize = 20;
const TRUSTED_OVERLAY_REPO_CANDIDATE_LIMIT: usize = 8;
const GFI_REPO_CANDIDATE_LIMIT: usize = 4;
const TRUSTED_LABEL_PER_PAGE: usize = 8;
const GLOBAL_SEARCH_PER_PAGE: usize = 30;
const DISCOVERY_SEARCH_CONCURRENCY_LIMIT: usize = 4;

const BEGINNER_LABELS: [&str; 8] = [
    "good first issue",
    "good-first-issue",
    "beginner",
    "beginner-friendly",
    "easy",
    "starter",
    "help wanted",
    "low-hanging-fruit",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueRef {
    pub owner: String,
    pub repo: String,
    pub number: u64,
}

impl IssueRef {
    pub fn repo_full_name(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    pub fn parse(value: &str) -> Result<Self> {
        if value.starts_with("http://") || value.starts_with("https://") {
            return Self::parse_url(value);
        }

        let (repo_part, number_part) = value
            .split_once('#')
            .ok_or(IssueFinderError::InvalidIssueReference)?;
        let (owner, repo) = repo_part
            .split_once('/')
            .ok_or(IssueFinderError::InvalidIssueReference)?;
        let number = number_part
            .parse::<u64>()
            .map_err(|_| IssueFinderError::InvalidIssueReference)?;

        if owner.trim().is_empty() || repo.trim().is_empty() || number == 0 {
            return Err(IssueFinderError::InvalidIssueReference.into());
        }

        Ok(Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
            number,
        })
    }

    pub fn parse_url(value: &str) -> Result<Self> {
        let url = Url::parse(value).map_err(|_| IssueFinderError::InvalidIssueReference)?;
        if url.host_str() != Some("github.com") {
            return Err(IssueFinderError::InvalidIssueReference.into());
        }

        let parts = url
            .path_segments()
            .ok_or(IssueFinderError::InvalidIssueReference)?;
        let segments = parts.collect::<Vec<_>>();
        if segments.len() < 4 || segments[2] != "issues" {
            return Err(IssueFinderError::InvalidIssueReference.into());
        }

        let number = segments[3]
            .parse::<u64>()
            .map_err(|_| IssueFinderError::InvalidIssueReference)?;

        Ok(Self {
            owner: segments[0].to_string(),
            repo: segments[1].to_string(),
            number,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitHubIssue {
    pub id: u64,
    pub number: u64,
    pub title: String,
    pub body: String,
    pub labels: Vec<String>,
    pub url: String,
    pub repo_full_name: String,
    pub repo_name: String,
    pub repo_description: String,
    pub repo_stars: u64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DiscoveryCachePayload {
    fetched_at: DateTime<Utc>,
    candidates: Vec<DiscoveryCandidate>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    items: Vec<SearchIssue>,
}

#[derive(Debug, Deserialize)]
struct SearchIssue {
    id: u64,
    number: u64,
    title: String,
    body: Option<String>,
    html_url: String,
    repository_url: String,
    labels: Vec<GitHubLabel>,
    pull_request: Option<serde_json::Value>,
    locked: bool,
    assignee: Option<serde_json::Value>,
    assignees: Option<Vec<serde_json::Value>>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum GitHubLabel {
    Name(String),
    Object { name: Option<String> },
}

#[derive(Debug, Deserialize)]
struct RepoResponse {
    full_name: String,
    name: String,
    description: Option<String>,
    stargazers_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct IssueResponse {
    id: u64,
    number: u64,
    title: String,
    body: Option<String>,
    html_url: String,
    labels: Vec<GitHubLabel>,
    pull_request: Option<serde_json::Value>,
    locked: bool,
    assignee: Option<serde_json::Value>,
    assignees: Option<Vec<serde_json::Value>>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone)]
struct RepoMetadata {
    full_name: String,
    name: String,
    description: String,
    stars: u64,
}

pub struct GitHubClient {
    http: reqwest::Client,
    token: String,
    api_base_url: String,
}

impl GitHubClient {
    pub fn new(config: &Config) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent("issue-finder")
            .build()?;
        Ok(Self {
            http,
            token: config.github.token.clone(),
            api_base_url: std::env::var("ISSUE_FINDER_GITHUB_API_BASE")
                .unwrap_or_else(|_| "https://api.github.com".to_string()),
        })
    }

    pub async fn discover_issues(
        &self,
        paths: &IssueFinderPaths,
        refresh: bool,
    ) -> Result<Vec<GitHubIssue>> {
        let profile = Config::default().profile;
        Ok(self
            .discover_candidates(paths, refresh, &profile)
            .await?
            .into_iter()
            .map(|candidate| candidate.issue)
            .collect())
    }

    pub async fn discover_candidates(
        &self,
        paths: &IssueFinderPaths,
        refresh: bool,
        profile: &ProfileConfig,
    ) -> Result<Vec<DiscoveryCandidate>> {
        if !refresh {
            if let Some(cached) = load_cached_candidates(paths)? {
                return Ok(cached);
            }
        }

        let mut lanes = Vec::new();
        lanes.extend(
            overlay_repositories()?
                .into_iter()
                .take(TRUSTED_OVERLAY_REPO_REQUEST_LIMIT)
                .map(|repository| {
                    SearchLaneRequest::trusted(repository, RepoTrustTier::OverlayTrusted)
                }),
        );

        lanes.extend(
            gfi_repositories(profile, GFI_REPO_REQUEST_LIMIT)?
                .into_iter()
                .map(|repository| {
                    SearchLaneRequest::trusted(repository, RepoTrustTier::GfiTrusted)
                }),
        );

        lanes.extend(
            global_search_lanes(profile)
                .into_iter()
                .take(GLOBAL_SEARCH_REQUEST_LIMIT)
                .map(SearchLaneRequest::global),
        );

        let lane_results = stream::iter(
            lanes
                .into_iter()
                .map(|lane| async move { self.fetch_lane_candidates(lane, profile).await.ok() }),
        )
        .buffer_unordered(DISCOVERY_SEARCH_CONCURRENCY_LIMIT)
        .collect::<Vec<_>>()
        .await;

        let mut candidates = Vec::new();
        for mut lane_candidates in lane_results.into_iter().flatten() {
            candidates.append(&mut lane_candidates);
        }

        let candidates = merge_candidates(candidates, profile);
        save_cached_candidates(paths, &candidates)?;
        Ok(candidates)
    }

    async fn fetch_lane_candidates(
        &self,
        lane: SearchLaneRequest,
        profile: &ProfileConfig,
    ) -> Result<Vec<DiscoveryCandidate>> {
        match lane {
            SearchLaneRequest::TrustedRepo {
                id,
                repository,
                trust_tier,
                candidate_limit,
            } => {
                self.list_trusted_repository_lane(
                    &repository,
                    &id,
                    trust_tier,
                    candidate_limit,
                    profile,
                )
                .await
            }
            SearchLaneRequest::Global {
                id,
                query,
                per_page,
            } => {
                self.search_lane(&query, per_page, &id, RepoTrustTier::Global, profile)
                    .await
            }
        }
    }

    async fn list_trusted_repository_lane(
        &self,
        repository: &TrustedRepository,
        lane_id: &str,
        trust_tier: RepoTrustTier,
        candidate_limit: usize,
        profile: &ProfileConfig,
    ) -> Result<Vec<DiscoveryCandidate>> {
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();

        for label in BEGINNER_LABELS {
            if candidates.len() >= candidate_limit {
                break;
            }

            let per_page = TRUSTED_LABEL_PER_PAGE.to_string();
            let url = self.api_url(&format!(
                "/repos/{}/{}/issues",
                repository.owner, repository.name
            ));
            let response = self
                .authorized(self.http.get(url))
                .query(&[
                    ("state", "open"),
                    ("labels", label),
                    ("sort", "updated"),
                    ("direction", "desc"),
                    ("per_page", per_page.as_str()),
                ])
                .send()
                .await?;
            let response = require_success(response).await?;
            let issues = response.json::<Vec<IssueResponse>>().await?;

            for item in issues {
                if candidates.len() >= candidate_limit {
                    break;
                }
                if !should_include_issue(
                    item.pull_request.is_some(),
                    item.locked,
                    item.assignee.is_some(),
                    item.assignees
                        .as_ref()
                        .map(|items| !items.is_empty())
                        .unwrap_or(false),
                    &item.labels,
                ) {
                    continue;
                }

                let key = format!("{}#{}", repository.full_name(), item.number);
                if !seen.insert(key) {
                    continue;
                }

                let issue = GitHubIssue {
                    id: item.id,
                    number: item.number,
                    title: item.title,
                    body: item.body.unwrap_or_default(),
                    labels: extract_label_names(&item.labels),
                    url: item.html_url,
                    repo_full_name: repository.full_name(),
                    repo_name: repository.name.clone(),
                    repo_description: String::new(),
                    repo_stars: 0,
                    created_at: item.created_at,
                    updated_at: item.updated_at,
                };
                candidates.push(DiscoveryCandidate::new(
                    issue,
                    lane_id.to_string(),
                    trust_tier,
                    profile,
                ));
            }
        }

        Ok(candidates)
    }

    async fn search_lane(
        &self,
        query: &str,
        per_page: usize,
        lane_id: &str,
        trust_tier: RepoTrustTier,
        profile: &ProfileConfig,
    ) -> Result<Vec<DiscoveryCandidate>> {
        let per_page = per_page.to_string();
        let url = self.api_url("/search/issues");
        let response = self
            .authorized(self.http.get(url))
            .query(&[
                ("q", query),
                ("sort", "updated"),
                ("order", "desc"),
                ("per_page", per_page.as_str()),
            ])
            .send()
            .await?;

        let response = require_success(response).await?;
        let payload = response.json::<SearchResponse>().await?;
        let mut candidates = Vec::new();

        for item in payload.items {
            if !should_include_issue(
                item.pull_request.is_some(),
                item.locked,
                item.assignee.is_some(),
                item.assignees
                    .as_ref()
                    .map(|items| !items.is_empty())
                    .unwrap_or(false),
                &item.labels,
            ) {
                continue;
            }

            let (owner, repo) = parse_repo_api_url(&item.repository_url)?;
            let repo_full_name = format!("{owner}/{repo}");

            let issue = GitHubIssue {
                id: item.id,
                number: item.number,
                title: item.title,
                body: item.body.unwrap_or_default(),
                labels: extract_label_names(&item.labels),
                url: item.html_url,
                repo_full_name,
                repo_name: repo,
                repo_description: String::new(),
                repo_stars: 0,
                created_at: item.created_at,
                updated_at: item.updated_at,
            };
            candidates.push(DiscoveryCandidate::new(
                issue,
                lane_id.to_string(),
                trust_tier,
                profile,
            ));
        }

        Ok(candidates)
    }

    pub async fn fetch_issue(&self, issue_ref: &IssueRef) -> Result<GitHubIssue> {
        let mut repo_cache = HashMap::new();
        let metadata = self
            .repo_metadata(&issue_ref.owner, &issue_ref.repo, &mut repo_cache)
            .await
            .unwrap_or_else(|_| RepoMetadata {
                full_name: issue_ref.repo_full_name(),
                name: issue_ref.repo.clone(),
                description: String::new(),
                stars: 0,
            });

        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            self.api_base_url.trim_end_matches('/'),
            issue_ref.owner,
            issue_ref.repo,
            issue_ref.number
        );
        let response = self.authorized(self.http.get(url)).send().await?;
        let response = require_success(response).await?;
        let issue = response.json::<IssueResponse>().await?;

        if issue.pull_request.is_some() {
            anyhow::bail!(
                "{} is a pull request, not an issue",
                issue_ref.repo_full_name()
            );
        }

        if !should_include_issue(
            false,
            issue.locked,
            issue.assignee.is_some(),
            issue
                .assignees
                .as_ref()
                .map(|items| !items.is_empty())
                .unwrap_or(false),
            &issue.labels,
        ) {
            anyhow::bail!("issue is locked, assigned, or carries a blocking label");
        }

        Ok(GitHubIssue {
            id: issue.id,
            number: issue.number,
            title: issue.title,
            body: issue.body.unwrap_or_default(),
            labels: extract_label_names(&issue.labels),
            url: issue.html_url,
            repo_full_name: metadata.full_name,
            repo_name: metadata.name,
            repo_description: metadata.description,
            repo_stars: metadata.stars,
            created_at: issue.created_at,
            updated_at: issue.updated_at,
        })
    }

    pub async fn validate_token(&self) -> Result<String> {
        if self.token.trim().is_empty() {
            anyhow::bail!("GitHub token is missing");
        }

        let response = self
            .authorized(self.http.get(self.api_url("/user")))
            .send()
            .await?;
        let response = require_success(response).await?;
        let value = response.json::<serde_json::Value>().await?;
        Ok(value
            .get("login")
            .and_then(|login| login.as_str())
            .unwrap_or("unknown")
            .to_string())
    }

    fn authorized(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if self.token.trim().is_empty() {
            request
        } else {
            request.bearer_auth(self.token.trim())
        }
    }

    async fn repo_metadata(
        &self,
        owner: &str,
        repo: &str,
        cache: &mut HashMap<String, RepoMetadata>,
    ) -> Result<RepoMetadata> {
        let key = format!("{owner}/{repo}");
        if let Some(metadata) = cache.get(&key) {
            return Ok(metadata.clone());
        }

        let url = self.api_url(&format!("/repos/{owner}/{repo}"));
        let response = self.authorized(self.http.get(url)).send().await?;
        let response = require_success(response).await?;
        let repo = response.json::<RepoResponse>().await?;
        let metadata = RepoMetadata {
            full_name: repo.full_name,
            name: repo.name,
            description: repo.description.unwrap_or_default(),
            stars: repo.stargazers_count.unwrap_or_default(),
        };
        cache.insert(key, metadata.clone());
        Ok(metadata)
    }

    fn api_url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.api_base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }
}

pub fn build_search_query(label: &str) -> String {
    format!("label:\"{label}\" archived:false is:issue is:open no:assignee")
}

#[derive(Debug)]
enum SearchLaneRequest {
    TrustedRepo {
        id: String,
        repository: TrustedRepository,
        trust_tier: RepoTrustTier,
        candidate_limit: usize,
    },
    Global {
        id: String,
        query: String,
        per_page: usize,
    },
}

impl SearchLaneRequest {
    fn trusted(repository: TrustedRepository, trust_tier: RepoTrustTier) -> Self {
        let repo_full_name = repository.full_name();
        let candidate_limit = match trust_tier {
            RepoTrustTier::OverlayTrusted => TRUSTED_OVERLAY_REPO_CANDIDATE_LIMIT,
            RepoTrustTier::GfiTrusted => GFI_REPO_CANDIDATE_LIMIT,
            RepoTrustTier::Global => 0,
        };
        Self::TrustedRepo {
            id: format!("{trust_tier}:{repo_full_name}"),
            repository,
            trust_tier,
            candidate_limit,
        }
    }

    fn global(lane: GlobalSearchLane) -> Self {
        Self::Global {
            id: lane.id,
            query: lane.query,
            per_page: GLOBAL_SEARCH_PER_PAGE,
        }
    }
}

#[derive(Debug)]
struct GlobalSearchLane {
    id: String,
    query: String,
}

fn global_search_lanes(profile: &ProfileConfig) -> Vec<GlobalSearchLane> {
    let mut lanes = vec![
        GlobalSearchLane {
            id: "global:good-first-issue".to_string(),
            query: build_search_query("good first issue"),
        },
        GlobalSearchLane {
            id: "global:good-first-issue-hyphen".to_string(),
            query: build_search_query("good-first-issue"),
        },
    ];

    let terms = crate::scoring::profile_terms(profile)
        .into_iter()
        .filter(|term| term.len() >= 3)
        .take(6)
        .collect::<Vec<_>>();
    for term in terms {
        let term = search_term(&term);
        lanes.push(GlobalSearchLane {
            id: format!("global:beginner:{term}"),
            query: format!("label:beginner archived:false is:issue is:open no:assignee {term}"),
        });
        lanes.push(GlobalSearchLane {
            id: format!("global:easy:{term}"),
            query: format!(
                "label:easy archived:false is:issue is:open no:assignee {term} expected actual"
            ),
        });
        lanes.push(GlobalSearchLane {
            id: format!("global:help-wanted:{term}"),
            query: format!(
                "label:\"help wanted\" archived:false is:issue is:open no:assignee {term} repro"
            ),
        });
    }

    lanes
}

fn search_term(term: &str) -> String {
    if term.contains(' ') {
        format!("\"{term}\"")
    } else {
        term.to_string()
    }
}

fn load_cached_candidates(paths: &IssueFinderPaths) -> Result<Option<Vec<DiscoveryCandidate>>> {
    let cache_path = paths.issue_cache_path();
    if !cache_path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&cache_path)
        .with_context(|| format!("unable to read {}", cache_path.display()))?;
    let Ok(payload) = serde_json::from_str::<DiscoveryCachePayload>(&raw) else {
        return Ok(None);
    };
    if Utc::now() - payload.fetched_at > Duration::minutes(SEARCH_CACHE_TTL_MINUTES) {
        return Ok(None);
    }

    Ok(Some(payload.candidates))
}

fn save_cached_candidates(
    paths: &IssueFinderPaths,
    candidates: &[DiscoveryCandidate],
) -> Result<()> {
    let payload = DiscoveryCachePayload {
        fetched_at: Utc::now(),
        candidates: candidates.to_vec(),
    };
    atomic_write(
        paths.issue_cache_path().as_path(),
        serde_json::to_vec_pretty(&payload)?,
    )?;
    Ok(())
}

async fn require_success(response: reqwest::Response) -> Result<reqwest::Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }

    let body = response.text().await.unwrap_or_default();
    if status == StatusCode::FORBIDDEN || status == StatusCode::TOO_MANY_REQUESTS {
        return Err(IssueFinderError::GitHubRateLimited.into());
    }

    Err(IssueFinderError::GitHubResponse(format!("{status}: {body}")).into())
}

fn parse_repo_api_url(value: &str) -> Result<(String, String)> {
    let url = Url::parse(value)?;
    let segments = url.path_segments().context("repository URL missing path")?;
    let parts = segments.collect::<Vec<_>>();
    let repos_index = parts
        .iter()
        .position(|part| *part == "repos")
        .context("repository URL missing repos segment")?;
    let owner = parts
        .get(repos_index + 1)
        .context("repository URL missing owner")?;
    let repo = parts
        .get(repos_index + 2)
        .context("repository URL missing repo")?;
    Ok((owner.to_string(), repo.to_string()))
}

fn should_include_issue(
    is_pull_request: bool,
    locked: bool,
    has_assignee: bool,
    has_assignees: bool,
    labels: &[GitHubLabel],
) -> bool {
    if is_pull_request || locked || has_assignee || has_assignees {
        return false;
    }

    let label_names = extract_label_names(labels);
    !has_action_blocking_label(&label_names)
}

fn extract_label_names(labels: &[GitHubLabel]) -> Vec<String> {
    labels
        .iter()
        .filter_map(|label| match label {
            GitHubLabel::Name(name) => Some(name.clone()),
            GitHubLabel::Object { name } => name.clone(),
        })
        .filter(|name| !name.trim().is_empty())
        .collect()
}

fn has_action_blocking_label(labels: &[String]) -> bool {
    const BLOCKED: [&str; 8] = [
        "blocked",
        "duplicate",
        "invalid",
        "needs info",
        "needs information",
        "question",
        "discussion",
        "wontfix",
    ];

    labels.iter().any(|label| {
        let normalized = normalize_label(label);
        BLOCKED.iter().any(|blocked| {
            normalized == *blocked
                || normalized.ends_with(&format!(" {blocked}"))
                || normalized.contains(&format!("{blocked}:"))
        })
    })
}

fn normalize_label(label: &str) -> String {
    label
        .to_lowercase()
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{build_search_query, IssueRef};

    #[test]
    fn parses_short_issue_reference() {
        let parsed = IssueRef::parse("owner/repo#123").unwrap();
        assert_eq!(parsed.owner, "owner");
        assert_eq!(parsed.repo, "repo");
        assert_eq!(parsed.number, 123);
    }

    #[test]
    fn parses_github_issue_url() {
        let parsed = IssueRef::parse_url("https://github.com/owner/repo/issues/42").unwrap();
        assert_eq!(parsed.repo_full_name(), "owner/repo");
        assert_eq!(parsed.number, 42);
    }

    #[test]
    fn builds_good_first_issue_search_query() {
        assert_eq!(
            build_search_query("good first issue"),
            "label:\"good first issue\" archived:false is:issue is:open no:assignee"
        );
    }
}
