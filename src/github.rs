use crate::errors::{message, AppError, Result};
use crate::models::PullRequest;
use crate::process::CommandRunner;
use serde::Deserialize;
use std::path::Path;

const PULL_REQUESTS_QUERY: &str = r#"
query($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    pullRequests(first: 100, states: [OPEN, MERGED, CLOSED], orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes {
        number
        headRefName
        state
        isCrossRepository
        reviewThreads(first: 100) {
          nodes {
            isResolved
          }
        }
        commits(last: 1) {
          nodes {
            commit {
              statusCheckRollup {
                state
              }
            }
          }
        }
      }
    }
  }
}
"#;

#[derive(Debug, Deserialize)]
struct GhGraphqlResponse {
    data: Option<GhGraphqlData>,
}

#[derive(Debug, Deserialize)]
struct GhGraphqlData {
    repository: Option<GhRepository>,
}

#[derive(Debug, Deserialize)]
struct GhRepository {
    #[serde(rename = "pullRequests")]
    pull_requests: GhPullRequestConnection,
}

#[derive(Debug, Deserialize)]
struct GhPullRequestConnection {
    nodes: Vec<GhPullRequestNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPullRequestNode {
    number: Option<u64>,
    head_ref_name: Option<String>,
    state: Option<String>,
    is_cross_repository: Option<bool>,
    review_threads: GhReviewThreadConnection,
    commits: GhCommitConnection,
}

#[derive(Debug, Deserialize)]
struct GhReviewThreadConnection {
    nodes: Vec<GhReviewThread>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReviewThread {
    is_resolved: bool,
}

#[derive(Debug, Deserialize)]
struct GhCommitConnection {
    nodes: Vec<GhCommitNode>,
}

#[derive(Debug, Deserialize)]
struct GhCommitNode {
    commit: GhCommit,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCommit {
    status_check_rollup: Option<GhStatusCheckRollup>,
}

#[derive(Debug, Deserialize)]
struct GhStatusCheckRollup {
    state: Option<String>,
}

pub fn pull_requests(runner: &dyn CommandRunner, repo: &Path) -> Result<Vec<PullRequest>> {
    let args = vec![
        "api".to_string(),
        "graphql".to_string(),
        "-f".to_string(),
        format!("query={PULL_REQUESTS_QUERY}"),
        "-F".to_string(),
        "owner={owner}".to_string(),
        "-F".to_string(),
        "name={repo}".to_string(),
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
    let response: GhGraphqlResponse = serde_json::from_str(&out.stdout).map_err(|e| {
        AppError::Message(format!("Could not parse gh PR JSON for {}: {e}", repo.display()))
    })?;
    let nodes = response
        .data
        .and_then(|data| data.repository)
        .map(|repository| repository.pull_requests.nodes)
        .unwrap_or_default();
    Ok(nodes
        .into_iter()
        .filter(|pr| pr.is_cross_repository != Some(true))
        .filter_map(|pr| {
            pr.head_ref_name.map(|head_ref_name| {
                let total_review_comments = pr.review_threads.nodes.len() as u64;
                let unresolved_review_comments = pr
                    .review_threads
                    .nodes
                    .iter()
                    .filter(|thread| !thread.is_resolved)
                    .count() as u64;
                let checks_status = pr
                    .commits
                    .nodes
                    .first()
                    .and_then(|node| node.commit.status_check_rollup.as_ref())
                    .and_then(|rollup| rollup.state.clone());
                PullRequest {
                    head_ref_name,
                    pr_number: pr.number,
                    status: pr.state,
                    unresolved_review_comments: Some(unresolved_review_comments),
                    total_review_comments: Some(total_review_comments),
                    checks_status,
                }
            })
        })
        .collect())
}

