use crate::errors::{message, AppError, Result};
use crate::models::PullRequest;
use crate::process::CommandRunner;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPullRequest {
    head_ref_name: Option<String>,
    number: Option<u64>,
    state: Option<String>,
    is_cross_repository: Option<bool>,
}

pub fn pull_requests(runner: &dyn CommandRunner, repo: &Path) -> Result<Vec<PullRequest>> {
    let args = vec![
        "pr".to_string(),
        "list".to_string(),
        "--state".to_string(),
        "all".to_string(),
        "--json".to_string(),
        "headRefName,number,state,isCrossRepository".to_string(),
    ];
    let out = runner
        .run("gh", &args, Some(repo))
        .map_err(|e| AppError::Message(format!("gh was not found or failed to start: {e}")))?;
    if out.status != 0 {
        return message(format!(
            "Could not list PRs for {}{}",
            repo.display(),
            if out.stderr.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", out.stderr.trim())
            }
        ));
    }
    let prs: Vec<GhPullRequest> = serde_json::from_str(&out.stdout).map_err(|e| {
        AppError::Message(format!("Could not parse gh PR JSON for {}: {e}", repo.display()))
    })?;
    Ok(prs
        .into_iter()
        .filter(|pr| pr.is_cross_repository != Some(true))
        .filter_map(|pr| {
            pr.head_ref_name.map(|head_ref_name| PullRequest {
                head_ref_name,
                pr_number: pr.number,
                status: pr.state,
            })
        })
        .collect())
}

