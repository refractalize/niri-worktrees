import QtQuick
import qs.Common
import qs.Modules.Plugins

PluginSettings {
    pluginId: "niriWorktrees"

    StringSetting {
        settingKey: "binary"
        label: "niri-worktrees binary"
        description: "Command or absolute path used to run niri-worktrees"
        placeholder: "niri-worktrees"
        defaultValue: "niri-worktrees"
    }

    StringSetting {
        settingKey: "extraPath"
        label: "Extra PATH directories"
        description: "Colon-separated directories searched for niri-worktrees and gh, before DMS's own PATH"
        placeholder: "~/.local/bin:~/.local/share/mise/shims"
        defaultValue: "~/.local/bin:~/.local/share/mise/shims"
    }

    SelectionSetting {
        settingKey: "pollInterval"
        label: "Pull request refresh interval"
        description: "How often to refresh pull request status from GitHub"
        options: [
            { label: "30 seconds", value: "30" },
            { label: "1 minute", value: "60" },
            { label: "2 minutes", value: "120" },
            { label: "5 minutes", value: "300" }
        ]
        defaultValue: "60"
    }
}
