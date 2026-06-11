use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

use crate::inbox;
use crate::paths::IssueFinderPaths;
use crate::tool_outputs::{read_context_structured_output, ReadContextStructuredOutput};

const DEFAULT_CONTEXT_MAX_BYTES: usize = 12_000;
const CONTEXT_MAX_BYTES_LIMIT: usize = 50_000;

#[derive(Debug)]
pub enum ReadContextError {
    InvalidArguments(String),
    System(anyhow::Error),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadContextToolArgs {
    pub handoff_id: String,
    pub section: String,
    #[serde(default)]
    pub max_bytes: Option<usize>,
}

pub fn read_context_section(
    paths: &IssueFinderPaths,
    tool_name: &str,
    args: ReadContextToolArgs,
) -> Result<ReadContextStructuredOutput, ReadContextError> {
    let section_path = section_relative_path(&args.section).ok_or_else(|| {
        ReadContextError::InvalidArguments(format!("unsupported context section {}", args.section))
    })?;
    let item = inbox::find_item(paths, &args.handoff_id).map_err(ReadContextError::System)?;
    let handoff_json_path = PathBuf::from(&item.handoff_json_path);
    let handoff_dir = handoff_json_path
        .parent()
        .context("inbox item has no handoff directory")
        .map_err(ReadContextError::System)?;
    let handoff_dir = canonicalize_existing(handoff_dir)?;
    let target = canonicalize_existing(&handoff_dir.join(section_path))?;
    if !target.starts_with(&handoff_dir) {
        return Err(ReadContextError::InvalidArguments(
            "context section resolves outside the handoff directory".to_string(),
        ));
    }

    let max_bytes = args
        .max_bytes
        .unwrap_or(DEFAULT_CONTEXT_MAX_BYTES)
        .min(CONTEXT_MAX_BYTES_LIMIT);
    let bytes = fs::read(&target)
        .with_context(|| format!("unable to read {}", target.display()))
        .map_err(ReadContextError::System)?;
    let truncated = bytes.len() > max_bytes;
    let visible_bytes = if truncated {
        &bytes[..max_bytes]
    } else {
        &bytes[..]
    };
    let content = String::from_utf8_lossy(visible_bytes).to_string();

    Ok(read_context_structured_output(
        tool_name,
        args.handoff_id,
        args.section,
        target.to_string_lossy().to_string(),
        truncated,
        content,
    ))
}

fn section_relative_path(section: &str) -> Option<&'static Path> {
    match section {
        "entry" => Some(Path::new("context/entry.md")),
        "safety" => Some(Path::new("context/safety.md")),
        "probe" => Some(Path::new("context/probe.md")),
        "value" => Some(Path::new("context/value.md")),
        "issue" => Some(Path::new("context/issue.md")),
        "repo" => Some(Path::new("context/repo.md")),
        "validation" => Some(Path::new("context/validation.md")),
        "handoff_json" => Some(Path::new("handoff.json")),
        "agent_policy" => Some(Path::new("agent-policy.json")),
        "probe_json" => Some(Path::new("probe.json")),
        _ => None,
    }
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf, ReadContextError> {
    path.canonicalize()
        .with_context(|| format!("unable to resolve {}", path.display()))
        .map_err(ReadContextError::System)
}
