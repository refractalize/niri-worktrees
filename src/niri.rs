use crate::errors::{message, AppError, Result};
use niri_ipc::{Action, Request, Response, WorkspaceReferenceArg};
use serde_json::Value;

pub trait NiriClient {
    fn workspaces(&self) -> Result<Vec<Value>>;
    fn windows(&self) -> Result<Vec<Value>>;
    fn focus_monitor(&self, output: &str) -> Result<()>;
    fn focus_workspace_idx(&self, idx: i64) -> Result<()>;
}

#[derive(Debug, Default)]
pub struct SocketNiriClient;

impl SocketNiriClient {
    fn request(&self, request: Request) -> Result<Response> {
        let mut socket = niri_ipc::socket::Socket::connect()
            .map_err(|e| AppError::Message(format!("Could not connect to niri socket: {e}")))?;
        socket
            .send(request)
            .map_err(|e| AppError::Message(format!("Could not communicate with niri socket: {e}")))?
            .map_err(|e| AppError::Message(format!("niri returned an error: {e}")))
    }
}

impl NiriClient for SocketNiriClient {
    fn workspaces(&self) -> Result<Vec<Value>> {
        match self.request(Request::Workspaces)? {
            Response::Workspaces(workspaces) => to_values(workspaces),
            _ => message("niri workspaces response had the wrong type"),
        }
    }

    fn windows(&self) -> Result<Vec<Value>> {
        match self.request(Request::Windows)? {
            Response::Windows(windows) => to_values(windows),
            _ => message("niri windows response had the wrong type"),
        }
    }

    fn focus_monitor(&self, output: &str) -> Result<()> {
        handled(self.request(Request::Action(Action::FocusMonitor {
            output: output.to_string(),
        }))?)
    }

    fn focus_workspace_idx(&self, idx: i64) -> Result<()> {
        let idx = u8::try_from(idx)
            .map_err(|_| AppError::Message(format!("Niri workspace idx {idx} is out of range")))?;
        handled(self.request(Request::Action(Action::FocusWorkspace {
            reference: WorkspaceReferenceArg::Index(idx),
        }))?)
    }
}

fn to_values<T: serde::Serialize>(items: Vec<T>) -> Result<Vec<Value>> {
    items
        .into_iter()
        .map(|item| {
            serde_json::to_value(item)
                .map_err(|e| AppError::Message(format!("Could not serialize niri response: {e}")))
        })
        .collect()
}

fn handled(value: Response) -> Result<()> {
    match value {
        Response::Handled => Ok(()),
        _ => message("niri action response had the wrong type"),
    }
}

pub fn focused_workspace(client: &dyn NiriClient) -> Result<Value> {
    for workspace in client.workspaces()? {
        if workspace.get("is_focused").and_then(Value::as_bool) == Some(true) {
            return Ok(workspace);
        }
    }
    message("No focused Niri workspace was found")
}

pub fn find_workspace_by_id(client: &dyn NiriClient, workspace_id: u64) -> Result<Option<Value>> {
    Ok(client
        .workspaces()?
        .into_iter()
        .find(|workspace| workspace.get("id").and_then(Value::as_u64) == Some(workspace_id)))
}

pub fn focus_workspace(client: &dyn NiriClient, workspace_id: u64) -> Result<Value> {
    let Some(workspace) = find_workspace_by_id(client, workspace_id)? else {
        return message(format!("Niri workspace {workspace_id} does not exist"));
    };
    let output = workspace
        .get("output")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Message(format!("Niri workspace {workspace_id} does not have an output")))?;
    let idx = workspace
        .get("idx")
        .and_then(Value::as_i64)
        .ok_or_else(|| AppError::Message(format!("Niri workspace {workspace_id} does not have an idx")))?;
    client.focus_monitor(output)?;
    client.focus_workspace_idx(idx)?;
    Ok(workspace)
}

pub fn focus_last_workspace_on_current_output(client: &dyn NiriClient) -> Result<Value> {
    let workspaces = client.workspaces()?;
    let focused = workspaces
        .iter()
        .find(|workspace| workspace.get("is_focused").and_then(Value::as_bool) == Some(true))
        .ok_or_else(|| AppError::Message("No focused Niri workspace was found".to_string()))?;
    let output = focused
        .get("output")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Message("Focused Niri workspace does not have an output".to_string()))?;
    let idx = workspaces
        .iter()
        .filter(|workspace| workspace.get("output").and_then(Value::as_str) == Some(output))
        .filter_map(|workspace| workspace.get("idx").and_then(Value::as_i64))
        .max()
        .unwrap_or(0);
    client.focus_workspace_idx(idx)?;
    focused_workspace(client)
}

pub fn latest_focus_timestamps_by_workspace(windows: &[Value]) -> std::collections::HashMap<u64, (i64, i64)> {
    let mut latest = std::collections::HashMap::new();
    for window in windows {
        let Some(workspace_id) = window.get("workspace_id").and_then(Value::as_u64) else {
            continue;
        };
        let Some(ts) = window.get("focus_timestamp") else {
            continue;
        };
        let Some(secs) = ts.get("secs").and_then(Value::as_i64) else {
            continue;
        };
        let Some(nanos) = ts.get("nanos").and_then(Value::as_i64) else {
            continue;
        };
        let value = (secs, nanos);
        if value > *latest.get(&workspace_id).unwrap_or(&(-1, -1)) {
            latest.insert(workspace_id, value);
        }
    }
    latest
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_latest_focus_timestamp() {
        let windows = vec![
            serde_json::json!({"workspace_id": 1, "focus_timestamp": {"secs": 1, "nanos": 0}}),
            serde_json::json!({"workspace_id": 1, "focus_timestamp": {"secs": 2, "nanos": 0}}),
        ];
        assert_eq!(latest_focus_timestamps_by_workspace(&windows).get(&1), Some(&(2, 0)));
    }
}
