use std::sync::mpsc::{self, Receiver};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemEvent {
    ShowPalette,
    TogglePalette,
    OpenSettings,
    ReloadConfig,
    QuitApp,
}

/// Parses a human-readable shortcut (e.g. `"Ctrl+Space"`, `"Alt+Space"`, `"Ctrl+Shift+Space"`, `"Alt+Q"`)
/// into `(win32_modifiers, virtual_key_code, canonical_display_label)`.
pub fn parse_hotkey_spec(raw: &str) -> (u32, u32, String) {
    const MOD_ALT_FLAG: u32 = 0x0001;
    const MOD_CONTROL_FLAG: u32 = 0x0002;
    const MOD_SHIFT_FLAG: u32 = 0x0004;
    const MOD_WIN_FLAG: u32 = 0x0008;
    const MOD_NOREPEAT_FLAG: u32 = 0x4000;

    let mut mods: u32 = 0;
    let mut vk: Option<u32> = None;
    let mut key_label = String::new();

    for token in raw.split('+').map(str::trim).filter(|s| !s.is_empty()) {
        let lower = token.to_ascii_lowercase();
        match lower.as_str() {
            "ctrl" | "control" | "ctl" => mods |= MOD_CONTROL_FLAG,
            "alt" | "menu" => mods |= MOD_ALT_FLAG,
            "shift" | "shft" => mods |= MOD_SHIFT_FLAG,
            "win" | "super" | "meta" | "windows" => mods |= MOD_WIN_FLAG,
            "space" | "spacebar" => {
                vk = Some(0x20);
                key_label = "Space".to_string();
            }
            "enter" | "return" => {
                vk = Some(0x0D);
                key_label = "Enter".to_string();
            }
            "tab" => {
                vk = Some(0x09);
                key_label = "Tab".to_string();
            }
            "backspace" | "bksp" => {
                vk = Some(0x08);
                key_label = "Backspace".to_string();
            }
            "`" | "tilde" | "backtick" => {
                vk = Some(0xC0);
                key_label = "`".to_string();
            }
            _ => {
                if let Some(num_str) = lower.strip_prefix('f') {
                    if let Ok(n) = num_str.parse::<u32>() {
                        if (1..=12).contains(&n) {
                            vk = Some(0x70 + (n - 1));
                            key_label = format!("F{}", n);
                            continue;
                        }
                    }
                }
                if token.len() == 1 {
                    let ch = token.chars().next().unwrap();
                    if ch.is_ascii_alphabetic() {
                        let up = ch.to_ascii_uppercase();
                        vk = Some(up as u32);
                        key_label = up.to_string();
                    } else if ch.is_ascii_digit() {
                        vk = Some(ch as u32);
                        key_label = ch.to_string();
                    }
                }
            }
        }
    }

    if mods == 0 || vk.is_none() {
        return (
            MOD_CONTROL_FLAG | MOD_NOREPEAT_FLAG,
            0x20,
            "Ctrl+Space".to_string(),
        );
    }

    let mut parts = Vec::with_capacity(5);
    if (mods & MOD_CONTROL_FLAG) != 0 {
        parts.push("Ctrl".to_string());
    }
    if (mods & MOD_ALT_FLAG) != 0 {
        parts.push("Alt".to_string());
    }
    if (mods & MOD_SHIFT_FLAG) != 0 {
        parts.push("Shift".to_string());
    }
    if (mods & MOD_WIN_FLAG) != 0 {
        parts.push("Win".to_string());
    }
    parts.push(key_label);

    (mods | MOD_NOREPEAT_FLAG, vk.unwrap(), parts.join("+"))
}

pub fn normalize_hotkey_str(raw: &str) -> String {
    parse_hotkey_spec(raw).2
}

#[cfg(target_os = "windows")]
mod win32 {
    use super::{parse_hotkey_spec, SystemEvent};
    use crate::icon::create_win32_tray_icon_buffers;
    use std::mem::zeroed;
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use std::ptr::{null, null_mut};
    use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
    use std::sync::mpsc::Sender;
    use std::sync::{Mutex, OnceLock};
    use std::thread;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, RegisterHotKey, SetFocus, UnregisterHotKey, MOD_CONTROL, MOD_NOREPEAT,
        VK_LBUTTON, VK_MBUTTON, VK_RBUTTON, VK_SPACE,
    };
    use windows_sys::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
        NOTIFYICONDATAW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIcon, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
        DestroyMenu, DestroyWindow, DispatchMessageW, FindWindowW, GetCursorPos,
        GetForegroundWindow, GetMessageW, GetSystemMetrics, GetWindowLongW, GetWindowRect,
        GetWindowThreadProcessId, IsWindow, PostMessageW, PostQuitMessage, RegisterClassW,
        SetForegroundWindow, SetWindowLongW, SetWindowPos, ShowWindow, TrackPopupMenu,
        TranslateMessage, GWL_STYLE, MF_CHECKED, MF_SEPARATOR, MF_STRING, MF_UNCHECKED, MSG,
        SM_CXSCREEN, SM_CYSCREEN, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
        SW_RESTORE, SW_SHOW, TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
        WM_APP, WM_CLOSE, WM_DESTROY, WM_HOTKEY, WM_LBUTTONUP, WM_RBUTTONUP, WNDCLASSW,
        WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_THICKFRAME,
    };

    const CREATE_NO_WINDOW: u32 = 0x08000000;
    const RUN_REG_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    const RUN_REG_VALUE_NAME: &str = "CmdPalRs";

    const TRAY_CLASS_NAME: &str = "CmdPalRsTrayHiddenWndClass";
    const WM_TRAYICON: u32 = WM_APP + 1;
    const WM_IPC_SHOW_PALETTE: u32 = WM_APP + 2;
    const WM_APP_REBIND_HOTKEY: u32 = WM_APP + 3;
    const HOTKEY_ID_TOGGLE: i32 = 1;

    const MENU_OPEN_PALETTE: u32 = 1001;
    const MENU_OPEN_SETTINGS: u32 = 1002;
    const MENU_RELOAD_CONFIG: u32 = 1003;
    const MENU_QUIT_APP: u32 = 1004;
    const MENU_TOGGLE_STARTUP: u32 = 1005;

    static EVENT_SENDER: OnceLock<Sender<SystemEvent>> = OnceLock::new();
    static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);
    static CACHED_PALETTE_HWND: AtomicIsize = AtomicIsize::new(0);

    static HOTKEY_MODS: AtomicU32 = AtomicU32::new((MOD_CONTROL | MOD_NOREPEAT) as u32);
    static HOTKEY_VK: AtomicU32 = AtomicU32::new(VK_SPACE as u32);
    static STARTUP_ENABLED: AtomicBool = AtomicBool::new(false);
    static HOTKEY_LABEL: OnceLock<Mutex<String>> = OnceLock::new();

    fn current_hotkey_label() -> String {
        HOTKEY_LABEL
            .get_or_init(|| Mutex::new("Ctrl+Space".to_string()))
            .lock()
            .map(|g| g.clone())
            .unwrap_or_else(|_| "Ctrl+Space".to_string())
    }

    fn set_hotkey_label(label: String) {
        if let Ok(mut guard) = HOTKEY_LABEL
            .get_or_init(|| Mutex::new("Ctrl+Space".to_string()))
            .lock()
        {
            *guard = label;
        }
    }

    fn to_wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn emit_event(event: SystemEvent) {
        if let Some(tx) = EVENT_SENDER.get() {
            let _ = tx.send(event);
        }
        let _ = slint::invoke_from_event_loop(|| {});
    }

    pub fn apply_runtime_config(run_on_startup: bool, hotkey_raw: &str) {
        let (mods, vk, label) = parse_hotkey_spec(hotkey_raw);
        let prev_mods = HOTKEY_MODS.swap(mods, Ordering::SeqCst);
        let prev_vk = HOTKEY_VK.swap(vk, Ordering::SeqCst);
        set_hotkey_label(label);

        let prev_startup = STARTUP_ENABLED.swap(run_on_startup, Ordering::SeqCst);
        if prev_startup != run_on_startup || run_on_startup {
            sync_windows_startup_registry(run_on_startup);
        }

        let hwnd_val = TRAY_HWND.load(Ordering::SeqCst);
        if hwnd_val != 0 && (prev_mods != mods || prev_vk != vk) {
            unsafe {
                PostMessageW(hwnd_val as HWND, WM_APP_REBIND_HOTKEY, 0, 0);
            }
        }
    }

    fn sync_windows_startup_registry(enable: bool) {
        if enable {
            if let Ok(exe_path) = std::env::current_exe() {
                let quoted = format!("\"{}\"", exe_path.to_string_lossy());
                let _ = Command::new("reg.exe")
                    .args([
                        "add",
                        RUN_REG_KEY,
                        "/v",
                        RUN_REG_VALUE_NAME,
                        "/t",
                        "REG_SZ",
                        "/d",
                        &quoted,
                        "/f",
                    ])
                    .creation_flags(CREATE_NO_WINDOW)
                    .status();
            }
        } else {
            let _ = Command::new("reg.exe")
                .args(["delete", RUN_REG_KEY, "/v", RUN_REG_VALUE_NAME, "/f"])
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
    }

    pub fn ensure_single_instance() -> bool {
        unsafe {
            let class_wide = to_wide(TRAY_CLASS_NAME);
            let existing_hwnd = FindWindowW(class_wide.as_ptr(), null());
            if !existing_hwnd.is_null() {
                PostMessageW(existing_hwnd, WM_IPC_SHOW_PALETTE, 0, 0);
                return false;
            }
            true
        }
    }

    pub fn cleanup_tray_window() {
        let hwnd_val = TRAY_HWND.load(Ordering::SeqCst);
        if hwnd_val != 0 {
            unsafe {
                PostMessageW(hwnd_val as HWND, WM_CLOSE, 0, 0);
            }
        }
    }

    pub fn spawn_tray_and_hotkey_thread(tx: Sender<SystemEvent>) {
        let _ = EVENT_SENDER.set(tx);

        thread::spawn(move || unsafe {
            let hinstance = GetModuleHandleW(null());
            let class_name = to_wide(TRAY_CLASS_NAME);

            let mut wc: WNDCLASSW = zeroed();
            wc.lpfnWndProc = Some(tray_wnd_proc);
            wc.hInstance = hinstance;
            wc.lpszClassName = class_name.as_ptr();
            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                to_wide("CmdPalRsTrayHost").as_ptr(),
                0,
                0,
                0,
                0,
                0,
                null_mut(),
                null_mut(),
                hinstance,
                null(),
            );

            if hwnd.is_null() {
                return;
            }
            TRAY_HWND.store(hwnd as isize, Ordering::SeqCst);

            let mods = HOTKEY_MODS.load(Ordering::SeqCst);
            let vk = HOTKEY_VK.load(Ordering::SeqCst);
            let _ = RegisterHotKey(hwnd, HOTKEY_ID_TOGGLE, mods, vk);

            let (and_mask, bgra) = create_win32_tray_icon_buffers(32);
            let hicon = CreateIcon(
                hinstance,
                32,
                32,
                1,
                32,
                and_mask.as_ptr(),
                bgra.as_ptr(),
            );

            let mut nid: NOTIFYICONDATAW = zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_TRAYICON;
            nid.hIcon = hicon;

            let tip_text = format!("Command Palette ({})", current_hotkey_label());
            let tip = to_wide(&tip_text);
            for (i, &ch) in tip.iter().take(127).enumerate() {
                nid.szTip[i] = ch;
            }

            Shell_NotifyIconW(NIM_ADD, &nid);

            let mut msg: MSG = zeroed();
            while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            Shell_NotifyIconW(NIM_DELETE, &nid);
            let _ = UnregisterHotKey(hwnd, HOTKEY_ID_TOGGLE);
            if !hicon.is_null() {
                DestroyIcon(hicon);
            }
        });
    }

    unsafe extern "system" fn tray_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_HOTKEY => {
                if wparam as i32 == HOTKEY_ID_TOGGLE {
                    emit_event(SystemEvent::TogglePalette);
                }
                0
            }
            WM_APP_REBIND_HOTKEY => {
                let _ = UnregisterHotKey(hwnd, HOTKEY_ID_TOGGLE);
                let mods = HOTKEY_MODS.load(Ordering::SeqCst);
                let vk = HOTKEY_VK.load(Ordering::SeqCst);
                let _ = RegisterHotKey(hwnd, HOTKEY_ID_TOGGLE, mods, vk);

                let mut nid: NOTIFYICONDATAW = zeroed();
                nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = hwnd;
                nid.uID = 1;
                nid.uFlags = NIF_TIP;
                let tip_text = format!("Command Palette ({})", current_hotkey_label());
                let tip = to_wide(&tip_text);
                for (i, &ch) in tip.iter().take(127).enumerate() {
                    nid.szTip[i] = ch;
                }
                Shell_NotifyIconW(NIM_MODIFY, &nid);
                0
            }
            WM_IPC_SHOW_PALETTE => {
                emit_event(SystemEvent::ShowPalette);
                0
            }
            WM_TRAYICON => {
                let mouse_msg = (lparam as u32) & 0xFFFF;
                if mouse_msg == WM_LBUTTONUP {
                    emit_event(SystemEvent::TogglePalette);
                } else if mouse_msg == WM_RBUTTONUP {
                    show_tray_context_menu(hwnd);
                }
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn show_tray_context_menu(hwnd: HWND) {
        let hmenu = CreatePopupMenu();
        if hmenu.is_null() {
            return;
        }

        let hk_label = current_hotkey_label();
        let open_text = format!("Open Command Palette\t{}", hk_label);
        let label_open = to_wide(&open_text);
        let label_startup = to_wide("Run on Windows Startup");
        let label_settings = to_wide("Settings...");
        let label_reload = to_wide("Reload Apps & Config");
        let label_quit = to_wide("Quit Command Palette");

        let startup_flags = if STARTUP_ENABLED.load(Ordering::SeqCst) {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING | MF_UNCHECKED
        };

        AppendMenuW(
            hmenu,
            MF_STRING,
            MENU_OPEN_PALETTE as usize,
            label_open.as_ptr(),
        );
        AppendMenuW(
            hmenu,
            startup_flags,
            MENU_TOGGLE_STARTUP as usize,
            label_startup.as_ptr(),
        );
        AppendMenuW(
            hmenu,
            MF_STRING,
            MENU_OPEN_SETTINGS as usize,
            label_settings.as_ptr(),
        );
        AppendMenuW(
            hmenu,
            MF_STRING,
            MENU_RELOAD_CONFIG as usize,
            label_reload.as_ptr(),
        );
        AppendMenuW(hmenu, MF_SEPARATOR, 0, null());
        AppendMenuW(hmenu, MF_STRING, MENU_QUIT_APP as usize, label_quit.as_ptr());

        let mut pt: POINT = zeroed();
        GetCursorPos(&mut pt);
        SetForegroundWindow(hwnd);

        let selected = TrackPopupMenu(
            hmenu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_LEFTALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            null(),
        );
        PostMessageW(hwnd, 0, 0, 0);
        DestroyMenu(hmenu);

        match selected as u32 {
            MENU_OPEN_PALETTE => emit_event(SystemEvent::TogglePalette),
            MENU_TOGGLE_STARTUP => {
                let mut cfg = crate::config::AppConfig::load_or_create("config.toml");
                cfg.run_on_startup = !cfg.run_on_startup;
                cfg.save("config.toml");
                emit_event(SystemEvent::ReloadConfig);
            }
            MENU_OPEN_SETTINGS => emit_event(SystemEvent::OpenSettings),
            MENU_RELOAD_CONFIG => emit_event(SystemEvent::ReloadConfig),
            MENU_QUIT_APP => emit_event(SystemEvent::QuitApp),
            _ => {}
        }
    }

    pub fn find_window_by_title(title: &str) -> HWND {
        unsafe {
            let wide = to_wide(title);
            FindWindowW(null(), wide.as_ptr())
        }
    }

    fn get_palette_hwnd(palette_title: &str) -> HWND {
        let cached = CACHED_PALETTE_HWND.load(Ordering::Relaxed);
        unsafe {
            if cached != 0 && IsWindow(cached as HWND) != 0 {
                return cached as HWND;
            }
        }
        let found = find_window_by_title(palette_title);
        if !found.is_null() {
            CACHED_PALETTE_HWND.store(found as isize, Ordering::Relaxed);
        }
        found
    }

    pub fn enable_maximize_and_resize(title: &str) {
        unsafe {
            let hwnd = find_window_by_title(title);
            if hwnd.is_null() {
                return;
            }
            let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
            let new_style = style | WS_MAXIMIZEBOX | WS_MINIMIZEBOX | WS_THICKFRAME;
            SetWindowLongW(hwnd, GWL_STYLE, new_style as i32);
            SetWindowPos(
                hwnd,
                null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }

    pub fn force_activate_window(title: &str) {
        unsafe {
            let hwnd = find_window_by_title(title);
            if hwnd.is_null() {
                return;
            }
            let fg_hwnd = GetForegroundWindow();
            let current_thread = GetCurrentThreadId();
            let fg_thread = GetWindowThreadProcessId(fg_hwnd, null_mut());

            if fg_thread != 0 && fg_thread != current_thread {
                AttachThreadInput(fg_thread, current_thread, 1);
                ShowWindow(hwnd, SW_RESTORE);
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
                SetFocus(hwnd);
                AttachThreadInput(fg_thread, current_thread, 0);
            } else {
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
                SetFocus(hwnd);
            }
        }
    }

    pub fn get_primary_screen_size() -> (f32, f32) {
        unsafe {
            let w = GetSystemMetrics(SM_CXSCREEN).max(1280) as f32;
            let h = GetSystemMetrics(SM_CYSCREEN).max(720) as f32;
            (w, h)
        }
    }

    pub fn should_auto_hide_palette(palette_title: &str, check_focus: bool) -> bool {
        unsafe {
            let hwnd = get_palette_hwnd(palette_title);
            if hwnd.is_null() {
                return false;
            }

            let l_down = (GetAsyncKeyState(VK_LBUTTON as i32) as u16 & 0x8000) != 0;
            let r_down = (GetAsyncKeyState(VK_RBUTTON as i32) as u16 & 0x8000) != 0;
            let m_down = (GetAsyncKeyState(VK_MBUTTON as i32) as u16 & 0x8000) != 0;

            if l_down || r_down || m_down {
                let mut pt: POINT = zeroed();
                let mut rect: RECT = zeroed();
                if GetCursorPos(&mut pt) != 0 && GetWindowRect(hwnd, &mut rect) != 0 {
                    let inside = pt.x >= rect.left
                        && pt.x <= rect.right
                        && pt.y >= rect.top
                        && pt.y <= rect.bottom;
                    if !inside {
                        return true;
                    }
                }
            }

            if check_focus {
                let fg = GetForegroundWindow();
                if !fg.is_null() && fg != hwnd {
                    return true;
                }
            }

            false
        }
    }
}

pub fn apply_runtime_system_config(run_on_startup: bool, hotkey: &str) {
    #[cfg(target_os = "windows")]
    win32::apply_runtime_config(run_on_startup, hotkey);
    #[cfg(not(target_os = "windows"))]
    let _ = (run_on_startup, hotkey);
}

pub fn request_config_reload() {
    #[cfg(target_os = "windows")]
    win32::emit_event(SystemEvent::ReloadConfig);
}

pub fn ensure_single_instance_or_activate_existing() -> bool {
    #[cfg(target_os = "windows")]
    {
        win32::ensure_single_instance()
    }
    #[cfg(not(target_os = "windows"))]
    {
        true
    }
}

pub fn shutdown_system_integration() {
    #[cfg(target_os = "windows")]
    win32::cleanup_tray_window();
}

pub fn init_system_integration() -> Receiver<SystemEvent> {
    let (tx, rx) = mpsc::channel();
    #[cfg(target_os = "windows")]
    win32::spawn_tray_and_hotkey_thread(tx);
    #[cfg(not(target_os = "windows"))]
    let _ = tx;
    rx
}

pub fn activate_window_by_title(title: &str) {
    #[cfg(target_os = "windows")]
    win32::force_activate_window(title);
    #[cfg(not(target_os = "windows"))]
    let _ = title;
}

pub fn enable_window_maximize(title: &str) {
    #[cfg(target_os = "windows")]
    win32::enable_maximize_and_resize(title);
    #[cfg(not(target_os = "windows"))]
    let _ = title;
}

pub fn primary_screen_size() -> (f32, f32) {
    #[cfg(target_os = "windows")]
    {
        win32::get_primary_screen_size()
    }
    #[cfg(not(target_os = "windows"))]
    {
        (1920.0, 1080.0)
    }
}

pub fn check_outside_click_or_focus_loss(palette_title: &str, check_focus: bool) -> bool {
    #[cfg(target_os = "windows")]
    {
        win32::should_auto_hide_palette(palette_title, check_focus)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (palette_title, check_focus);
        false
    }
}
