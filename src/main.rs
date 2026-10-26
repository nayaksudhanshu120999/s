#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod engine;
mod executor;
mod icon;
mod indexer;
mod models;
mod notes;
mod system;

use config::{AppConfig, CustomCommandItem, CustomSettingItem, CustomThemeConfig};
use engine::QueryEngine;
use executor::execute_candidate;
use icon::create_slint_app_icon;
use indexer::{build_custom_files_index, build_start_menu_index};
use models::{CandidateItem, ExecutionTarget};
use notes::{
    append_note_entry, ensure_markdown_dir, extract_in_note_file_switch, extract_note_bang_prefix,
    format_note_bang_label, is_note_bang, list_markdown_files, load_markdown_preview_blocks,
    make_note_bang_key, note_file_from_bang, sanitize_md_filename, DEFAULT_NOTE_FILE,
};
use slint::{
    Color, ComponentHandle, LogicalPosition, LogicalSize, ModelRc, SharedString, Timer, TimerMode,
    VecModel,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};
use system::{
    activate_window_by_title, check_outside_click_or_focus_loss, enable_window_maximize,
    ensure_single_instance_or_activate_existing, init_system_integration, primary_screen_size,
    shutdown_system_integration, SystemEvent,
};

slint::include_modules!();

const PALETTE_TITLE: &str = "Command Palette";
const SETTINGS_TITLE: &str = "Command Palette Settings";
const CONFIG_FILE_PATH: &str = "config.toml";
const WINDOW_WIDTH: f32 = 680.0;
const COMPACT_HEIGHT: f32 = 68.0;
const MAX_EXPANDED_HEIGHT: f32 = 460.0;

struct PaletteState {
    engine: QueryEngine,
    current_candidates: Vec<CandidateItem>,
    palette_visible: bool,
    last_shown_at: Instant,
}

fn main() -> Result<(), slint::PlatformError> {
    // 1. Single-Instance Lock: prevents multiple tray icons and activates existing instance
    if !ensure_single_instance_or_activate_existing() {
        return Ok(());
    }

    // Ensure `markdown/` directory and default `markdown/notes.md` exist
    let _ = ensure_markdown_dir();

    let config = AppConfig::load_or_create(CONFIG_FILE_PATH);
    let indexed_apps = build_start_menu_index();
    let indexed_files = build_custom_files_index(&config.custom_file_scan_paths);
    let engine = QueryEngine::new(config, indexed_apps, indexed_files);

    let app_icon = create_slint_app_icon();
    let initial_theme = build_ui_theme_colors(&engine.config);

    let palette_ui = PaletteWindow::new()?;
    palette_ui.set_app_icon(app_icon.clone());
    palette_ui.set_theme(initial_theme.clone());
    palette_ui.set_dynamic_height(COMPACT_HEIGHT);
    palette_ui
        .window()
        .set_size(LogicalSize::new(WINDOW_WIDTH, COMPACT_HEIGHT));

    let settings_ui = SettingsWindow::new()?;
    settings_ui.set_app_icon(app_icon);
    settings_ui.set_theme(initial_theme);

    let state = Rc::new(RefCell::new(PaletteState {
        engine,
        current_candidates: Vec::new(),
        palette_visible: true,
        last_shown_at: Instant::now(),
    }));

    sync_settings_ui(&settings_ui, &state.borrow().engine);

    // Query input edited callback (Handles Web Bangs AND Note Bangs + .md file switching)
    {
        let ui_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_query_changed(move |query| {
            let Some(window) = ui_weak.upgrade() else {
                return;
            };

            let current_bang = window.get_active_bang().to_string();
            let raw_q = query.to_string();

            // Clear temporary save status text when typing
            if !window.get_note_status_text().is_empty() {
                window.set_note_status_text(SharedString::from(""));
            }

            if current_bang.is_empty() {
                // 1. Check if user typed a Note bang (e.g. "note ", "note:work ", "work.md ", or "<existing_md> ")
                let maybe_note_bang = {
                    let st = state_ref.borrow();
                    extract_note_bang_prefix(&raw_q, &st.engine.config.bangs)
                };

                if let Some((note_bang_key, note_file, remainder)) = maybe_note_bang {
                    activate_note_bang_on_window(&window, &state_ref, &note_bang_key, &note_file, &remainder);
                    return;
                }

                // 2. Check standard configured Web Bangs (e.g. "yt ", "g ")
                let maybe_bang = {
                    let st = state_ref.borrow();
                    st.engine.extract_bang_prefix(&raw_q)
                };

                if let Some((bang_key, remainder)) = maybe_bang {
                    window.set_is_note_mode(false);
                    window.set_active_bang(SharedString::from(bang_key.as_str()));
                    window.set_active_bang_label(SharedString::from(
                        bang_key.to_ascii_uppercase().as_str(),
                    ));
                    window.set_query_text(SharedString::from(remainder.as_str()));
                    refresh_results(&window, &state_ref, &bang_key, &remainder);
                    return;
                }
            } else if is_note_bang(&current_bang) {
                // While already inside Note mode, check if user typed `!otherfile ` or `note:otherfile ` to switch .md file
                if let Some((new_file, remainder)) = extract_in_note_file_switch(&raw_q) {
                    let new_bang_key = make_note_bang_key(&new_file);
                    activate_note_bang_on_window(&window, &state_ref, &new_bang_key, &new_file, &remainder);
                    window.set_note_status_text(SharedString::from(
                        format!("Switched preview to markdown/{}", new_file).as_str(),
                    ));
                    return;
                }
            }

            refresh_results(&window, &state_ref, &current_bang, &raw_q);
        });
    }

    // Submit note on Enter when in Note Mode
    {
        let ui_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_submit_note(move |text| {
            let Some(window) = ui_weak.upgrade() else {
                return;
            };

            let raw_note = text.trim().to_string();
            let active_file = window.get_active_note_file().to_string();
            let target_file = if active_file.is_empty() {
                DEFAULT_NOTE_FILE.to_string()
            } else {
                sanitize_md_filename(&active_file)
            };

            if raw_note.is_empty() {
                return;
            }

            if append_note_entry(&target_file, &raw_note).is_ok() {
                window.set_query_text(SharedString::from(""));
                window.set_note_status_text(SharedString::from(
                    format!("✓ Saved in '''...''' to markdown/{}", target_file).as_str(),
                ));
                let bang_key = make_note_bang_key(&target_file);
                refresh_results(&window, &state_ref, &bang_key, "");
            }
        });
    }

    // Switch active .md note file by clicking a file pill in the preview header
    {
        let ui_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_switch_note_file(move |md_file| {
            let Some(window) = ui_weak.upgrade() else {
                return;
            };
            let clean_file = sanitize_md_filename(md_file.as_str());
            let bang_key = make_note_bang_key(&clean_file);
            let current_q = window.get_query_text().to_string();
            activate_note_bang_on_window(&window, &state_ref, &bang_key, &clean_file, &current_q);
        });
    }

    // Cycle through .md files in markdown/ via Tab / Shift+Tab while in Note Mode
    {
        let ui_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_cycle_note_file(move |reverse| {
            let Some(window) = ui_weak.upgrade() else {
                return;
            };
            let current_file = sanitize_md_filename(window.get_active_note_file().as_str());
            let files = list_markdown_files(&current_file);
            if files.is_empty() {
                return;
            }
            let cur_idx = files.iter().position(|f| f == &current_file).unwrap_or(0);
            let next_idx = if reverse {
                (cur_idx + files.len() - 1) % files.len()
            } else {
                (cur_idx + 1) % files.len()
            };
            let next_file = &files[next_idx];
            let bang_key = make_note_bang_key(next_file);
            let current_q = window.get_query_text().to_string();
            activate_note_bang_on_window(&window, &state_ref, &bang_key, next_file, &current_q);
        });
    }

    // Clear active Bang Pill Chip (Backspace on empty query or clicking ✕ on chip)
    {
        let ui_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_clear_active_bang(move || {
            if let Some(window) = ui_weak.upgrade() {
                window.set_is_note_mode(false);
                window.set_note_status_text(SharedString::from(""));
                window.set_active_bang(SharedString::from(""));
                window.set_active_bang_label(SharedString::from(""));
                let q = window.get_query_text().to_string();
                refresh_results(&window, &state_ref, "", &q);
            }
        });
    }

    // Activate item (Enter or Left-Click)
    {
        let ui_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_activate_item(move |index| {
            let (candidate, cmd_path) = {
                let st = state_ref.borrow();
                (
                    st.current_candidates.get(index as usize).cloned(),
                    st.engine.config.custom_cmd_path.clone(),
                )
            };

            if let Some(item) = candidate {
                if matches!(item.target, ExecutionTarget::OpenPaletteSettings) {
                    if let Some(window) = ui_weak.upgrade() {
                        hide_palette(&window, &state_ref);
                    }
                    if let Some(settings_win) = settings_weak.upgrade() {
                        open_settings_window(&settings_win, &state_ref.borrow().engine);
                    }
                } else if execute_candidate(&item, false, &cmd_path).is_ok() {
                    if let Some(window) = ui_weak.upgrade() {
                        hide_palette(&window, &state_ref);
                    }
                }
            }
        });
    }

    // Run as Administrator (Right-click menu or Ctrl+Enter)
    {
        let ui_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_run_item_admin(move |index| {
            let (candidate, cmd_path) = {
                let st = state_ref.borrow();
                (
                    st.current_candidates.get(index as usize).cloned(),
                    st.engine.config.custom_cmd_path.clone(),
                )
            };

            if let Some(item) = candidate {
                if execute_candidate(&item, true, &cmd_path).is_ok() {
                    if let Some(window) = ui_weak.upgrade() {
                        hide_palette(&window, &state_ref);
                    }
                }
            }
        });
    }

    // Open Settings GUI from Palette gear button or Ctrl+,
    {
        let ui_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_open_settings(move || {
            if let Some(window) = ui_weak.upgrade() {
                hide_palette(&window, &state_ref);
            }
            if let Some(settings_win) = settings_weak.upgrade() {
                open_settings_window(&settings_win, &state_ref.borrow().engine);
            }
        });
    }

    // Escape key handling
    {
        let ui_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(&state);
        palette_ui.on_escape_pressed(move || {
            if let Some(window) = ui_weak.upgrade() {
                if !window.get_query_text().is_empty() || !window.get_active_bang().is_empty() {
                    window.set_is_note_mode(false);
                    window.set_note_status_text(SharedString::from(""));
                    window.set_active_bang(SharedString::from(""));
                    window.set_active_bang_label(SharedString::from(""));
                    window.set_query_text(SharedString::from(""));
                    refresh_results(&window, &state_ref, "", "");
                } else {
                    hide_palette(&window, &state_ref);
                }
            }
        });
    }

    wire_settings_callbacks(&settings_ui, &palette_ui, &state);

    let sys_rx = init_system_integration();

    let monitor_timer = Timer::default();
    {
        let ui_weak = palette_ui.as_weak();
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(&state);

        monitor_timer.start(
            TimerMode::Repeated,
            Duration::from_millis(35),
            move || {
                while let Ok(ev) = sys_rx.try_recv() {
                    match ev {
                        SystemEvent::ShowPalette => {
                            if let Some(window) = ui_weak.upgrade() {
                                show_and_focus_palette(&window, &state_ref);
                            }
                        }
                        SystemEvent::TogglePalette => {
                            if let Some(window) = ui_weak.upgrade() {
                                let is_vis = state_ref.borrow().palette_visible;
                                if is_vis {
                                    hide_palette(&window, &state_ref);
                                } else {
                                    show_and_focus_palette(&window, &state_ref);
                                }
                            }
                        }
                        SystemEvent::OpenSettings => {
                            if let Some(window) = ui_weak.upgrade() {
                                hide_palette(&window, &state_ref);
                            }
                            if let Some(settings_win) = settings_weak.upgrade() {
                                open_settings_window(&settings_win, &state_ref.borrow().engine);
                            }
                        }
                        SystemEvent::ReloadConfig => {
                            let new_cfg = AppConfig::load_or_create(CONFIG_FILE_PATH);
                            let new_apps = build_start_menu_index();
                            let new_files = build_custom_files_index(&new_cfg.custom_file_scan_paths);
                            let ui_theme = build_ui_theme_colors(&new_cfg);
                            {
                                let mut st = state_ref.borrow_mut();
                                st.engine = QueryEngine::new(new_cfg, new_apps, new_files);
                            }
                            if let Some(pal) = ui_weak.upgrade() {
                                pal.set_theme(ui_theme.clone());
                            }
                            if let Some(settings_win) = settings_weak.upgrade() {
                                settings_win.set_theme(ui_theme);
                                sync_settings_ui(&settings_win, &state_ref.borrow().engine);
                                settings_win.set_status_message(SharedString::from(
                                    "All indexes & config reloaded!",
                                ));
                            }
                        }
                        SystemEvent::QuitApp => {
                            shutdown_system_integration();
                            let _ = slint::quit_event_loop();
                        }
                    }
                }

                let (is_vis, elapsed_ms) = {
                    let st = state_ref.borrow();
                    (st.palette_visible, st.last_shown_at.elapsed().as_millis())
                };

                if is_vis && elapsed_ms > 180 {
                    let check_focus = elapsed_ms > 350;
                    if check_outside_click_or_focus_loss(PALETTE_TITLE, check_focus) {
                        if let Some(window) = ui_weak.upgrade() {
                            hide_palette(&window, &state_ref);
                        }
                    }
                }
            },
        );
    }

    show_and_focus_palette(&palette_ui, &state);
    let res = slint::run_event_loop_until_quit();
    shutdown_system_integration();
    res
}

fn activate_note_bang_on_window(
    window: &PaletteWindow,
    state_ref: &Rc<RefCell<PaletteState>>,
    bang_key: &str,
    note_file: &str,
    remainder: &str,
) {
    let clean_file = sanitize_md_filename(note_file);
    let label = format_note_bang_label(&clean_file);
    window.set_is_note_mode(true);
    window.set_active_note_file(SharedString::from(clean_file.as_str()));
    window.set_active_bang(SharedString::from(bang_key));
    window.set_active_bang_label(SharedString::from(label.as_str()));
    window.set_query_text(SharedString::from(remainder));
    window.set_scroll_y(0.0);
    refresh_results(window, state_ref, bang_key, remainder);
}

fn open_settings_window(settings_win: &SettingsWindow, engine: &QueryEngine) {
    sync_settings_ui(settings_win, engine);
    let _ = settings_win.show();
    enable_window_maximize(SETTINGS_TITLE);
    activate_window_by_title(SETTINGS_TITLE);
}

fn show_and_focus_palette(window: &PaletteWindow, state_ref: &Rc<RefCell<PaletteState>>) {
    let (screen_w, screen_h) = primary_screen_size();
    let x = ((screen_w - WINDOW_WIDTH) * 0.5).max(0.0);
    let y = (screen_h * 0.20).max(60.0);

    window.window().set_position(LogicalPosition::new(x, y));
    window.set_context_menu_visible(false);

    {
        let mut st = state_ref.borrow_mut();
        st.palette_visible = true;
        st.last_shown_at = Instant::now();
    }

    let _ = window.show();
    activate_window_by_title(PALETTE_TITLE);
    window.invoke_focus_input();
}

fn hide_palette(window: &PaletteWindow, state_ref: &Rc<RefCell<PaletteState>>) {
    window.set_context_menu_visible(false);
    window.set_is_note_mode(false);
    window.set_note_status_text(SharedString::from(""));
    window.set_active_bang(SharedString::from(""));
    window.set_active_bang_label(SharedString::from(""));
    window.set_query_text(SharedString::from(""));
    refresh_results(window, state_ref, "", "");
    let mut st = state_ref.borrow_mut();
    st.palette_visible = false;
    st.current_candidates.clear();
    let _ = window.hide();
}

fn refresh_results(
    window: &PaletteWindow,
    state_ref: &Rc<RefCell<PaletteState>>,
    active_bang: &str,
    query: &str,
) {
    // If a Note bang is active, render the formatted Markdown Preview in place of suggestions!
    if let Some(note_file) = note_file_from_bang(active_bang) {
        window.set_is_note_mode(true);
        window.set_active_note_file(SharedString::from(note_file.as_str()));

        let md_files: Vec<SharedString> = list_markdown_files(&note_file)
            .into_iter()
            .map(|f| SharedString::from(f.as_str()))
            .collect();
        window.set_note_files(ModelRc::from(Rc::new(VecModel::from(md_files))));

        let parsed_blocks = load_markdown_preview_blocks(&note_file);
        let ui_blocks: Vec<UIMarkdownBlock> = parsed_blocks
            .into_iter()
            .map(|b| UIMarkdownBlock {
                kind: SharedString::from(b.kind),
                text: SharedString::from(b.text.as_str()),
                subtext: SharedString::from(b.subtext.as_str()),
            })
            .collect();
        window.set_note_preview_blocks(ModelRc::from(Rc::new(VecModel::from(ui_blocks))));

        {
            let mut state = state_ref.borrow_mut();
            state.current_candidates.clear();
        }
        window.set_results(ModelRc::from(Rc::new(VecModel::from(Vec::<UIResultItem>::new()))));

        window.set_dynamic_height(MAX_EXPANDED_HEIGHT);
        window
            .window()
            .set_size(LogicalSize::new(WINDOW_WIDTH, MAX_EXPANDED_HEIGHT));
        return;
    }

    window.set_is_note_mode(false);

    let candidates = {
        let state = state_ref.borrow();
        state.engine.evaluate_with_bang(active_bang, query)
    };

    let mut header_count = 0usize;
    let mut prev_category: Option<&str> = None;

    let ui_items: Vec<UIResultItem> = candidates
        .iter()
        .map(|c| {
            let cat_str = c.category.as_str();
            let is_first = prev_category != Some(cat_str);
            if is_first {
                header_count += 1;
                prev_category = Some(cat_str);
            }

            let score_label = if c.score > 0 && c.score < 10_000 {
                format!("score {}", c.score)
            } else {
                String::new()
            };

            UIResultItem {
                title: SharedString::from(c.title.as_str()),
                badge: SharedString::from(c.badge),
                category: SharedString::from(cat_str),
                category_header: SharedString::from(c.category.header_title()),
                icon_text: SharedString::from(c.category.icon_glyph()),
                score_text: SharedString::from(score_label.as_str()),
                can_elevate: c.can_elevate,
                is_first_in_category: is_first,
            }
        })
        .collect();

    {
        let mut state = state_ref.borrow_mut();
        state.current_candidates = candidates;
    }

    let target_height = if ui_items.is_empty() {
        COMPACT_HEIGHT
    } else {
        let rows_h = (ui_items.len() as f32) * 45.0;
        let headers_h = (header_count as f32) * 25.0;
        (COMPACT_HEIGHT + rows_h + headers_h + 42.0).min(MAX_EXPANDED_HEIGHT)
    };

    window.set_dynamic_height(target_height);
    window
        .window()
        .set_size(LogicalSize::new(WINDOW_WIDTH, target_height));

    window.set_selected_index(0);
    window.set_results(ModelRc::from(Rc::new(VecModel::from(ui_items))));
}

fn parse_hex_color(hex: &str, fallback: Color) -> Color {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        ) {
            return Color::from_rgb_u8(r, g, b);
        }
    }
    fallback
}

fn build_ui_theme_colors(config: &AppConfig) -> UIThemeColors {
    let default_pt = UIThemeColors {
        window_bg: Color::from_rgb_u8(0x1f, 0x1f, 0x1f),
        surface_bg: Color::from_rgb_u8(0x2b, 0x2b, 0x2b),
        card_bg: Color::from_rgb_u8(0x28, 0x28, 0x28),
        hover_bg: Color::from_rgb_u8(0x32, 0x32, 0x32),
        selected_bg: Color::from_rgb_u8(0x38, 0x38, 0x38),
        border: Color::from_rgb_u8(0x3d, 0x3d, 0x3d),
        accent: Color::from_rgb_u8(0x60, 0xcd, 0xff),
        text_primary: Color::from_rgb_u8(0xff, 0xff, 0xff),
        text_secondary: Color::from_rgb_u8(0xd6, 0xd6, 0xd6),
        text_muted: Color::from_rgb_u8(0x9d, 0x9d, 0x9d),
    };

    if config.theme == "custom" {
        let ct = &config.custom_theme;
        let selected = parse_hex_color(&ct.selected_bg, default_pt.selected_bg);
        let text_pri = parse_hex_color(&ct.text_primary, default_pt.text_primary);
        UIThemeColors {
            window_bg: parse_hex_color(&ct.window_bg, default_pt.window_bg),
            surface_bg: parse_hex_color(&ct.surface_bg, default_pt.surface_bg),
            card_bg: parse_hex_color(&ct.card_bg, default_pt.card_bg),
            hover_bg: selected,
            selected_bg: selected,
            border: parse_hex_color(&ct.border, default_pt.border),
            accent: parse_hex_color(&ct.accent, default_pt.accent),
            text_primary: text_pri,
            text_secondary: text_pri,
            text_muted: parse_hex_color(&ct.text_muted, default_pt.text_muted),
        }
    } else {
        default_pt
    }
}

fn sync_settings_ui(settings_ui: &SettingsWindow, engine: &QueryEngine) {
    let config = &engine.config;
    let ui_theme = build_ui_theme_colors(config);

    settings_ui.set_theme_mode(SharedString::from(config.theme.as_str()));
    settings_ui.set_theme(ui_theme);

    settings_ui.set_custom_window_bg(SharedString::from(config.custom_theme.window_bg.as_str()));
    settings_ui.set_custom_surface_bg(SharedString::from(config.custom_theme.surface_bg.as_str()));
    settings_ui.set_custom_card_bg(SharedString::from(config.custom_theme.card_bg.as_str()));
    settings_ui.set_custom_selected_bg(SharedString::from(config.custom_theme.selected_bg.as_str()));
    settings_ui.set_custom_border(SharedString::from(config.custom_theme.border.as_str()));
    settings_ui.set_custom_accent(SharedString::from(config.custom_theme.accent.as_str()));
    settings_ui.set_custom_text_primary(SharedString::from(
        config.custom_theme.text_primary.as_str(),
    ));
    settings_ui.set_custom_text_muted(SharedString::from(config.custom_theme.text_muted.as_str()));

    settings_ui.set_show_apps(config.show_apps);
    settings_ui.set_custom_cmd_path(SharedString::from(config.custom_cmd_path.as_str()));
    settings_ui.set_custom_settings_path(SharedString::from(
        config.custom_system_settings_path.as_str(),
    ));
    settings_ui.set_indexed_files_count(engine.custom_files.len() as i32);

    // 1. Category Order
    let cat_items: Vec<UICategoryOrderItem> = config
        .ordered_categories()
        .into_iter()
        .map(|cat| UICategoryOrderItem {
            id: SharedString::from(cat.as_str()),
            label: SharedString::from(cat.display_name()),
            description: SharedString::from(cat.description()),
        })
        .collect();
    settings_ui.set_category_order(ModelRc::from(Rc::new(VecModel::from(cat_items))));

    // 2. Installed Apps with Checkboxes
    let filter_q = settings_ui.get_app_filter_text().to_ascii_lowercase();
    let app_toggle_items: Vec<UIAppToggleItem> = engine
        .apps
        .iter()
        .filter(|app| filter_q.trim().is_empty() || app.name_lower.contains(filter_q.trim()))
        .map(|app| UIAppToggleItem {
            name: SharedString::from(app.name.as_str()),
            visible: !config.is_app_hidden(&app.name),
        })
        .collect();
    settings_ui.set_app_items(ModelRc::from(Rc::new(VecModel::from(app_toggle_items))));

    // 3. Custom Commands
    let cmd_items: Vec<UISimpleEntry> = config
        .custom_commands
        .iter()
        .map(|c| UISimpleEntry {
            name: SharedString::from(c.name.as_str()),
            target: SharedString::from(c.command.as_str()),
        })
        .collect();
    settings_ui.set_custom_commands(ModelRc::from(Rc::new(VecModel::from(cmd_items))));

    // 4. Custom System Settings
    let set_items: Vec<UISimpleEntry> = config
        .custom_settings
        .iter()
        .map(|s| UISimpleEntry {
            name: SharedString::from(s.name.as_str()),
            target: SharedString::from(s.path.as_str()),
        })
        .collect();
    settings_ui.set_custom_settings(ModelRc::from(Rc::new(VecModel::from(set_items))));

    // 5. Custom File Scan Paths
    let scan_paths: Vec<SharedString> = config
        .custom_file_scan_paths
        .iter()
        .map(|p| SharedString::from(p.as_str()))
        .collect();
    settings_ui.set_custom_file_scan_paths(ModelRc::from(Rc::new(VecModel::from(scan_paths))));

    // 6. Web Bangs
    let mut bangs: Vec<UIBangEntry> = config
        .bangs
        .iter()
        .map(|(k, v)| UIBangEntry {
            key: SharedString::from(k.as_str()),
            url: SharedString::from(v.as_str()),
        })
        .collect();
    bangs.sort_by(|a, b| a.key.cmp(&b.key));
    settings_ui.set_bang_entries(ModelRc::from(Rc::new(VecModel::from(bangs))));
}

fn wire_settings_callbacks(
    settings_ui: &SettingsWindow,
    palette_ui: &PaletteWindow,
    state: &Rc<RefCell<PaletteState>>,
) {
    // Use PowerToys Default Theme
    {
        let settings_weak = settings_ui.as_weak();
        let palette_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_use_powertoys_theme(move || {
            let ui_theme = {
                let mut st = state_ref.borrow_mut();
                st.engine.config.theme = "powertoys".to_string();
                st.engine.config.save(CONFIG_FILE_PATH);
                build_ui_theme_colors(&st.engine.config)
            };
            if let Some(pal) = palette_weak.upgrade() {
                pal.set_theme(ui_theme);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                win.set_status_message(SharedString::from(
                    "Applied PowerToys Command Palette theme!",
                ));
            }
        });
    }

    // Save & Apply User Custom Theme
    {
        let settings_weak = settings_ui.as_weak();
        let palette_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_save_custom_theme(
            move |win_bg, surf_bg, card_bg, sel_bg, border, accent, txt_pri, txt_muted| {
                let ui_theme = {
                    let mut st = state_ref.borrow_mut();
                    st.engine.config.theme = "custom".to_string();
                    st.engine.config.custom_theme = CustomThemeConfig {
                        window_bg: win_bg.trim().to_string(),
                        surface_bg: surf_bg.trim().to_string(),
                        card_bg: card_bg.trim().to_string(),
                        selected_bg: sel_bg.trim().to_string(),
                        border: border.trim().to_string(),
                        accent: accent.trim().to_string(),
                        text_primary: txt_pri.trim().to_string(),
                        text_muted: txt_muted.trim().to_string(),
                    };
                    st.engine.config.save(CONFIG_FILE_PATH);
                    build_ui_theme_colors(&st.engine.config)
                };
                if let Some(pal) = palette_weak.upgrade() {
                    pal.set_theme(ui_theme);
                }
                if let Some(win) = settings_weak.upgrade() {
                    sync_settings_ui(&win, &state_ref.borrow().engine);
                    win.set_status_message(SharedString::from("Applied & saved Custom Theme!"));
                }
            },
        );
    }

    // Move Category Up
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_move_category_up(move |idx| {
            let i = idx as usize;
            if i > 0 {
                {
                    let mut st = state_ref.borrow_mut();
                    if i < st.engine.config.category_order.len() {
                        st.engine.config.category_order.swap(i, i - 1);
                        st.engine.config.save(CONFIG_FILE_PATH);
                    }
                }
                if let Some(win) = settings_weak.upgrade() {
                    sync_settings_ui(&win, &state_ref.borrow().engine);
                    win.set_status_message(SharedString::from("Updated category order"));
                }
            }
        });
    }

    // Move Category Down
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_move_category_down(move |idx| {
            let i = idx as usize;
            {
                let mut st = state_ref.borrow_mut();
                if i + 1 < st.engine.config.category_order.len() {
                    st.engine.config.category_order.swap(i, i + 1);
                    st.engine.config.save(CONFIG_FILE_PATH);
                }
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                win.set_status_message(SharedString::from("Updated category order"));
            }
        });
    }

    // Filter Installed Apps List
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_filter_apps_changed(move |_| {
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
            }
        });
    }

    // Toggle Individual App Checkbox
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_toggle_app_item(move |app_name, visible| {
            {
                let mut st = state_ref.borrow_mut();
                st.engine.config.set_app_visibility(app_name.as_str(), visible);
                st.engine.config.save(CONFIG_FILE_PATH);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                let msg = if visible {
                    format!("'{}' enabled in suggestions", app_name)
                } else {
                    format!("'{}' hidden from suggestions", app_name)
                };
                win.set_status_message(SharedString::from(msg));
            }
        });
    }

    // Show All / Hide All Apps
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_set_all_apps_visible(move |visible| {
            {
                let mut st = state_ref.borrow_mut();
                if visible {
                    st.engine.config.hidden_apps.clear();
                } else {
                    st.engine.config.hidden_apps =
                        st.engine.apps.iter().map(|a| a.name.clone()).collect();
                }
                st.engine.config.save(CONFIG_FILE_PATH);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                let msg = if visible {
                    "All apps enabled in suggestions"
                } else {
                    "All apps hidden from suggestions"
                };
                win.set_status_message(SharedString::from(msg));
            }
        });
    }

    // Save Default CMD Path
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_save_cmd_path(move |cmd_path| {
            {
                let mut st = state_ref.borrow_mut();
                st.engine.config.custom_cmd_path = cmd_path.to_string();
                st.engine.config.save(CONFIG_FILE_PATH);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                win.set_status_message(SharedString::from("Saved default CMD path!"));
            }
        });
    }

    // Add Custom Command
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_add_custom_command(move |name, command| {
            let n = name.trim().to_string();
            let c = command.trim().to_string();
            if n.is_empty() || c.is_empty() {
                return;
            }
            {
                let mut st = state_ref.borrow_mut();
                st.engine
                    .config
                    .custom_commands
                    .push(CustomCommandItem { name: n, command: c });
                st.engine.config.save(CONFIG_FILE_PATH);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                win.set_status_message(SharedString::from("Added custom command!"));
            }
        });
    }

    // Remove Custom Command
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_remove_custom_command(move |idx| {
            let i = idx as usize;
            {
                let mut st = state_ref.borrow_mut();
                if i < st.engine.config.custom_commands.len() {
                    st.engine.config.custom_commands.remove(i);
                    st.engine.config.save(CONFIG_FILE_PATH);
                }
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                win.set_status_message(SharedString::from("Removed custom command"));
            }
        });
    }

    // Save Default System Settings Path
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_save_settings_path(move |settings_path| {
            {
                let mut st = state_ref.borrow_mut();
                st.engine.config.custom_system_settings_path = settings_path.to_string();
                st.engine.config.save(CONFIG_FILE_PATH);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                win.set_status_message(SharedString::from("Saved default System Settings path!"));
            }
        });
    }

    // Add Custom System Setting
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_add_custom_setting(move |name, path| {
            let n = name.trim().to_string();
            let p = path.trim().to_string();
            if n.is_empty() || p.is_empty() {
                return;
            }
            {
                let mut st = state_ref.borrow_mut();
                st.engine
                    .config
                    .custom_settings
                    .push(CustomSettingItem { name: n, path: p });
                st.engine.config.save(CONFIG_FILE_PATH);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                win.set_status_message(SharedString::from("Added custom system setting!"));
            }
        });
    }

    // Remove Custom System Setting
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_remove_custom_setting(move |idx| {
            let i = idx as usize;
            {
                let mut st = state_ref.borrow_mut();
                if i < st.engine.config.custom_settings.len() {
                    st.engine.config.custom_settings.remove(i);
                    st.engine.config.save(CONFIG_FILE_PATH);
                }
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                win.set_status_message(SharedString::from("Removed custom system setting"));
            }
        });
    }

    // Add Custom File Scan Path
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_add_file_scan_path(move |scan_path| {
            let trimmed = scan_path.trim().to_string();
            if !trimmed.is_empty() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine.config.custom_file_scan_paths.push(trimmed);
                    st.engine.config.save(CONFIG_FILE_PATH);
                    st.engine.custom_files =
                        build_custom_files_index(&st.engine.config.custom_file_scan_paths);
                }
                if let Some(win) = settings_weak.upgrade() {
                    sync_settings_ui(&win, &state_ref.borrow().engine);
                    win.set_status_message(SharedString::from("Scanned custom path for files!"));
                }
            }
        });
    }

    // Remove Custom File Scan Path
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_remove_file_scan_path(move |idx| {
            let i = idx as usize;
            {
                let mut st = state_ref.borrow_mut();
                if i < st.engine.config.custom_file_scan_paths.len() {
                    st.engine.config.custom_file_scan_paths.remove(i);
                    st.engine.config.save(CONFIG_FILE_PATH);
                    st.engine.custom_files =
                        build_custom_files_index(&st.engine.config.custom_file_scan_paths);
                }
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                win.set_status_message(SharedString::from("Removed scan path & updated index"));
            }
        });
    }

    // Add Web Bang
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_add_bang(move |key, url| {
            let k = key.trim().to_ascii_lowercase();
            let u = url.trim().to_string();
            if !k.is_empty() && !u.is_empty() {
                {
                    let mut st = state_ref.borrow_mut();
                    st.engine.config.bangs.insert(k, u);
                    st.engine.config.save(CONFIG_FILE_PATH);
                }
                if let Some(win) = settings_weak.upgrade() {
                    sync_settings_ui(&win, &state_ref.borrow().engine);
                    win.set_status_message(SharedString::from("Saved web bang shortcut!"));
                }
            }
        });
    }

    // Remove Web Bang
    {
        let settings_weak = settings_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_remove_bang(move |key| {
            {
                let mut st = state_ref.borrow_mut();
                st.engine.config.bangs.remove(key.as_str());
                st.engine.config.save(CONFIG_FILE_PATH);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                win.set_status_message(SharedString::from("Removed web bang shortcut"));
            }
        });
    }

    // Reload All Indexes & Config
    {
        let settings_weak = settings_ui.as_weak();
        let palette_weak = palette_ui.as_weak();
        let state_ref = Rc::clone(state);
        settings_ui.on_reload_index(move || {
            let cfg = AppConfig::load_or_create(CONFIG_FILE_PATH);
            let apps = build_start_menu_index();
            let files = build_custom_files_index(&cfg.custom_file_scan_paths);
            let ui_theme = build_ui_theme_colors(&cfg);
            {
                let mut st = state_ref.borrow_mut();
                st.engine = QueryEngine::new(cfg, apps, files);
            }
            if let Some(win) = settings_weak.upgrade() {
                sync_settings_ui(&win, &state_ref.borrow().engine);
                win.set_status_message(SharedString::from("Rescanned Apps & Custom File Paths!"));
            }
            if let Some(pal) = palette_weak.upgrade() {
                pal.set_theme(ui_theme);
                let bang = pal.get_active_bang().to_string();
                let q = pal.get_query_text().to_string();
                refresh_results(&pal, &state_ref, &bang, &q);
            }
        });
    }
}
