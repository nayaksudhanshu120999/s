use crate::config::AppConfig;
use crate::models::{AppEntry, CandidateItem, ExecutionTarget, ScannedFileEntry, ShelfCategory};
use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use std::env;
use std::path::{Path, PathBuf};

const MAX_TOTAL_RESULTS: usize = 24;

struct StaticCommandSpec {
    title: &'static str,
    badge: &'static str,
    category: ShelfCategory,
    triggers: &'static [&'static str],
    program: &'static str,
    args: &'static [&'static str],
    settings_uri: Option<&'static str>,
    is_palette_settings: bool,
}

const PRIORITY_SPECS: &[StaticCommandSpec] = &[
    StaticCommandSpec {
        title: "Command Palette Settings",
        badge: "SETTINGS",
        category: ShelfCategory::Settings,
        triggers: &[
            "palette settings",
            "preferences",
            "configure palette",
            "cmdpal settings",
            "theme",
        ],
        program: "",
        args: &[],
        settings_uri: None,
        is_palette_settings: true,
    },
    StaticCommandSpec {
        title: "Shutdown Computer",
        badge: "COMMAND",
        category: ShelfCategory::Cmds,
        triggers: &["shutdown", "power off", "turn off"],
        program: "shutdown",
        args: &["/s", "/t", "0"],
        settings_uri: None,
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Restart Computer",
        badge: "COMMAND",
        category: ShelfCategory::Cmds,
        triggers: &["restart", "reboot"],
        program: "shutdown",
        args: &["/r", "/t", "0"],
        settings_uri: None,
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Sleep Computer",
        badge: "COMMAND",
        category: ShelfCategory::Cmds,
        triggers: &["sleep", "suspend", "hibernate"],
        program: "rundll32.exe",
        args: &["powrprof.dll,SetSuspendState", "0,1,0"],
        settings_uri: None,
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Lock Workstation",
        badge: "COMMAND",
        category: ShelfCategory::Cmds,
        triggers: &["lock", "lock screen", "lock pc"],
        program: "rundll32.exe",
        args: &["user32.dll,LockWorkStation"],
        settings_uri: None,
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "System Settings Home",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["system settings", "windows settings", "settings"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Display Settings",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["display settings", "display", "resolution", "monitor"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:display"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Sound & Audio Settings",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["sound settings", "sound", "audio", "volume", "microphone"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:sound"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Network & Internet Settings",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["network settings", "network", "ethernet", "internet"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:network"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Wi-Fi Settings",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["wifi", "wi-fi", "wireless", "wifi settings"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:network-wifi"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Bluetooth & Devices",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["bluetooth", "devices", "bluetooth settings"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:bluetooth"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Installed Apps & Features",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["installed apps", "apps features", "add or remove programs"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:appsfeatures"),
        is_palette_settings: false,
    },
    StaticCommandSpec {
        title: "Windows Update",
        badge: "SETTING",
        category: ShelfCategory::Settings,
        triggers: &["windows update", "update", "check for updates"],
        program: "",
        args: &[],
        settings_uri: Some("ms-settings:windowsupdate"),
        is_palette_settings: false,
    },
];

const HOTKEY_PRESETS: &[&str] = &[
    "Ctrl+Space",
    "Alt+Space",
    "Ctrl+Shift+Space",
    "Ctrl+Alt+Space",
    "Alt+Q",
];

const COMMON_WEB_TLDS: &[&str] = &[
    "com", "org", "net", "io", "rs", "dev", "ai", "in", "co", "edu", "gov", "me", "app", "gg",
    "tv", "xyz", "info", "sh", "tech", "cloud", "page", "site", "uk", "de", "fr", "jp", "ca", "au",
];

pub struct QueryEngine {
    pub config: AppConfig,
    pub apps: Vec<AppEntry>,
    pub custom_files: Vec<ScannedFileEntry>,
    matcher: SkimMatcherV2,
}

impl QueryEngine {
    pub fn new(
        config: AppConfig,
        apps: Vec<AppEntry>,
        custom_files: Vec<ScannedFileEntry>,
    ) -> Self {
        Self {
            config,
            apps,
            custom_files,
            matcher: SkimMatcherV2::default(),
        }
    }

    /// Detects when the user types `<bang> ` (e.g. `"yt "`) to activate the Bang Pill Chip.
    pub fn extract_bang_prefix(&self, raw_input: &str) -> Option<(String, String)> {
        let (first_token, remainder) = raw_input.split_once(char::is_whitespace)?;
        let key = first_token.trim().to_ascii_lowercase();
        if self.config.bangs.contains_key(&key) {
            Some((key, remainder.trim_start().to_string()))
        } else {
            None
        }
    }

    pub fn evaluate_with_bang(&self, active_bang: &str, raw_input: &str) -> Vec<CandidateItem> {
        let bang_key = active_bang.trim().to_ascii_lowercase();

        if !bang_key.is_empty() {
            if let Some(url_template) = self.config.bangs.get(&bang_key) {
                let query = raw_input.trim();
                let encoded = urlencoding::encode(query);
                let target_url = url_template.replace("{q}", &encoded);
                let title = if query.is_empty() {
                    format!(
                        "Type a query to search {}...",
                        bang_key.to_ascii_uppercase()
                    )
                } else {
                    format!("Search {} for \"{}\"", bang_key.to_ascii_uppercase(), query)
                };

                return vec![CandidateItem {
                    title,
                    badge: "BANG",
                    category: ShelfCategory::Web,
                    can_elevate: false,
                    score: 10_000,
                    target: ExecutionTarget::WebUrl(target_url),
                }];
            }
        }

        let trimmed = raw_input.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }

        let normalized = trimmed.to_ascii_lowercase();
        let mut all_matches = Vec::with_capacity(36);

        // 1. Direct File / Folder Path Detection
        let direct_path_hit = self.detect_direct_path(trimmed);
        if let Some(item) = direct_path_hit.clone() {
            all_matches.push(item);
        }

        // 2. Direct URL Detection (if not a local path)
        let direct_url_hit = if direct_path_hit.is_none() {
            self.detect_direct_url(trimmed)
        } else {
            None
        };
        if let Some(item) = direct_url_hit.clone() {
            all_matches.push(item);
        }

        // 3. Inline Calculator & Base Converter (if not a direct path/URL)
        if direct_path_hit.is_none() && direct_url_hit.is_none() {
            if let Some(calc_item) = evaluate_calculator(trimmed) {
                all_matches.push(calc_item);
            }
        }

        // 4. Standard Fuzzy Shelves
        all_matches.extend(self.match_commands(&normalized, trimmed));
        all_matches.extend(self.match_settings(&normalized, trimmed));
        all_matches.extend(self.match_apps(&normalized, trimmed));
        all_matches.extend(self.match_custom_files(&normalized, trimmed));

        let cat_order = self.config.ordered_categories();
        let cat_rank = |cat: ShelfCategory| -> usize {
            cat_order.iter().position(|&c| c == cat).unwrap_or(99)
        };

        all_matches.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| cat_rank(a.category).cmp(&cat_rank(b.category)))
                .then_with(|| a.title.cmp(&b.title))
        });

        all_matches.truncate(MAX_TOTAL_RESULTS);

        // Fallback Google search (only if input wasn't already a direct URL)
        if direct_url_hit.is_none() {
            let encoded = urlencoding::encode(trimmed);
            let google_url = format!("https://www.google.com/search?q={}", encoded);
            all_matches.push(CandidateItem {
                title: format!("Search Google for \"{}\"", trimmed),
                badge: "WEB",
                category: ShelfCategory::Web,
                can_elevate: false,
                score: 0,
                target: ExecutionTarget::WebUrl(google_url),
            });
        }

        all_matches
    }

    fn detect_direct_path(&self, raw: &str) -> Option<CandidateItem> {
        let unquoted = raw.trim().trim_matches('"').trim();
        if unquoted.is_empty() {
            return None;
        }

        let expanded = expand_env_and_tilde(unquoted);
        let bytes = expanded.as_bytes();

        let looks_like_drive = bytes.len() >= 2
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes.len() == 2 || bytes[2] == b'\\' || bytes[2] == b'/');
        let looks_like_unc = expanded.starts_with(r"\\");
        let looks_like_env_or_tilde = unquoted.starts_with('%') || unquoted.starts_with('~');

        if !looks_like_drive && !looks_like_unc && !looks_like_env_or_tilde {
            return None;
        }

        let normalized_path = if bytes.len() == 2 && bytes[1] == b':' {
            format!("{}\\", expanded)
        } else {
            expanded
        };

        let path = PathBuf::from(&normalized_path);
        if path.exists() {
            let is_dir = path.is_dir();
            let title = if is_dir {
                format!("Open Folder: {}", path.display())
            } else {
                format!("Open File: {}", path.display())
            };
            return Some(CandidateItem {
                title,
                badge: if is_dir { "FOLDER" } else { "FILE" },
                category: ShelfCategory::Files,
                can_elevate: !is_dir,
                score: 25_000,
                target: ExecutionTarget::FilePath(path),
            });
        }

        // If user is mid-typing a sub-path, offer to open the existing parent directory
        if let Some(parent) = Path::new(&normalized_path).parent() {
            if !parent.as_os_str().is_empty() && parent.exists() && parent.is_dir() {
                return Some(CandidateItem {
                    title: format!("Open Parent Folder: {}", parent.display()),
                    badge: "FOLDER",
                    category: ShelfCategory::Files,
                    can_elevate: false,
                    score: 24_000,
                    target: ExecutionTarget::FilePath(parent.to_path_buf()),
                });
            }
        }

        None
    }

    fn detect_direct_url(&self, raw: &str) -> Option<CandidateItem> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
            return None;
        }

        let lower = trimmed.to_ascii_lowercase();

        if lower.starts_with("https://") || lower.starts_with("http://") {
            let after_scheme = &trimmed[trimmed.find("://")? + 3..];
            if !after_scheme.is_empty() {
                return Some(CandidateItem {
                    title: format!("Open URL: {}", trimmed),
                    badge: "URL",
                    category: ShelfCategory::Web,
                    can_elevate: false,
                    score: 25_000,
                    target: ExecutionTarget::WebUrl(trimmed.to_string()),
                });
            }
        }

        if lower.starts_with("localhost:")
            || lower == "localhost"
            || lower.starts_with("127.0.0.1:")
        {
            let target = format!("http://{}", trimmed);
            return Some(CandidateItem {
                title: format!("Open Local URL: {}", target),
                badge: "URL",
                category: ShelfCategory::Web,
                can_elevate: false,
                score: 25_000,
                target: ExecutionTarget::WebUrl(target),
            });
        }

        if lower.starts_with("www.") && trimmed.len() > 4 && trimmed[4..].contains('.') {
            let target = format!("https://{}", trimmed);
            return Some(CandidateItem {
                title: format!("Open URL: {}", target),
                badge: "URL",
                category: ShelfCategory::Web,
                can_elevate: false,
                score: 25_000,
                target: ExecutionTarget::WebUrl(target),
            });
        }

        // Bare domain check (e.g. "github.com/rust-lang" or "crates.io")
        let host_part = lower.split(&['/', '?', '#'][..]).next().unwrap_or("");
        let host_without_port = host_part.split(':').next().unwrap_or("");
        if host_without_port.contains('.')
            && !host_without_port.starts_with('.')
            && !host_without_port.ends_with('.')
        {
            let labels: Vec<&str> = host_without_port.split('.').collect();
            if labels.len() >= 2
                && labels.iter().all(|l| {
                    !l.is_empty()
                        && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                        && !l.starts_with('-')
                        && !l.ends_with('-')
                })
            {
                let tld = *labels.last().unwrap();
                if COMMON_WEB_TLDS.contains(&tld) {
                    let target = format!("https://{}", trimmed);
                    return Some(CandidateItem {
                        title: format!("Open URL: {}", target),
                        badge: "URL",
                        category: ShelfCategory::Web,
                        can_elevate: false,
                        score: 24_500,
                        target: ExecutionTarget::WebUrl(target),
                    });
                }
            }
        }

        None
    }

    fn compute_fuzzy_score(
        &self,
        candidate_name: &str,
        candidate_lower: &str,
        query_lower: &str,
        raw_query: &str,
    ) -> Option<i64> {
        let mut score = self.matcher.fuzzy_match(candidate_name, raw_query)?;

        if candidate_lower == query_lower {
            score += 500;
        } else if candidate_lower.starts_with(query_lower) {
            score += 220 - (candidate_lower.len() as i64).min(80);
        } else if candidate_lower.contains(query_lower) {
            score += 90 - (candidate_lower.len() as i64).min(50);
        }

        Some(score)
    }

    fn match_apps(&self, normalized_query: &str, raw_query: &str) -> Vec<CandidateItem> {
        if !self.config.show_apps {
            return Vec::new();
        }

        self.apps
            .iter()
            .filter(|app| !self.config.is_app_hidden(&app.name))
            .filter_map(|app| {
                let score = self.compute_fuzzy_score(
                    &app.name,
                    &app.name_lower,
                    normalized_query,
                    raw_query,
                )?;
                Some(CandidateItem {
                    title: app.name.clone(),
                    badge: "APP",
                    category: ShelfCategory::Apps,
                    can_elevate: true,
                    score,
                    target: ExecutionTarget::AppPath(app.path.clone()),
                })
            })
            .collect()
    }

    fn match_custom_files(&self, normalized_query: &str, raw_query: &str) -> Vec<CandidateItem> {
        self.custom_files
            .iter()
            .filter_map(|file| {
                let score = self.compute_fuzzy_score(
                    &file.display_name,
                    &file.name_lower,
                    normalized_query,
                    raw_query,
                )?;
                Some(CandidateItem {
                    title: file.display_name.clone(),
                    badge: "FILE",
                    category: ShelfCategory::Files,
                    can_elevate: false,
                    score,
                    target: ExecutionTarget::FilePath(file.path.clone()),
                })
            })
            .collect()
    }

    fn match_commands(&self, normalized_query: &str, raw_query: &str) -> Vec<CandidateItem> {
        let mut results = Vec::new();

        for spec in PRIORITY_SPECS
            .iter()
            .filter(|s| s.category == ShelfCategory::Cmds)
        {
            if let Some(score) =
                self.score_triggers_and_title(normalized_query, raw_query, spec.title, spec.triggers)
            {
                results.push(CandidateItem {
                    title: spec.title.to_string(),
                    badge: spec.badge,
                    category: ShelfCategory::Cmds,
                    can_elevate: true,
                    score,
                    target: ExecutionTarget::SystemProcess {
                        program: spec.program.to_string(),
                        args: spec.args.iter().map(|s| (*s).to_string()).collect(),
                    },
                });
            }
        }

        for custom in &self.config.custom_commands {
            let trigger_slice = [custom.name.as_str()];
            if let Some(score) = self.score_triggers_and_title(
                normalized_query,
                raw_query,
                &custom.name,
                &trigger_slice,
            ) {
                results.push(CandidateItem {
                    title: custom.name.clone(),
                    badge: "COMMAND",
                    category: ShelfCategory::Cmds,
                    can_elevate: true,
                    score,
                    target: ExecutionTarget::CustomCommand(custom.command.clone()),
                });
            }
        }

        results
    }

    fn match_settings(&self, normalized_query: &str, raw_query: &str) -> Vec<CandidateItem> {
        let mut results = Vec::new();

        for spec in PRIORITY_SPECS
            .iter()
            .filter(|s| s.category == ShelfCategory::Settings)
        {
            if let Some(score) =
                self.score_triggers_and_title(normalized_query, raw_query, spec.title, spec.triggers)
            {
                let target = if spec.is_palette_settings {
                    ExecutionTarget::OpenPaletteSettings
                } else {
                    let uri = spec.settings_uri.unwrap_or("ms-settings:");
                    let resolved_uri = if uri == "ms-settings:" {
                        self.config.custom_system_settings_path.clone()
                    } else {
                        uri.to_string()
                    };
                    ExecutionTarget::SettingsUri(resolved_uri)
                };

                results.push(CandidateItem {
                    title: spec.title.to_string(),
                    badge: spec.badge,
                    category: ShelfCategory::Settings,
                    can_elevate: false,
                    score,
                    target,
                });
            }
        }

        // Built-in Toggle: Run on Windows Startup
        let startup_title = if self.config.run_on_startup {
            "Run on Windows Startup: ON (Press Enter to Disable)"
        } else {
            "Run on Windows Startup: OFF (Press Enter to Enable)"
        };
        let startup_triggers = [
            "startup",
            "run on startup",
            "autostart",
            "launch on startup",
            "windows startup",
        ];
        if let Some(score) = self.score_triggers_and_title(
            normalized_query,
            raw_query,
            startup_title,
            &startup_triggers,
        ) {
            results.push(CandidateItem {
                title: startup_title.to_string(),
                badge: "STARTUP",
                category: ShelfCategory::Settings,
                can_elevate: false,
                score: score + 60,
                target: ExecutionTarget::ToggleStartup(!self.config.run_on_startup),
            });
        }

        // Built-in Global Hotkey Switcher Presets
        for &preset in HOTKEY_PRESETS {
            let is_active = self.config.hotkey.eq_ignore_ascii_case(preset);
            let hk_title = if is_active {
                format!("Global Hotkey: {} (Active)", preset)
            } else {
                format!("Set Global Hotkey: {}", preset)
            };
            let hk_triggers = [
                "hotkey",
                "shortcut",
                "global hotkey",
                "keybind",
                preset,
            ];
            if let Some(score) =
                self.score_triggers_and_title(normalized_query, raw_query, &hk_title, &hk_triggers)
            {
                results.push(CandidateItem {
                    title: hk_title,
                    badge: "HOTKEY",
                    category: ShelfCategory::Settings,
                    can_elevate: false,
                    score,
                    target: ExecutionTarget::SetHotkey(preset.to_string()),
                });
            }
        }

        for custom in &self.config.custom_settings {
            let trigger_slice = [custom.name.as_str()];
            if let Some(score) = self.score_triggers_and_title(
                normalized_query,
                raw_query,
                &custom.name,
                &trigger_slice,
            ) {
                results.push(CandidateItem {
                    title: custom.name.clone(),
                    badge: "SETTING",
                    category: ShelfCategory::Settings,
                    can_elevate: false,
                    score,
                    target: ExecutionTarget::SettingsUri(custom.path.clone()),
                });
            }
        }

        results
    }

    fn score_triggers_and_title(
        &self,
        normalized_query: &str,
        raw_query: &str,
        title: &str,
        triggers: &[&str],
    ) -> Option<i64> {
        let title_lower = title.to_ascii_lowercase();
        let mut best_score =
            self.compute_fuzzy_score(title, &title_lower, normalized_query, raw_query);

        for &trigger in triggers {
            let t_lower = trigger.to_ascii_lowercase();
            if let Some(s) =
                self.compute_fuzzy_score(trigger, &t_lower, normalized_query, raw_query)
            {
                best_score = Some(best_score.map_or(s, |curr| curr.max(s)));
            }
        }

        best_score.filter(|&s| normalized_query.len() >= 2 || s >= 120)
    }
}

fn expand_env_and_tilde(input: &str) -> String {
    let mut out = input.to_string();

    if out == "~" || out.starts_with("~/") || out.starts_with(r"~\") {
        if let Ok(home) = env::var("USERPROFILE") {
            out = format!("{}{}", home, &out[1..]);
        }
    }

    // Expand Windows %VAR% segments
    let mut result = String::with_capacity(out.len());
    let mut rest = out.as_str();
    while let Some(start) = rest.find('%') {
        result.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        if let Some(end) = after_start.find('%') {
            let var_name = &after_start[..end];
            if !var_name.is_empty() {
                if let Ok(val) = env::var(var_name) {
                    result.push_str(&val);
                } else {
                    result.push('%');
                    result.push_str(var_name);
                    result.push('%');
                }
            }
            rest = &after_start[end + 1..];
        } else {
            result.push('%');
            result.push_str(after_start);
            rest = "";
        }
    }
    result.push_str(rest);
    result
}

// ============================================================================
// Inline Calculator & Base Converter
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum TargetBase {
    Auto,
    Dec,
    Hex,
    Bin,
    Oct,
}

fn evaluate_calculator(raw_input: &str) -> Option<CandidateItem> {
    let mut expr = raw_input.trim();
    if expr.is_empty() {
        return None;
    }

    let explicit_eq = if let Some(stripped) = expr.strip_prefix('=') {
        expr = stripped.trim();
        true
    } else {
        false
    };

    if let Some(stripped) = expr.strip_suffix('=') {
        expr = stripped.trim();
    }

    if expr.is_empty() {
        return None;
    }

    let lower = expr.to_ascii_lowercase();
    let (expr_body, target_base) = if let Some(b) = lower
        .strip_suffix(" to hex")
        .or_else(|| lower.strip_suffix(" in hex"))
    {
        (&expr[..b.len()], TargetBase::Hex)
    } else if let Some(b) = lower
        .strip_suffix(" to bin")
        .or_else(|| lower.strip_suffix(" in bin"))
        .or_else(|| lower.strip_suffix(" to binary"))
        .or_else(|| lower.strip_suffix(" in binary"))
    {
        (&expr[..b.len()], TargetBase::Bin)
    } else if let Some(b) = lower
        .strip_suffix(" to oct")
        .or_else(|| lower.strip_suffix(" in oct"))
    {
        (&expr[..b.len()], TargetBase::Oct)
    } else if let Some(b) = lower
        .strip_suffix(" to dec")
        .or_else(|| lower.strip_suffix(" in dec"))
        .or_else(|| lower.strip_suffix(" to decimal"))
        .or_else(|| lower.strip_suffix(" in decimal"))
    {
        (&expr[..b.len()], TargetBase::Dec)
    } else {
        (expr, TargetBase::Auto)
    };

    let expr_body = expr_body.trim();
    if expr_body.is_empty() {
        return None;
    }

    let body_lower = expr_body.to_ascii_lowercase();
    let is_radix_literal = body_lower.starts_with("0x")
        || body_lower.starts_with("0b")
        || body_lower.starts_with("0o");

    let has_digit = expr_body.chars().any(|c| c.is_ascii_hexdigit());
    if !has_digit && body_lower != "pi" && body_lower != "e" {
        return None;
    }

    let has_operator_or_fn = expr_body
        .chars()
        .any(|c| matches!(c, '+' | '-' | '*' | '/' | '%' | '^' | '(' | ')' | '×' | '÷'))
        || body_lower.contains("sqrt")
        || body_lower.contains("sin")
        || body_lower.contains("cos")
        || body_lower.contains("tan")
        || body_lower.contains("log")
        || body_lower.contains("ln")
        || body_lower.contains("abs")
        || body_lower.contains("round")
        || body_lower.contains("floor")
        || body_lower.contains("ceil");

    // Avoid triggering on a plain number like "2026" unless prefixed with `=` or using base conversion / radix literal
    if !explicit_eq && target_base == TargetBase::Auto && !is_radix_literal && !has_operator_or_fn {
        return None;
    }

    let mut parser = MathParser::new(expr_body);
    let val = parser.parse_expr()?;
    parser.skip_whitespace();
    if parser.pos < parser.chars.len() || !val.is_finite() {
        return None;
    }

    // If user typed just a plain number without `=`, ignore (e.g. "-5" alone isn't useful unless `=-5`)
    if !explicit_eq
        && target_base == TargetBase::Auto
        && !is_radix_literal
        && expr_body.parse::<f64>().is_ok()
    {
        return None;
    }

    let (formatted, detail_suffix) = match target_base {
        TargetBase::Hex => {
            let n = val.round() as i64;
            (format!("0x{:X}", n), String::new())
        }
        TargetBase::Bin => {
            let n = val.round() as i64;
            (format!("0b{:b}", n), String::new())
        }
        TargetBase::Oct => {
            let n = val.round() as i64;
            (format!("0o{:o}", n), String::new())
        }
        TargetBase::Dec | TargetBase::Auto => {
            if (val - val.round()).abs() < 1e-9 && val.abs() < 9e15 {
                let n = val.round() as i64;
                let extra = if is_radix_literal && target_base == TargetBase::Auto {
                    format!("  •  Hex: 0x{:X}  •  Bin: 0b{:b}", n, n)
                } else {
                    String::new()
                };
                (n.to_string(), extra)
            } else {
                let s = format!("{:.8}", val);
                let clean = s.trim_end_matches('0').trim_end_matches('.').to_string();
                (clean, String::new())
            }
        }
    };

    let title = format!(
        "{} = {}{}   (Enter to Copy)",
        expr_body, formatted, detail_suffix
    );

    Some(CandidateItem {
        title,
        badge: "CALC",
        category: ShelfCategory::Cmds,
        can_elevate: false,
        score: 25_000,
        target: ExecutionTarget::CopyToClipboard(formatted),
    })
}

struct MathParser {
    chars: Vec<char>,
    pos: usize,
}

impl MathParser {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<char> {
        self.skip_whitespace();
        self.chars.get(self.pos).copied()
    }

    fn parse_expr(&mut self) -> Option<f64> {
        let mut left = self.parse_term()?;
        while let Some(op) = self.peek() {
            if op == '+' {
                self.pos += 1;
                left += self.parse_term()?;
            } else if op == '-' {
                self.pos += 1;
                left -= self.parse_term()?;
            } else {
                break;
            }
        }
        Some(left)
    }

    fn parse_term(&mut self) -> Option<f64> {
        let mut left = self.parse_power()?;
        while let Some(op) = self.peek() {
            if op == '*' && self.chars.get(self.pos + 1) == Some(&'*') {
                break; // Handled in parse_power as `**`
            }
            match op {
                '*' | '×' => {
                    self.pos += 1;
                    left *= self.parse_power()?;
                }
                '/' | '÷' => {
                    self.pos += 1;
                    let right = self.parse_power()?;
                    if right == 0.0 {
                        return None;
                    }
                    left /= right;
                }
                '%' => {
                    self.pos += 1;
                    let right = self.parse_power()?;
                    if right == 0.0 {
                        return None;
                    }
                    left %= right;
                }
                _ => break,
            }
        }
        Some(left)
    }

    fn parse_power(&mut self) -> Option<f64> {
        let base = self.parse_unary()?;
        self.skip_whitespace();
        if self.pos < self.chars.len() {
            if self.chars[self.pos] == '^' {
                self.pos += 1;
                let exp = self.parse_power()?;
                return Some(base.powf(exp));
            } else if self.chars[self.pos] == '*' && self.chars.get(self.pos + 1) == Some(&'*') {
                self.pos += 2;
                let exp = self.parse_power()?;
                return Some(base.powf(exp));
            }
        }
        Some(base)
    }

    fn parse_unary(&mut self) -> Option<f64> {
        match self.peek()? {
            '+' => {
                self.pos += 1;
                self.parse_unary()
            }
            '-' => {
                self.pos += 1;
                Some(-self.parse_unary()?)
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Option<f64> {
        let ch = self.peek()?;

        if ch == '(' {
            self.pos += 1;
            let val = self.parse_expr()?;
            if self.peek() != Some(')') {
                return None;
            }
            self.pos += 1;
            return Some(val);
        }

        if ch.is_ascii_digit() || ch == '.' {
            return self.parse_number();
        }

        if ch.is_ascii_alphabetic() {
            let start = self.pos;
            while self.pos < self.chars.len()
                && (self.chars[self.pos].is_ascii_alphanumeric() || self.chars[self.pos] == '_')
            {
                self.pos += 1;
            }
            let ident: String = self.chars[start..self.pos]
                .iter()
                .collect::<String>()
                .to_ascii_lowercase();

            match ident.as_str() {
                "pi" => return Some(std::f64::consts::PI),
                "e" => return Some(std::f64::consts::E),
                _ => {}
            }

            if self.peek() != Some('(') {
                return None;
            }
            self.pos += 1;
            let arg = self.parse_expr()?;
            if self.peek() != Some(')') {
                return None;
            }
            self.pos += 1;

            return match ident.as_str() {
                "sqrt" => Some(arg.sqrt()),
                "abs" => Some(arg.abs()),
                "round" => Some(arg.round()),
                "floor" => Some(arg.floor()),
                "ceil" => Some(arg.ceil()),
                "sin" => Some(arg.sin()),
                "cos" => Some(arg.cos()),
                "tan" => Some(arg.tan()),
                "ln" => Some(arg.ln()),
                "log" | "log10" => Some(arg.log10()),
                _ => None,
            };
        }

        None
    }

    fn parse_number(&mut self) -> Option<f64> {
        self.skip_whitespace();
        if self.pos + 2 <= self.chars.len() && self.chars[self.pos] == '0' {
            let prefix = self.chars[self.pos + 1].to_ascii_lowercase();
            if prefix == 'x' {
                self.pos += 2;
                let mut s = String::new();
                while self.pos < self.chars.len()
                    && (self.chars[self.pos].is_ascii_hexdigit() || self.chars[self.pos] == '_')
                {
                    if self.chars[self.pos] != '_' {
                        s.push(self.chars[self.pos]);
                    }
                    self.pos += 1;
                }
                return i64::from_str_radix(&s, 16).ok().map(|v| v as f64);
            } else if prefix == 'b' {
                self.pos += 2;
                let mut s = String::new();
                while self.pos < self.chars.len()
                    && (matches!(self.chars[self.pos], '0' | '1' | '_'))
                {
                    if self.chars[self.pos] != '_' {
                        s.push(self.chars[self.pos]);
                    }
                    self.pos += 1;
                }
                return i64::from_str_radix(&s, 2).ok().map(|v| v as f64);
            } else if prefix == 'o' {
                self.pos += 2;
                let mut s = String::new();
                while self.pos < self.chars.len()
                    && (matches!(self.chars[self.pos], '0'..='7' | '_'))
                {
                    if self.chars[self.pos] != '_' {
                        s.push(self.chars[self.pos]);
                    }
                    self.pos += 1;
                }
                return i64::from_str_radix(&s, 8).ok().map(|v| v as f64);
            }
        }

        let mut num_str = String::new();
        let mut seen_dot = false;
        while self.pos < self.chars.len() {
            let c = self.chars[self.pos];
            if c.is_ascii_digit() {
                num_str.push(c);
                self.pos += 1;
            } else if c == '_' || c == ',' {
                self.pos += 1;
            } else if c == '.' && !seen_dot {
                seen_dot = true;
                num_str.push('.');
                self.pos += 1;
            } else {
                break;
            }
        }

        num_str.parse::<f64>().ok()
    }
}
