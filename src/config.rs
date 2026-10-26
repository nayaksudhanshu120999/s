use crate::models::ShelfCategory;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_CONFIG_TOML: &str = r##"show_apps = true
run_on_startup = false
hotkey = "Ctrl+Space"
theme = "powertoys"
custom_cmd_path = "cmd.exe"
custom_system_settings_path = "ms-settings:"
category_order = ["Cmds", "Settings", "Apps", "Files", "Web"]
hidden_apps = []
custom_file_scan_paths = []
custom_commands = []
custom_settings = []

[custom_theme]
window_bg = "#1f1f1f"
surface_bg = "#2b2b2b"
card_bg = "#282828"
selected_bg = "#383838"
border = "#3d3d3d"
accent = "#60cdff"
text_primary = "#ffffff"
text_muted = "#9d9d9d"

[bangs]
yt = "https://www.youtube.com/results?search_query={q}"
g = "https://www.google.com/search?q={q}"
ddg = "https://duckduckgo.com/?q={q}"
gh = "https://github.com/search?q={q}"
rs = "https://docs.rs/releases/search?query={q}"
crates = "https://crates.io/search?q={q}"
wiki = "https://en.wikipedia.org/w/index.php?search={q}"
so = "https://stackoverflow.com/search?q={q}"
r = "https://www.reddit.com/search/?q={q}"
maps = "https://www.google.com/maps/search/{q}"
"##;

fn default_true() -> bool {
    true
}

fn default_hotkey() -> String {
    "Ctrl+Space".to_string()
}

fn default_theme() -> String {
    "powertoys".to_string()
}

fn default_cmd_path() -> String {
    "cmd.exe".to_string()
}

fn default_settings_path() -> String {
    "ms-settings:".to_string()
}

fn default_category_order() -> Vec<String> {
    vec![
        "Cmds".to_string(),
        "Settings".to_string(),
        "Apps".to_string(),
        "Files".to_string(),
        "Web".to_string(),
    ]
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomThemeConfig {
    pub window_bg: String,
    pub surface_bg: String,
    pub card_bg: String,
    pub selected_bg: String,
    pub border: String,
    pub accent: String,
    pub text_primary: String,
    pub text_muted: String,
}

impl Default for CustomThemeConfig {
    fn default() -> Self {
        Self {
            window_bg: "#1f1f1f".to_string(),
            surface_bg: "#2b2b2b".to_string(),
            card_bg: "#282828".to_string(),
            selected_bg: "#383838".to_string(),
            border: "#3d3d3d".to_string(),
            accent: "#60cdff".to_string(),
            text_primary: "#ffffff".to_string(),
            text_muted: "#9d9d9d".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomCommandItem {
    pub name: String,
    pub command: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomSettingItem {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    #[serde(default = "default_true")]
    pub show_apps: bool,
    #[serde(default)]
    pub run_on_startup: bool,
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default)]
    pub custom_theme: CustomThemeConfig,
    #[serde(default = "default_cmd_path")]
    pub custom_cmd_path: String,
    #[serde(default = "default_settings_path")]
    pub custom_system_settings_path: String,
    #[serde(default = "default_category_order")]
    pub category_order: Vec<String>,
    #[serde(default)]
    pub hidden_apps: Vec<String>,
    #[serde(default)]
    pub custom_file_scan_paths: Vec<String>,
    #[serde(default)]
    pub custom_commands: Vec<CustomCommandItem>,
    #[serde(default)]
    pub custom_settings: Vec<CustomSettingItem>,
    #[serde(default)]
    pub bangs: HashMap<String, String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        toml::from_str(DEFAULT_CONFIG_TOML).unwrap_or(Self {
            show_apps: true,
            run_on_startup: false,
            hotkey: default_hotkey(),
            theme: default_theme(),
            custom_theme: CustomThemeConfig::default(),
            custom_cmd_path: default_cmd_path(),
            custom_system_settings_path: default_settings_path(),
            category_order: default_category_order(),
            hidden_apps: Vec::new(),
            custom_file_scan_paths: Vec::new(),
            custom_commands: Vec::new(),
            custom_settings: Vec::new(),
            bangs: HashMap::new(),
        })
    }
}

impl AppConfig {
    /// Resolves relative config paths (such as `"config.toml"`) next to the executable
    /// if not found in the current working directory (critical when launched at Windows Startup).
    pub fn resolve_config_path<P: AsRef<Path>>(path: P) -> PathBuf {
        let p = path.as_ref();
        if p.is_absolute() || p.exists() {
            return p.to_path_buf();
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                return dir.join(p);
            }
        }
        p.to_path_buf()
    }

    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Self {
        let resolved = Self::resolve_config_path(path);

        if !resolved.exists() {
            let _ = fs::write(&resolved, DEFAULT_CONFIG_TOML);
        }

        let raw_content =
            fs::read_to_string(&resolved).unwrap_or_else(|_| DEFAULT_CONFIG_TOML.to_string());

        let mut parsed: AppConfig = toml::from_str(&raw_content).unwrap_or_default();
        parsed.normalize();
        crate::system::apply_runtime_system_config(parsed.run_on_startup, &parsed.hotkey);
        parsed
    }

    pub fn save<P: AsRef<Path>>(&mut self, path: P) {
        self.normalize();
        let resolved = Self::resolve_config_path(path);
        if let Ok(serialized) = toml::to_string_pretty(self) {
            let _ = fs::write(resolved, serialized);
        }
        crate::system::apply_runtime_system_config(self.run_on_startup, &self.hotkey);
    }

    pub fn normalize(&mut self) {
        self.hotkey = crate::system::normalize_hotkey_str(&self.hotkey);

        self.bangs = self
            .bangs
            .drain()
            .filter(|(k, v)| !k.trim().is_empty() && !v.trim().is_empty())
            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
            .collect();

        let t = self.theme.trim().to_ascii_lowercase();
        self.theme = if t == "custom" {
            "custom".to_string()
        } else {
            "powertoys".to_string()
        };

        if self.custom_cmd_path.trim().is_empty() {
            self.custom_cmd_path = default_cmd_path();
        } else {
            self.custom_cmd_path = self.custom_cmd_path.trim().to_string();
        }

        if self.custom_system_settings_path.trim().is_empty() {
            self.custom_system_settings_path = default_settings_path();
        } else {
            self.custom_system_settings_path = self.custom_system_settings_path.trim().to_string();
        }

        let mut ordered = Vec::with_capacity(5);
        for item in &self.category_order {
            let cat = ShelfCategory::from_label(item).as_str().to_string();
            if !ordered.contains(&cat) {
                ordered.push(cat);
            }
        }
        for fallback in ["Cmds", "Settings", "Apps", "Files", "Web"] {
            let s = fallback.to_string();
            if !ordered.contains(&s) {
                ordered.push(s);
            }
        }
        self.category_order = ordered;

        let mut clean_hidden = Vec::new();
        for app in &self.hidden_apps {
            let trimmed = app.trim();
            if !trimmed.is_empty()
                && !clean_hidden
                    .iter()
                    .any(|h: &String| h.eq_ignore_ascii_case(trimmed))
            {
                clean_hidden.push(trimmed.to_string());
            }
        }
        self.hidden_apps = clean_hidden;

        let mut clean_paths = Vec::new();
        for p in &self.custom_file_scan_paths {
            let trimmed = p.trim();
            if !trimmed.is_empty()
                && !clean_paths
                    .iter()
                    .any(|existing: &String| existing.eq_ignore_ascii_case(trimmed))
            {
                clean_paths.push(trimmed.to_string());
            }
        }
        self.custom_file_scan_paths = clean_paths;
    }

    pub fn ordered_categories(&self) -> Vec<ShelfCategory> {
        self.category_order
            .iter()
            .map(|s| ShelfCategory::from_label(s))
            .collect()
    }

    pub fn is_app_hidden(&self, app_name: &str) -> bool {
        self.hidden_apps
            .iter()
            .any(|h| h.eq_ignore_ascii_case(app_name.trim()))
    }

    pub fn set_app_visibility(&mut self, app_name: &str, visible: bool) {
        let trimmed = app_name.trim();
        if visible {
            self.hidden_apps
                .retain(|h| !h.eq_ignore_ascii_case(trimmed));
        } else if !self.is_app_hidden(trimmed) {
            self.hidden_apps.push(trimmed.to_string());
        }
    }
}
