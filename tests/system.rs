use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::thread;
use tempfile::TempDir;

struct TestEnv {
    temp: TempDir,
    bin: PathBuf,
    home: PathBuf,
    runtime: PathBuf,
    data: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        let home = temp.path().join("home");
        let runtime = temp.path().join("runtime");
        let data = temp.path().join("data");
        for path in [&bin, &home, &runtime, &data] {
            fs::create_dir_all(path).unwrap();
        }
        Self {
            temp,
            bin,
            home,
            runtime,
            data,
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("niri-worktrees").unwrap();
        let path = format!(
            "{}:{}",
            self.bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        cmd.env("PATH", path)
            .env("HOME", &self.home)
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("XDG_DATA_HOME", &self.data);
        cmd
    }

    fn write_exe(&self, name: &str, body: &str) {
        let path = self.bin.join(name);
        fs::write(&path, body).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).unwrap();
    }

    fn repo_path(&self) -> PathBuf {
        self.temp.path().join("repo")
    }

    fn worktree_path(&self) -> PathBuf {
        self.temp.path().join("feature")
    }

    fn write_stores(&self) {
        let store_dir = self.runtime.join("niri-worktrees");
        let repo_dir = self.data.join("niri-worktrees");
        fs::create_dir_all(&store_dir).unwrap();
        fs::create_dir_all(&repo_dir).unwrap();
        fs::create_dir_all(self.repo_path()).unwrap();
        fs::create_dir_all(self.worktree_path()).unwrap();
        fs::write(
            store_dir.join("worktrees.json"),
            serde_json::to_string_pretty(&json!({
                "worktrees": [{"path": self.worktree_path(), "workspace_id": 2}]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            repo_dir.join("repos.json"),
            serde_json::to_string_pretty(&json!({
                "repos": [{"path": self.repo_path(), "bare": false}]
            }))
            .unwrap(),
        )
        .unwrap();
    }

    fn write_stores_with_repo(&self, repo: serde_json::Value) {
        self.write_stores();
        let repo_dir = self.data.join("niri-worktrees");
        fs::write(
            repo_dir.join("repos.json"),
            serde_json::to_string_pretty(&json!({"repos": [repo]})).unwrap(),
        )
        .unwrap();
    }
}

fn git_mock() -> &'static str {
    r#"#!/bin/sh
args="$*"
case "$args" in
  *"rev-parse --git-dir"*) exit 0 ;;
  *"rev-parse --path-format=absolute --git-common-dir"*) echo "/tmp/common.git"; exit 0 ;;
  *"remote get-url origin"*) echo "git@example.com:repo.git"; exit 0 ;;
  *"worktree list --porcelain"*)
    repo=""
    while [ "$#" -gt 0 ]; do
      if [ "$1" = "-C" ]; then repo="$2"; shift 2; else shift; fi
    done
    base="$(dirname "$repo")"
    echo "worktree $repo"
    echo "branch refs/heads/main"
    echo
    echo "worktree $base/feature"
    echo "branch refs/heads/feature"
    exit 0
    ;;
  *"for-each-ref --format=%(refname:short) refs/heads"*) echo "main"; echo "feature"; exit 0 ;;
  *"for-each-ref --format=%(refname:short) refs/remotes"*) echo "origin"; echo "origin/main"; echo "origin/feature"; exit 0 ;;
  *"for-each-ref --format=%(upstream:short) refs/heads/main"*) echo "origin/main"; exit 0 ;;
  *"for-each-ref --format=%(upstream:short) refs/heads/feature"*) echo "origin/feature"; exit 0 ;;
  *"show-ref --verify --quiet refs/heads/feature"*) exit 0 ;;
  *"show-ref --verify --quiet refs/heads/missing"*) exit 1 ;;
  *"show-ref --verify --quiet refs/heads/new-feature"*) exit 1 ;;
  *"worktree add"*" feature"*)
    echo "$args" > "$GIT_MOCK_LOG"
    exit 0
    ;;
  *"worktree add"*"-b new-feature origin/feature"*)
    echo "$args" > "$GIT_MOCK_LOG"
    exit 0
    ;;
  *"rev-parse --abbrev-ref --symbolic-full-name @{u}"*) exit 1 ;;
  *"status --porcelain"*) exit 0 ;;
  *"worktree remove"*) exit 0 ;;
esac
exit 0
"#
}

fn script_mock() -> &'static str {
    r#"#!/bin/sh
echo "$PWD:$*" >> "$SCRIPT_MOCK_LOG"
if [ "$SCRIPT_MOCK_FAIL" = "1" ]; then
  exit 42
fi
exit 0
"#
}

fn start_niri_socket(path: &Path) -> std::io::Result<()> {
    let _ = fs::remove_file(path);
    let listener = UnixListener::bind(path)?;
    thread::spawn(move || {
        for stream in listener.incoming().take(16) {
            let mut stream = stream.unwrap();
            let mut request = String::new();
            {
                let mut reader = BufReader::new(&stream);
                reader.read_line(&mut request).unwrap();
            }
            let value: serde_json::Value = serde_json::from_str(&request).unwrap();
            let reply = match value {
                serde_json::Value::String(ref s) if s == "Workspaces" => json!({
                    "Ok": {"Workspaces": [
                        {"id": 1, "idx": 1, "name": null, "output": "DP-1",
                         "is_urgent": false, "is_active": true, "is_focused": false,
                         "active_window_id": null},
                        {"id": 2, "idx": 2, "name": null, "output": "DP-1",
                         "is_urgent": false, "is_active": true, "is_focused": true,
                         "active_window_id": null}
                    ]}
                }),
                serde_json::Value::String(ref s) if s == "Windows" => json!({"Ok": {"Windows": []}}),
                _ => json!({"Ok": "Handled"}),
            };
            writeln!(stream, "{reply}").unwrap();
        }
    });
    Ok(())
}

#[test]
fn create_branch_from_uses_requested_source_branch() {
    let env = TestEnv::new();
    env.write_stores();
    env.write_exe("git", git_mock());
    let log = env.temp.path().join("git.log");
    let socket_dir = tempfile::tempdir_in("/tmp").unwrap();
    let socket = socket_dir.path().join("niri.sock");
    if let Err(err) = start_niri_socket(&socket) {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("failed to start fake niri socket: {err}");
    }

    env.cmd()
        .env("NIRI_SOCKET", socket)
        .env("GIT_MOCK_LOG", &log)
        .args(["create-branch", "--repo"])
        .arg(env.repo_path())
        .args(["--from", "origin/feature", "new-feature"])
        .assert()
        .success();

    let logged = fs::read_to_string(log).unwrap();
    assert!(logged.contains("worktree add"));
    assert!(logged.contains("-b new-feature origin/feature"));
}

#[test]
fn create_worktree_creates_existing_branch_worktree_and_focuses_it() {
    let env = TestEnv::new();
    env.write_stores();
    env.write_exe("git", git_mock());
    let log = env.temp.path().join("git.log");
    let socket_dir = tempfile::tempdir_in("/tmp").unwrap();
    let socket = socket_dir.path().join("niri.sock");
    if let Err(err) = start_niri_socket(&socket) {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("failed to start fake niri socket: {err}");
    }

    env.cmd()
        .env("NIRI_SOCKET", socket)
        .env("GIT_MOCK_LOG", &log)
        .args(["create-worktree", "--repo"])
        .arg(env.repo_path())
        .arg("feature")
        .assert()
        .success();

    let logged = fs::read_to_string(log).unwrap();
    assert!(logged.contains("worktree add"));
    assert!(logged.contains(" feature"));
}

#[test]
fn create_worktree_fails_when_local_branch_is_missing() {
    let env = TestEnv::new();
    env.write_stores();
    env.write_exe("git", git_mock());

    env.cmd()
        .args(["create-worktree", "--repo"])
        .arg(env.repo_path())
        .arg("missing")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Local branch missing does not exist"));
}

#[test]
fn create_worktree_requires_repo_argument() {
    let env = TestEnv::new();

    env.cmd()
        .args(["create-worktree", "feature"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--repo"));
}

#[test]
fn create_branch_runs_setup_directly_in_worktree() {
    let env = TestEnv::new();
    env.write_stores_with_repo(json!({
        "path": env.repo_path(),
        "bare": false,
        "setup": ["script-mock", "setup-arg"]
    }));
    env.write_exe("git", git_mock());
    env.write_exe("script-mock", script_mock());
    let log = env.temp.path().join("script.log");
    let socket_dir = tempfile::tempdir_in("/tmp").unwrap();
    let socket = socket_dir.path().join("niri.sock");
    if let Err(err) = start_niri_socket(&socket) {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("failed to start fake niri socket: {err}");
    }

    env.cmd()
        .env("NIRI_SOCKET", socket)
        .env("GIT_MOCK_LOG", env.temp.path().join("git.log"))
        .env("SCRIPT_MOCK_LOG", &log)
        .args(["create-branch", "--repo"])
        .arg(env.repo_path())
        .args(["--from", "origin/feature", "new-feature"])
        .assert()
        .success();

    let logged = fs::read_to_string(log).unwrap();
    assert!(logged.contains(&format!("{}:setup-arg", env.temp.path().join("new-feature").display())));
}

#[test]
fn create_branch_fails_when_setup_fails() {
    let env = TestEnv::new();
    env.write_stores_with_repo(json!({
        "path": env.repo_path(),
        "bare": false,
        "setup": ["script-mock"]
    }));
    env.write_exe("git", git_mock());
    env.write_exe("script-mock", script_mock());
    let socket_dir = tempfile::tempdir_in("/tmp").unwrap();
    let socket = socket_dir.path().join("niri.sock");
    if let Err(err) = start_niri_socket(&socket) {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("failed to start fake niri socket: {err}");
    }

    env.cmd()
        .env("NIRI_SOCKET", socket)
        .env("GIT_MOCK_LOG", env.temp.path().join("git.log"))
        .env("SCRIPT_MOCK_LOG", env.temp.path().join("script.log"))
        .env("SCRIPT_MOCK_FAIL", "1")
        .args(["create-branch", "--repo"])
        .arg(env.repo_path())
        .args(["--from", "origin/feature", "new-feature"])
        .assert()
        .code(14)
        .stderr(predicate::str::contains("Setup command failed with exit code 42"));
}

#[test]
fn remove_worktree_fails_when_teardown_fails() {
    let env = TestEnv::new();
    env.write_stores_with_repo(json!({
        "path": env.repo_path(),
        "bare": false,
        "teardown": ["script-mock"]
    }));
    env.write_exe("git", git_mock());
    env.write_exe("script-mock", script_mock());
    let socket_dir = tempfile::tempdir_in("/tmp").unwrap();
    let socket = socket_dir.path().join("niri.sock");
    if let Err(err) = start_niri_socket(&socket) {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("failed to start fake niri socket: {err}");
    }

    env.cmd()
        .env("NIRI_SOCKET", socket)
        .env("SCRIPT_MOCK_LOG", env.temp.path().join("script.log"))
        .env("SCRIPT_MOCK_FAIL", "1")
        .args(["remove-worktree", "--json", "--worktree"])
        .arg(env.worktree_path())
        .assert()
        .code(12)
        .stderr(predicate::str::contains("\"code\": \"teardown_failed\""))
        .stderr(predicate::str::contains("\"exit_code\": 42"));
}

#[test]
fn set_repo_then_list_repos_json_uses_mock_git() {
    let env = TestEnv::new();
    fs::create_dir_all(env.repo_path()).unwrap();
    env.write_exe("git", git_mock());

    env.cmd()
        .args(["set-repo", "--repo"])
        .arg(env.repo_path())
        .assert()
        .success();

    env.cmd()
        .args(["list-repos", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"repo_origin\": \"git@example.com:repo.git\""))
        .stdout(predicate::str::contains("\"bare\": false"))
        .stdout(predicate::str::contains("\"list_pull_requests\": false"));
}

#[test]
fn set_repo_list_pull_requests_true_round_trips_through_list_repos_json() {
    let env = TestEnv::new();
    fs::create_dir_all(env.repo_path()).unwrap();
    env.write_exe("git", git_mock());

    env.cmd()
        .args(["set-repo", "--repo"])
        .arg(env.repo_path())
        .args(["--list-pull-requests", "true"])
        .assert()
        .success();

    env.cmd()
        .args(["list-repos", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"list_pull_requests\": true"));
}

#[test]
fn set_repo_setup_sets_and_unsets_setup_command_array() {
    let env = TestEnv::new();
    fs::create_dir_all(env.repo_path()).unwrap();
    env.write_exe("git", git_mock());

    env.cmd()
        .args(["set-repo", "--repo"])
        .arg(env.repo_path())
        .assert()
        .success();

    env.cmd()
        .args(["set-repo-setup", "--repo"])
        .arg(env.repo_path())
        .args(["--", "cmd", "arg one", "arg2"])
        .assert()
        .success();

    let store_path = env.data.join("niri-worktrees/repos.json");
    let store: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&store_path).unwrap()).unwrap();
    assert_eq!(store["repos"][0]["setup"], json!(["cmd", "arg one", "arg2"]));

    env.cmd()
        .args(["set-repo-setup", "--repo"])
        .arg(env.repo_path())
        .assert()
        .success();

    let store: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(store_path).unwrap()).unwrap();
    assert!(store["repos"][0].get("setup").is_none());
}

#[test]
fn set_repo_teardown_sets_and_unsets_teardown_command_array() {
    let env = TestEnv::new();
    fs::create_dir_all(env.repo_path()).unwrap();
    env.write_exe("git", git_mock());

    env.cmd()
        .args(["set-repo", "--repo"])
        .arg(env.repo_path())
        .assert()
        .success();

    env.cmd()
        .args(["set-repo-teardown", "--repo"])
        .arg(env.repo_path())
        .args(["--", "cmd", "arg"])
        .assert()
        .success();

    let store_path = env.data.join("niri-worktrees/repos.json");
    let store: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&store_path).unwrap()).unwrap();
    assert_eq!(store["repos"][0]["teardown"], json!(["cmd", "arg"]));

    env.cmd()
        .args(["set-repo-teardown", "--repo"])
        .arg(env.repo_path())
        .assert()
        .success();

    let store: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(store_path).unwrap()).unwrap();
    assert!(store["repos"][0].get("teardown").is_none());
}

#[test]
fn set_and_unset_workspace_write_runtime_store() {
    let env = TestEnv::new();
    fs::create_dir_all(env.worktree_path()).unwrap();

    env.cmd()
        .args(["set-workspace", "--worktree"])
        .arg(env.worktree_path())
        .args(["--workspace-id", "44"])
        .assert()
        .success();

    let store_path = env.runtime.join("niri-worktrees/worktrees.json");
    let text = fs::read_to_string(&store_path).unwrap();
    assert!(text.contains("\"workspace_id\": 44"));

    env.cmd()
        .args(["unset-workspace", "--worktree"])
        .arg(env.worktree_path())
        .assert()
        .success();
    let text = fs::read_to_string(store_path).unwrap();
    assert!(text.contains("\"worktrees\": []"));
}

#[test]
fn set_workspace_takes_workspace_from_existing_worktree() {
    let env = TestEnv::new();
    let old_worktree = env.temp.path().join("old");
    let new_worktree = env.temp.path().join("new");
    fs::create_dir_all(&old_worktree).unwrap();
    fs::create_dir_all(&new_worktree).unwrap();

    env.cmd()
        .args(["set-workspace", "--worktree"])
        .arg(&old_worktree)
        .args(["--workspace-id", "10"])
        .assert()
        .success();

    env.cmd()
        .args(["set-workspace", "--worktree"])
        .arg(&new_worktree)
        .args(["--workspace-id", "10"])
        .assert()
        .success();

    let store_path = env.runtime.join("niri-worktrees/worktrees.json");
    let store: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(store_path).unwrap()).unwrap();
    let worktrees = store["worktrees"].as_array().unwrap();
    assert_eq!(worktrees.len(), 1);
    assert_eq!(worktrees[0]["path"], new_worktree.display().to_string());
    assert_eq!(worktrees[0]["workspace_id"], 10);
}

#[test]
fn get_worktree_prints_worktree_for_explicit_workspace_id() {
    let env = TestEnv::new();
    env.write_stores();

    env.cmd()
        .args(["get-worktree", "--workspace-id", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("{}\n", env.worktree_path().display())));
}

#[test]
fn get_worktree_uses_focused_workspace_by_default() {
    let env = TestEnv::new();
    env.write_stores();
    let socket_dir = tempfile::tempdir_in("/tmp").unwrap();
    let socket = socket_dir.path().join("niri.sock");
    if let Err(err) = start_niri_socket(&socket) {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("failed to start fake niri socket: {err}");
    }

    env.cmd()
        .env("NIRI_SOCKET", socket)
        .arg("get-worktree")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!("{}\n", env.worktree_path().display())));
}

#[test]
fn get_worktree_fails_when_workspace_has_no_worktree() {
    let env = TestEnv::new();
    env.write_stores();

    env.cmd()
        .args(["get-worktree", "--workspace-id", "99"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("No worktree is stored for workspace 99"));
}

#[test]
fn list_branches_json_uses_git_mocks() {
    let env = TestEnv::new();
    env.write_stores();
    env.write_exe("git", git_mock());

    env.cmd()
        .args(["list-branches", "--json", "--repo"])
        .arg(env.repo_path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"local_branch\": \"feature\""))
        .stdout(predicate::str::contains("\"remote_branch\": \"origin/feature\""))
        .stdout(predicate::str::contains("\"remote_branch\": \"origin\"").not())
        .stdout(predicate::str::contains("\"workspace_id\": 2"));
}

#[test]
fn list_worktrees_json_uses_niri_socket_and_git_mocks() {
    let env = TestEnv::new();
    env.write_stores();
    env.write_exe("git", git_mock());
    let socket_dir = tempfile::tempdir_in("/tmp").unwrap();
    let socket = socket_dir.path().join("niri.sock");
    if let Err(err) = start_niri_socket(&socket) {
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("failed to start fake niri socket: {err}");
    }

    env.cmd()
        .env("NIRI_SOCKET", socket)
        .args(["list-worktrees", "--json", "--repo"])
        .arg(env.repo_path())
        .assert()
        .success()
        .stdout(predicate::str::contains("\"workspaces\"").not())
        .stdout(predicate::str::contains("\"workspace\""))
        .stdout(predicate::str::contains("\"windows\""))
        .stdout(predicate::str::contains("\"id\": 2"));
}
