use crate::models::{AppEntry, ScannedFileEntry};
use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn build_start_menu_index() -> Vec<AppEntry> {
    let mut roots = Vec::with_capacity(2);

    let program_data = env::var("PROGRAMDATA").unwrap_or_else(|_| r"C:\ProgramData".to_string());
    let global_start_menu = PathBuf::from(program_data)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs");
    roots.push(global_start_menu);

    if let Ok(appdata) = env::var("APPDATA") {
        let user_start_menu = PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        roots.push(user_start_menu);
    } else if let Ok(user_profile) = env::var("USERPROFILE") {
        let user_start_menu = PathBuf::from(user_profile)
            .join("AppData")
            .join("Roaming")
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs");
        roots.push(user_start_menu);
    }

    let mut seen_names = HashSet::with_capacity(256);
    let mut entries = Vec::with_capacity(256);

    for root in roots {
        if !root.exists() {
            continue;
        }

        for dir_entry in WalkDir::new(&root)
            .follow_links(false)
            .max_depth(6)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = dir_entry.path();
            if !path.is_file() || !is_launchable_extension(path) {
                continue;
            }

            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };

            let clean_name = stem.trim();
            if clean_name.is_empty() || is_uninstaller_noise(clean_name) {
                continue;
            }

            let name_lower = clean_name.to_ascii_lowercase();
            if seen_names.insert(name_lower.clone()) {
                entries.push(AppEntry {
                    name: clean_name.to_string(),
                    name_lower,
                    path: path.to_path_buf(),
                });
            }
        }
    }

    entries.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
    entries.shrink_to_fit();
    entries
}

/// Scans ONLY the user-configured custom paths for files to open directly (no full disk scan).
pub fn build_custom_files_index(custom_paths: &[String]) -> Vec<ScannedFileEntry> {
    let mut seen_paths = HashSet::new();
    let mut files = Vec::new();

    for raw_path in custom_paths {
        let trimmed = raw_path.trim();
        if trimmed.is_empty() {
            continue;
        }

        let root = PathBuf::from(trimmed);
        if !root.exists() {
            continue;
        }

        if root.is_file() {
            if let Some(name) = root.file_name().and_then(|n| n.to_str()) {
                let key = root.to_string_lossy().to_ascii_lowercase();
                if seen_paths.insert(key) {
                    files.push(ScannedFileEntry {
                        display_name: name.to_string(),
                        name_lower: name.to_ascii_lowercase(),
                        path: root,
                    });
                }
            }
            continue;
        }

        let walker = WalkDir::new(&root)
            .follow_links(false)
            .max_depth(5)
            .into_iter()
            .filter_entry(|e| !is_ignored_directory(e.path()));

        for dir_entry in walker.filter_map(Result::ok) {
            let path = dir_entry.path();
            if !path.is_file() {
                continue;
            }

            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };

            let clean_name = file_name.trim();
            if clean_name.is_empty() || clean_name.starts_with('.') {
                continue;
            }

            let key = path.to_string_lossy().to_ascii_lowercase();
            if seen_paths.insert(key) {
                files.push(ScannedFileEntry {
                    display_name: clean_name.to_string(),
                    name_lower: clean_name.to_ascii_lowercase(),
                    path: path.to_path_buf(),
                });
            }
        }
    }

    files.sort_by(|a, b| a.name_lower.cmp(&b.name_lower));
    files.shrink_to_fit();
    files
}

fn is_ignored_directory(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".git" | "node_modules" | "target" | "__pycache__" | ".vs" | ".idea" | "$recycle.bin"
    )
}

fn is_launchable_extension(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    ext.eq_ignore_ascii_case("lnk") || ext.eq_ignore_ascii_case("exe")
}

fn is_uninstaller_noise(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("uninstall ") || lower == "uninstall"
}
