use crate::config::AppConfig;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const NOTE_BANG_PREFIX: &str = "note:";
pub const DEFAULT_NOTE_FILE: &str = "notes.md";
const MARKDOWN_FOLDER_NAME: &str = "markdown";
const VISUAL_DAY_SEPARATOR: &str =
    "════════════════════════════════════════════════════════════════";

#[derive(Debug, Clone)]
pub struct ParsedMarkdownBlock {
    pub kind: &'static str,
    pub text: String,
    pub subtext: String,
}

/// Resolves the `markdown/` directory next to the executable or working directory
/// and ensures that both `markdown/` and `markdown/notes.md` exist.
pub fn ensure_markdown_dir() -> PathBuf {
    let dir = AppConfig::resolve_config_path(MARKDOWN_FOLDER_NAME);
    let _ = fs::create_dir_all(&dir);
    let default_md = dir.join(DEFAULT_NOTE_FILE);
    if !default_md.exists() {
        let initial = "# 📓 Notes — notes.md\n";
        let _ = fs::write(&default_md, initial);
    }
    dir
}

/// Sanitizes a user-provided note file name into a safe `<stem>.md` filename.
pub fn sanitize_md_filename(raw: &str) -> String {
    let trimmed = raw.trim().trim_start_matches('!');
    let without_ext = trimmed
        .strip_suffix(".md")
        .or_else(|| trimmed.strip_suffix(".MD"))
        .unwrap_or(trimmed);

    let clean: String = without_ext
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();

    let clean = clean.trim_matches('_');
    if clean.is_empty() {
        DEFAULT_NOTE_FILE.to_string()
    } else {
        format!("{}.md", clean)
    }
}

pub fn make_note_bang_key(filename: &str) -> String {
    format!("{}{}", NOTE_BANG_PREFIX, sanitize_md_filename(filename))
}

pub fn is_note_bang(active_bang: &str) -> bool {
    active_bang.starts_with(NOTE_BANG_PREFIX)
}

pub fn note_file_from_bang(active_bang: &str) -> Option<String> {
    active_bang
        .strip_prefix(NOTE_BANG_PREFIX)
        .map(sanitize_md_filename)
}

pub fn format_note_bang_label(filename: &str) -> String {
    let clean = sanitize_md_filename(filename);
    format!("NOTE • {}", clean)
}

/// Lists all `.md` files inside the `markdown/` folder (`notes.md` always first).
pub fn list_markdown_files(active_file: &str) -> Vec<String> {
    let dir = ensure_markdown_dir();
    let active_clean = sanitize_md_filename(active_file);

    let mut files = Vec::new();
    if let Ok(read_dir) = fs::read_dir(&dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("md") {
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            let clean = sanitize_md_filename(name);
                            if !files.contains(&clean) {
                                files.push(clean);
                            }
                        }
                    }
                }
            }
        }
    }

    if !files.contains(&DEFAULT_NOTE_FILE.to_string()) {
        files.push(DEFAULT_NOTE_FILE.to_string());
    }
    if !files.contains(&active_clean) {
        files.push(active_clean);
    }

    files.sort_by(|a, b| {
        if a == DEFAULT_NOTE_FILE {
            std::cmp::Ordering::Less
        } else if b == DEFAULT_NOTE_FILE {
            std::cmp::Ordering::Greater
        } else {
            a.cmp(b)
        }
    });
    files
}

/// Detects when the user types a Note bang in the search bar followed by Space:
/// - `note ` or `notes ` -> `note:notes.md`
/// - `note:work ` / `note/work ` / `md:work ` -> `note:work.md`
/// - `work.md ` -> `note:work.md`
/// - `<existing_md_stem> ` (if `markdown/<stem>.md` exists and isn't a web bang) -> `note:<stem>.md`
pub fn extract_note_bang_prefix(
    raw_input: &str,
    configured_web_bangs: &HashMap<String, String>,
) -> Option<(String, String, String)> {
    let (first_token, remainder) = raw_input.split_once(char::is_whitespace)?;
    let token = first_token.trim().to_ascii_lowercase();
    let rem = remainder.trim_start().to_string();

    if token == "note" || token == "notes" || token == "!note" || token == "!notes" {
        let file = DEFAULT_NOTE_FILE.to_string();
        return Some((make_note_bang_key(&file), file, rem));
    }

    for prefix in ["note:", "notes:", "note/", "md:", "!note:", "!md:"] {
        if let Some(target) = token.strip_prefix(prefix) {
            if !target.trim().is_empty() {
                let file = sanitize_md_filename(target);
                return Some((make_note_bang_key(&file), file, rem));
            }
        }
    }

    if let Some(stem) = token.strip_suffix(".md") {
        let clean_stem = stem.trim_start_matches('!');
        if !clean_stem.is_empty()
            && clean_stem
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            let file = sanitize_md_filename(clean_stem);
            return Some((make_note_bang_key(&file), file, rem));
        }
    }

    // Also allow typing `!<existing_or_new_md>` + Space or `<existing_md_stem>` + Space
    if !configured_web_bangs.contains_key(&token) {
        let dir = ensure_markdown_dir();
        let candidate_file = sanitize_md_filename(&token);
        let candidate_path = dir.join(&candidate_file);
        if candidate_path.exists() {
            return Some((make_note_bang_key(&candidate_file), candidate_file, rem));
        }
    }

    None
}

/// While ALREADY inside Note Mode, allows switching `.md` files via bang:
/// - `!work ` -> switches to `work.md`
/// - `note:work ` or `md:work ` -> switches to `work.md`
/// - `work.md ` -> switches to `work.md`
pub fn extract_in_note_file_switch(raw_input: &str) -> Option<(String, String)> {
    let (first_token, remainder) = raw_input.split_once(char::is_whitespace)?;
    let token = first_token.trim().to_ascii_lowercase();
    let rem = remainder.trim_start().to_string();

    if let Some(target) = token
        .strip_prefix('!')
        .or_else(|| token.strip_prefix("note:"))
        .or_else(|| token.strip_prefix("notes:"))
        .or_else(|| token.strip_prefix("note/"))
        .or_else(|| token.strip_prefix("md:"))
    {
        let clean = target.trim();
        if !clean.is_empty()
            && clean
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Some((sanitize_md_filename(clean), rem));
        }
    }

    if let Some(stem) = token.strip_suffix(".md") {
        if !stem.is_empty()
            && stem
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Some((sanitize_md_filename(stem), rem));
        }
    }

    None
}

/// Appends a note line to `markdown/<filename>.md`:
/// - Wraps the note inside `'''     '''` (triple single-quotes) by default
/// - Writes the current local date and time
/// - Groups notes by date for a full 1 day (`## 📅 YYYY-MM-DD (Weekday)`)
/// - Inserts a visual separator (`═══...` + `---`) between different days
pub fn append_note_entry(filename: &str, raw_note: &str) -> std::io::Result<PathBuf> {
    let trimmed = raw_note.trim();
    if trimmed.is_empty() {
        return Ok(ensure_markdown_dir().join(sanitize_md_filename(filename)));
    }

    let dir = ensure_markdown_dir();
    let safe_name = sanitize_md_filename(filename);
    let file_path = dir.join(&safe_name);

    let (date_iso, weekday_name, time_12h) = current_local_datetime_parts();
    let day_marker_prefix = format!("## 📅 {}", date_iso);
    let day_header_full = format!("## 📅 {} ({})", date_iso, weekday_name);

    let mut content = if file_path.exists() {
        fs::read_to_string(&file_path).unwrap_or_default()
    } else {
        String::new()
    };

    if content.trim().is_empty() {
        content = format!("# 📓 Notes — {}\n", safe_name);
    }

    let has_any_previous_day = content.lines().any(|l| l.trim_start().starts_with("## 📅 "));
    let has_today_section = content
        .lines()
        .any(|l| l.trim_start().starts_with(&day_marker_prefix));

    if !content.ends_with('\n') {
        content.push('\n');
    }

    if !has_today_section {
        if has_any_previous_day {
            content.push('\n');
            content.push_str(VISUAL_DAY_SEPARATOR);
            content.push_str("\n---\n\n");
        } else {
            content.push('\n');
        }
        content.push_str(&day_header_full);
        content.push_str("\n\n");
    } else {
        content.push('\n');
    }

    // Wrap text inside ''' ... ''' by default
    let inner_text = trimmed
        .strip_prefix("'''")
        .and_then(|s| s.strip_suffix("'''"))
        .map(str::trim)
        .unwrap_or(trimmed);

    let entry_block = format!(
        "- 🕒 **{} • {}**\n  ''' {} '''\n",
        date_iso, time_12h, inner_text
    );

    content.push_str(&entry_block);
    fs::write(&file_path, content)?;
    Ok(file_path)
}

/// Reads and parses `markdown/<filename>.md` into formatted UI blocks for the Palette preview.
pub fn load_markdown_preview_blocks(filename: &str) -> Vec<ParsedMarkdownBlock> {
    let dir = ensure_markdown_dir();
    let safe_name = sanitize_md_filename(filename);
    let file_path = dir.join(&safe_name);

    if !file_path.exists() {
        let initial = format!("# 📓 Notes — {}\n", safe_name);
        let _ = fs::write(&file_path, &initial);
    }

    let raw = fs::read_to_string(&file_path).unwrap_or_default();
    parse_markdown_to_blocks(&raw, &safe_name)
}

fn parse_markdown_to_blocks(raw: &str, filename: &str) -> Vec<ParsedMarkdownBlock> {
    let mut blocks = Vec::new();
    let mut in_code_block = false;
    let mut code_buffer = String::new();

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            if in_code_block {
                blocks.push(ParsedMarkdownBlock {
                    kind: "code",
                    text: code_buffer.trim_end().to_string(),
                    subtext: String::new(),
                });
                code_buffer.clear();
                in_code_block = false;
            } else {
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            if !code_buffer.is_empty() {
                code_buffer.push('\n');
            }
            code_buffer.push_str(line);
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        // Visual Day Separator
        if trimmed.contains("════")
            || trimmed == "---"
            || trimmed == "***"
            || trimmed == "___"
        {
            // Avoid duplicate consecutive separators when both ════ and --- are present
            if blocks
                .last()
                .map(|b: &ParsedMarkdownBlock| b.kind == "separator")
                .unwrap_or(false)
            {
                continue;
            }
            blocks.push(ParsedMarkdownBlock {
                kind: "separator",
                text: "─── ✦ DIFFERENT DAY ✦ ───".to_string(),
                subtext: String::new(),
            });
            continue;
        }

        // H1 Heading
        if let Some(h1) = trimmed.strip_prefix("# ") {
            blocks.push(ParsedMarkdownBlock {
                kind: "h1",
                text: strip_inline_md(h1),
                subtext: format!("markdown/{}", filename),
            });
            continue;
        }

        // H2 Date Section or Subheading
        if let Some(h2) = trimmed.strip_prefix("## ") {
            blocks.push(ParsedMarkdownBlock {
                kind: "date",
                text: strip_inline_md(h2),
                subtext: "DAILY LOG".to_string(),
            });
            continue;
        }

        // H3 Heading
        if let Some(h3) = trimmed.strip_prefix("### ") {
            blocks.push(ParsedMarkdownBlock {
                kind: "h3",
                text: strip_inline_md(h3),
                subtext: String::new(),
            });
            continue;
        }

        // Timestamp entry line (- 🕒 **YYYY-MM-DD • HH:MM:SS AM**)
        if trimmed.starts_with("- 🕒") || trimmed.starts_with("* 🕒") {
            let clean_ts = strip_inline_md(trimmed.trim_start_matches(['-', '*', ' ']));
            blocks.push(ParsedMarkdownBlock {
                kind: "timestamp",
                text: clean_ts,
                subtext: String::new(),
            });
            continue;
        }

        // Triple-quoted note line (''' ... ''')
        if trimmed.starts_with("'''") && trimmed.ends_with("'''") && trimmed.len() >= 6 {
            let inner = trimmed[3..trimmed.len() - 3].trim();
            blocks.push(ParsedMarkdownBlock {
                kind: "note",
                text: format!("''' {} '''", inner),
                subtext: inner.to_string(),
            });
            continue;
        }

        // Blockquote
        if let Some(q) = trimmed.strip_prefix("> ") {
            blocks.push(ParsedMarkdownBlock {
                kind: "quote",
                text: strip_inline_md(q),
                subtext: String::new(),
            });
            continue;
        }

        // Bullet list item
        if let Some(bullet) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            blocks.push(ParsedMarkdownBlock {
                kind: "bullet",
                text: strip_inline_md(bullet),
                subtext: String::new(),
            });
            continue;
        }

        // Default formatted paragraph
        blocks.push(ParsedMarkdownBlock {
            kind: "paragraph",
            text: strip_inline_md(trimmed),
            subtext: String::new(),
        });
    }

    if in_code_block && !code_buffer.is_empty() {
        blocks.push(ParsedMarkdownBlock {
            kind: "code",
            text: code_buffer,
            subtext: String::new(),
        });
    }

    if blocks.len() <= 1 {
        blocks.push(ParsedMarkdownBlock {
            kind: "empty",
            text: format!(
                "No notes recorded in {} yet. Type a note above and press Enter!",
                filename
            ),
            subtext: "Saved notes are automatically wrapped in ''' ... ''' with date & time."
                .to_string(),
        });
    }

    blocks
}

fn strip_inline_md(input: &str) -> String {
    input.replace("**", "").replace("__", "")
}

/// Returns `(YYYY-MM-DD, Weekday, hh:mm:ss AM/PM)` in local system time without extra crates.
fn current_local_datetime_parts() -> (String, &'static str, String) {
    #[cfg(target_os = "windows")]
    {
        #[repr(C)]
        #[derive(Default)]
        struct SystemTimeWin32 {
            year: u16,
            month: u16,
            day_of_week: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            milliseconds: u16,
        }

        extern "system" {
            fn GetLocalTime(lp_system_time: *mut SystemTimeWin32);
        }

        let mut st = SystemTimeWin32::default();
        unsafe {
            GetLocalTime(&mut st);
        }

        let weekday = match st.day_of_week {
            0 => "Sunday",
            1 => "Monday",
            2 => "Tuesday",
            3 => "Wednesday",
            4 => "Thursday",
            5 => "Friday",
            6 => "Saturday",
            _ => "Day",
        };

        let period = if st.hour < 12 { "AM" } else { "PM" };
        let hour12 = match st.hour % 12 {
            0 => 12,
            h => h,
        };

        let date_iso = format!("{:04}-{:02}-{:02}", st.year, st.month, st.day);
        let time_12h = format!("{:02}:{:02}:{:02} {}", hour12, st.minute, st.second, period);
        (date_iso, weekday, time_12h)
    }

    #[cfg(not(target_os = "windows"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let days = (secs / 86_400) as i64;
        let rem_secs = secs % 86_400;
        let hour = (rem_secs / 3600) as u16;
        let minute = ((rem_secs % 3600) / 60) as u16;
        let second = (rem_secs % 60) as u16;

        // Civil date from days since 1970-01-01
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = (z - era * 146_097) as u32;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let y = yoe as i64 + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = if m <= 2 { y + 1 } else { y };

        let weekday_idx = (days + 4).rem_euclid(7);
        let weekday = match weekday_idx {
            0 => "Sunday",
            1 => "Monday",
            2 => "Tuesday",
            3 => "Wednesday",
            4 => "Thursday",
            5 => "Friday",
            _ => "Saturday",
        };

        let period = if hour < 12 { "AM" } else { "PM" };
        let hour12 = match hour % 12 {
            0 => 12,
            h => h,
        };

        let date_iso = format!("{:04}-{:02}-{:02}", year, m, d);
        let time_12h = format!("{:02}:{:02}:{:02} {}", hour12, minute, second, period);
        (date_iso, weekday, time_12h)
    }
}
