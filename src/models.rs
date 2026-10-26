use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShelfCategory {
    Cmds,
    Settings,
    Apps,
    Files,
    Web,
}

impl ShelfCategory {
    pub fn from_label(label: &str) -> Self {
        match label.trim().to_ascii_lowercase().as_str() {
            "cmds" | "cmd" | "commands" | "command" => Self::Cmds,
            "settings" | "setting" => Self::Settings,
            "apps" | "app" | "applications" => Self::Apps,
            "files" | "file" => Self::Files,
            "web" | "bang" | "search" => Self::Web,
            _ => Self::Apps,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cmds => "Cmds",
            Self::Settings => "Settings",
            Self::Apps => "Apps",
            Self::Files => "Files",
            Self::Web => "Web",
        }
    }

    pub fn header_title(&self) -> &'static str {
        match self {
            Self::Cmds => "SYSTEM COMMANDS",
            Self::Settings => "SYSTEM SETTINGS",
            Self::Apps => "APPLICATIONS",
            Self::Files => "CUSTOM FILES",
            Self::Web => "WEB SEARCH",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Cmds => "System Commands",
            Self::Settings => "System Settings",
            Self::Apps => "Installed Applications",
            Self::Files => "Custom Scanned Files",
            Self::Web => "Web Search & Bangs",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Cmds => "Power actions, inline calculator, and custom commands",
            Self::Settings => "Windows ms-settings URIs, startup, and hotkey settings",
            Self::Apps => "Start Menu applications enabled in Installed Apps",
            Self::Files => "Direct file/folder paths and files from Custom File Scan paths",
            Self::Web => "Direct URLs, prefix-free bang shortcuts, and fallback web search",
        }
    }

    pub fn icon_glyph(&self) -> &'static str {
        match self {
            Self::Cmds => "⚡",
            Self::Settings => "⚙",
            Self::Apps => "❖",
            Self::Files => "📄",
            Self::Web => "🌐",
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExecutionTarget {
    SystemProcess {
        program: String,
        args: Vec<String>,
    },
    CustomCommand(String),
    SettingsUri(String),
    WebUrl(String),
    AppPath(PathBuf),
    FilePath(PathBuf),
    CopyToClipboard(String),
    ToggleStartup(bool),
    SetHotkey(String),
    OpenPaletteSettings,
}

#[derive(Debug, Clone)]
pub struct CandidateItem {
    pub title: String,
    pub badge: &'static str,
    pub category: ShelfCategory,
    pub can_elevate: bool,
    pub score: i64,
    pub target: ExecutionTarget,
}

#[derive(Debug, Clone)]
pub struct AppEntry {
    pub name: String,
    pub name_lower: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ScannedFileEntry {
    pub display_name: String,
    pub name_lower: String,
    pub path: PathBuf,
}
