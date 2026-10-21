import QtQuick
import Quickshell
import Quickshell.Io
import qs.Common
import qs.Modules.Plugins
import qs.Services
import qs.Widgets

PluginComponent {
    id: root

    property var popoutService: null

    // PluginComponent only gained `textSize` after DMS 1.6, so compute it here.
    readonly property int labelSize: Theme.barTextSize(barThickness, barConfig?.fontScale, barConfig?.maximizeWidgetText)
    readonly property string binary: pluginData.binary || "niri-worktrees"
    // DMS usually runs without the user's shell PATH, so niri-worktrees (and the
    // gh it calls) need extra directories to be found.
    readonly property string extraPath: pluginData.extraPath ?? "~/.local/bin:~/.local/share/mise/shims"
    readonly property string searchPath: {
        const home = Quickshell.env("HOME") || "";
        const extra = extraPath.split(":").filter(Boolean).map(dir => dir.replace(/^~(?=\/|$)/, home));
        return extra.concat([Quickshell.env("PATH") || "/usr/local/bin:/usr/bin:/bin"]).join(":");
    }
    readonly property int pollSeconds: Number(pluginData.pollInterval || 60)
    readonly property string workspaceId: NiriService.focusedWorkspaceId ? String(NiriService.focusedWorkspaceId) : ""

    // The worktree on the focused workspace, from `worktree-status --json`, or null.
    property var worktree: null
    // Last known pull request per worktree path, so switching workspaces doesn't
    // blank the PR while GitHub is queried again.
    property var pullRequests: ({})
    readonly property var pullRequest: worktree ? (pullRequests[worktree.path] ?? null) : null

    function refresh() {
        if (!workspaceId) {
            worktree = null;
            return;
        }
        localStatus.request(workspaceId);
        fullStatus.request(workspaceId);
    }

    function stateColor(status) {
        switch ((status || "").toUpperCase()) {
        case "OPEN":
            return Theme.success;
        case "MERGED":
            return Theme.tertiary;
        case "CLOSED":
            return Theme.error;
        default:
            return Theme.widgetTextColor;
        }
    }

    function stateLabel(status) {
        switch ((status || "").toUpperCase()) {
        case "OPEN":
            return "Open";
        case "MERGED":
            return "Merged";
        case "CLOSED":
            return "Closed";
        default:
            return status || "";
        }
    }

    function checksIcon(status) {
        switch ((status || "").toUpperCase()) {
        case "SUCCESS":
            return "check_circle";
        case "FAILURE":
        case "ERROR":
            return "cancel";
        case "PENDING":
        case "EXPECTED":
            return "schedule";
        default:
            return "";
        }
    }

    function checksColor(status) {
        switch ((status || "").toUpperCase()) {
        case "SUCCESS":
            return Theme.success;
        case "FAILURE":
        case "ERROR":
            return Theme.error;
        default:
            return Theme.warning;
        }
    }

    onWorkspaceIdChanged: refresh()
    onBinaryChanged: refresh()
    onSearchPathChanged: refresh()
    onWorktreeChanged: setVisibilityOverride(worktree !== null)

    Component.onCompleted: {
        setVisibilityOverride(false);
        refresh();
    }

    pillClickAction: () => {
        if (!worktree || !pullRequest || pullRequest.pr_number === null)
            return;
        Quickshell.execDetached({
            "command": ["env", `PATH=${searchPath}`, "gh", "pr", "view", String(pullRequest.pr_number), "--web"],
            "workingDirectory": worktree.path
        });
    }
    pillRightClickAction: () => refresh()

    // Runs `niri-worktrees worktree-status --json` for one workspace at a time.
    // Requests made while a run is in flight are coalesced into one follow-up run.
    component WorktreeStatusProcess: Process {
        id: proc

        property string binary: "niri-worktrees"
        property string searchPath: ""
        property bool includePullRequest: true
        property string workspaceId: ""
        property string pendingWorkspaceId: ""

        signal loaded(string workspaceId, var worktree)

        function request(id) {
            if (running) {
                pendingWorkspaceId = id;
                return;
            }
            workspaceId = id;
            running = true;
        }

        command: {
            const args = ["env", `PATH=${searchPath}`, binary, "worktree-status", "--json", "--workspace-id", workspaceId];
            return includePullRequest ? args : args.concat(["--no-pull-request"]);
        }

        stdout: StdioCollector {
            onStreamFinished: {
                if (!text.trim())
                    return;
                try {
                    proc.loaded(proc.workspaceId, JSON.parse(text).worktree);
                } catch (e) {
                    console.warn("niriWorktrees: could not parse worktree-status output:", e);
                }
            }
        }

        stderr: StdioCollector {
            onStreamFinished: {
                if (text.trim())
                    console.warn("niriWorktrees:", text.trim());
            }
        }

        onExited: {
            if (!pendingWorkspaceId)
                return;
            const id = pendingWorkspaceId;
            pendingWorkspaceId = "";
            Qt.callLater(() => request(id));
        }
    }

    WorktreeStatusProcess {
        id: localStatus
        binary: root.binary
        searchPath: root.searchPath
        includePullRequest: false
        onLoaded: (id, worktree) => {
            if (id === root.workspaceId)
                root.worktree = worktree;
        }
    }

    WorktreeStatusProcess {
        id: fullStatus
        binary: root.binary
        searchPath: root.searchPath
        includePullRequest: true
        onLoaded: (id, worktree) => {
            if (worktree) {
                const next = Object.assign({}, root.pullRequests);
                next[worktree.path] = worktree.pull_request;
                root.pullRequests = next;
            }
            if (id === root.workspaceId)
                root.worktree = worktree;
        }
    }

    Timer {
        interval: root.pollSeconds * 1000
        repeat: true
        running: root.workspaceId !== "" && root._barRevealed
        onTriggered: fullStatus.request(root.workspaceId)
    }

    // Commits and edits happen without a workspace change, so poll the cheap
    // local status more often than GitHub.
    Timer {
        interval: 10000
        repeat: true
        running: root.workspaceId !== "" && root._barRevealed
        onTriggered: localStatus.request(root.workspaceId)
    }

    horizontalBarPill: Component {
        Row {
            spacing: Theme.spacingXS

            DankIcon {
                anchors.verticalCenter: parent.verticalCenter
                name: "account_tree"
                size: root.iconSize
                color: Theme.widgetIconColor
            }

            StyledText {
                anchors.verticalCenter: parent.verticalCenter
                text: root.worktree?.name ?? ""
                font.pixelSize: root.labelSize
                color: Theme.widgetTextColor
            }

            StyledText {
                anchors.verticalCenter: parent.verticalCenter
                visible: (root.worktree?.commits_ahead_of_default_branch ?? 0) > 0
                text: `↑${root.worktree?.commits_ahead_of_default_branch ?? 0}`
                font.pixelSize: root.labelSize
                color: Theme.widgetTextColor
            }

            StyledText {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.worktree?.is_dirty === true
                text: "●"
                font.pixelSize: root.labelSize
                color: Theme.warning
            }

            StyledText {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.pullRequest !== null
                text: root.pullRequest ? [root.pullRequest.pr_number !== null ? `#${root.pullRequest.pr_number}` : "", root.stateLabel(root.pullRequest.status)].filter(Boolean).join(" ") : ""
                font.pixelSize: root.labelSize
                color: root.stateColor(root.pullRequest?.status)
            }

            DankIcon {
                anchors.verticalCenter: parent.verticalCenter
                visible: name !== ""
                name: root.checksIcon(root.pullRequest?.checks_status)
                size: root.iconSize
                color: root.checksColor(root.pullRequest?.checks_status)
            }

            Row {
                anchors.verticalCenter: parent.verticalCenter
                visible: (root.pullRequest?.unresolved_review_comments ?? 0) > 0
                spacing: Theme.spacingXXS

                DankIcon {
                    anchors.verticalCenter: parent.verticalCenter
                    name: "chat"
                    size: root.iconSize
                    color: Theme.warning
                }

                StyledText {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.pullRequest?.unresolved_review_comments ?? ""
                    font.pixelSize: root.labelSize
                    color: Theme.warning
                }
            }
        }
    }

    verticalBarPill: Component {
        Column {
            spacing: Theme.spacingXXS

            DankIcon {
                anchors.horizontalCenter: parent.horizontalCenter
                name: "account_tree"
                size: root.iconSize
                color: root.pullRequest ? root.stateColor(root.pullRequest.status) : Theme.widgetIconColor
            }

            StyledText {
                anchors.horizontalCenter: parent.horizontalCenter
                visible: root.worktree?.is_dirty === true
                text: "●"
                font.pixelSize: root.labelSize
                color: Theme.warning
            }

            DankIcon {
                anchors.horizontalCenter: parent.horizontalCenter
                visible: name !== ""
                name: root.checksIcon(root.pullRequest?.checks_status)
                size: root.iconSize
                color: root.checksColor(root.pullRequest?.checks_status)
            }
        }
    }
}
