# ⚡ cmdpal-rs

[![Build Status](https://github.com/nayaksudhanshu120999/r/actions/workflows/build.yml/badge.svg)](https://github.com/nayaksudhanshu120999/r/actions)
![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11-0078D4?logo=windows)
![Rust](https://img.shields.io/badge/rust-2021%20edition-dea584?logo=rust)
![UI](https://img.shields.io/badge/UI-Slint%201.8-6366f1)

**`cmdpal-rs`** is an ultra-lightweight, native Windows Command Palette written in **Rust** and **Slint**. Designed for speed and zero background bloat, it stays resident in the Windows system tray and summons instantaneously from anywhere via **`Ctrl + Space`** (or your own custom global hotkey).

---

## ✨ Features

* **🚀 Always-On & Customizable Global Shortcut (`Ctrl + Space` by default)**
  * Runs silently in the background with **Single-Instance enforcement** (launching the app again focuses the existing instance instead of spawning duplicate processes or tray icons).
  * Switch hotkeys **live without restarting**—type `hotkey` or `shortcut` in the palette to pick a preset (`Ctrl+Space`, `Alt+Space`, `Ctrl+Shift+Space`, `Ctrl+Alt+Space`, `Alt+Q`), or configure any combination of `Ctrl`, `Alt`, `Shift`, `Win` + `Space`, `Enter`, `Tab`, `A–Z`, `0–9`, or `F1–F12` in `config.toml`.
* **🔄 Run on Windows Startup**
  * Toggle automatic launch at Windows login (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`) directly from the **System Tray right-click menu**, by typing **`startup`** / **`autostart`** in the palette, or via `run_on_startup = true` in `config.toml`.
* **🧮 Inline Calculator & Base Converter**
  * Evaluate arithmetic expressions (`145 * 32`, `2 ^ 10`), scientific functions (`sqrt(144)`, `sin(pi / 2)`, `log(100)`), hex/binary literals (`0xff + 1`, `0b1010 * 2`), and base conversions (`255 to hex`, `0xff to dec`, `42 to bin`) in real time.
  * Press **`Enter`** on any calculator result to copy the answer directly to the Windows clipboard.
* **🔗 Direct URL & File/Folder Path Detection**
  * **File & Folder Paths**: Paste or type local paths (`C:\Windows\System32`, `"C:\Program Files"`), UNC paths (`\\server\share`), or environment/home paths (`%APPDATA%`, `%TEMP%`, `~\Downloads`) to open the file or folder directly.
  * **Direct URLs**: Type or paste URLs (`https://...`, `www.rust-lang.org`), local dev servers (`localhost:3000`, `127.0.0.1:8080`), or bare domains (`github.com/rust-lang`, `crates.io`) to launch them straight in your default browser via native Win32 `ShellExecuteW`.
* **📌 Taskbar System Tray Integration**
  * Native Windows notification area icon with left-click toggle and right-click context menu (*Open Command Palette*, *✓ Run on Windows Startup*, *Settings...*, *Reload Apps & Config*, *Quit*).
* **🎯 Smart Auto-Hide**
  * Automatically hides the palette as soon as a task is executed, when the window loses focus, when you click outside the palette, or when `Esc` is pressed.
* **🔍 Fuzzy Search Engine (`SkimMatcherV2`)**
  * Evaluates Installed Applications, System Settings, System Power Commands, and Custom Files in real time.
  * Ranks suggestions directly by their **fuzzy match score**, using your configured **Category Order of Preference** as a smart tie-breaker.
* **🏷️ Interactive Bang Pill Chips**
  * Type a bang key (such as `yt`, `gh`, `g`, `rs`) and press **`Space`** to automatically convert it into a **`🌐 YT ✕`** pill chip inside the search bar.
  * Type your search query right after the chip and press `Enter` to search, or press `Backspace` on an empty input to remove the chip.
* **🎨 PowerToys Command Palette Theme & Custom Theme Builder**
  * Ships with a default **PowerToys Command Palette** theme (Windows 11 Dark Acrylic `#1f1f1f` with Fluent Cyan `#60cdff` accents).
  * Includes a live **Custom Theme Builder** (`#RRGGBB` hex editor) in Settings to customize every surface, border, accent, and text color.
* **📂 Targeted Custom File Scanner (No Full-Disk Scan)**
  * Add specific folders in Settings to index their files and open them directly from the palette—without slowing down your PC with a full-disk scan.
* **🛠️ Full-Featured Settings GUI**
  * Resizable & maximizable native Settings window to manage themes, category priority order, installed app visibility checkboxes, custom commands, custom settings, file scan paths, and web bangs.

---

## ⚡ Quick Query Examples

| What You Type | What Appears at the Top | Action on `Enter` |
| :--- | :--- | :--- |
| **`145 * 32`** or **`(2 + 3) ^ 4`** | `145 * 32 = 4640 (Enter to Copy)` | Copies `4640` to clipboard |
| **`sqrt(144) + 0xff`** | `sqrt(144) + 0xff = 267 (Enter to Copy)` | Copies `267` to clipboard |
| **`255 to hex`** / **`42 to bin`** | `255 = 0xFF` / `42 = 0b101010` | Copies `0xFF` / `0b101010` to clipboard |
| **`0xff`** | `0xff = 255 • Hex: 0xFF • Bin: 0b11111111` | Copies `255` to clipboard |
| **`%APPDATA%`** or **`~\Downloads`** | `Open Folder: C:\Users\...\AppData\Roaming` | Opens the folder in File Explorer |
| **`C:\Windows\System32\drivers\etc\hosts`** | `Open File: C:\Windows\...\hosts` | Opens the file directly |
| **`localhost:3000`** or **`crates.io`** | `Open URL: http://localhost:3000` | Opens URL in default browser |
| **`startup`** | `Run on Windows Startup: OFF (Press Enter to Enable)` | Toggles Windows Startup registry key |
| **`hotkey`** | `Set Global Hotkey: Alt+Space` *(and presets)* | Rebinds global shortcut live |

---

## ⌨️ Keyboard & Mouse Controls

| Input / Shortcut | Action |
| :--- | :--- |
| **`Ctrl + Space`** *(or custom `hotkey`)* | Toggle Command Palette from anywhere in Windows |
| **`↑` / `↓`** or **`Tab` / `Shift + Tab`** | Navigate suggestions (with wrap-around & auto-scroll) |
| **`PageUp` / `PageDown`** | Jump 5 suggestions up or down |
| **`Enter`** or **Left-Click** | Open / Execute selected suggestion (or copy calculator result) |
| **`Ctrl + Enter`** or **Right-Click → Admin** | Run selected application or command as **Administrator** |
| **`<bang>` + `Space`** *(e.g. `yt `)* | Convert bang key into a **Bang Pill Chip** inside the search bar |
| **`Backspace`** *(when query is empty)* | Remove the active Bang Pill Chip |
| **`Ctrl + ,`** or **Click `⚙`** | Open the Command Palette Settings GUI |
| **`Esc`** | Close context menu → Clear search query / bang chip → Hide palette |
| **Mouse Hover / Mouse Wheel** | Highlight suggestion under cursor / Scroll suggestion list |
| **Click Outside Palette** | Automatically hide the Command Palette |

---

## ⚙️ Settings GUI Overview

Open Settings via the **`⚙`** icon in the palette, by pressing **`Ctrl + ,`**, by typing `palette settings`, or by right-clicking the **System Tray icon → `Settings...`**:

1. **Theme & Order**: Switch between the default **PowerToys Command Palette** theme and your **Custom Theme** (`#RRGGBB` hex editor), and reorder categories (`Cmds`, `Settings`, `Apps`, `Files`, `Web`).
2. **Installed Apps**: View all indexed Start Menu applications with a live search filter, **☑ Show All / ☐ Hide All** buttons, and individual checkboxes to show or hide specific apps from suggestions.
3. **Custom Commands**: Simplified 2-field form (*Name* + *Command or EXE Path*) and default `cmd.exe` path configuration.
4. **Custom Settings**: Simplified 2-field form (*Name* + *`ms-settings:` URI, `.cpl`, or `.msc` Path*).
5. **Custom File Scan**: Add specific folder paths to index their files and open them directly under the `CUSTOM FILES` category.
6. **Web Search Bangs**: Add or remove prefix-free web search shortcuts using `{q}` URL templates.

---

## 🌐 Default Web Search Bangs

Type any bang key followed by `Space` to activate its search chip:

| Bang Key | Service | URL Template |
| :--- | :--- | :--- |
| **`g`** | Google | `https://www.google.com/search?q={q}` |
| **`yt`** | YouTube | `https://www.youtube.com/results?search_query={q}` |
| **`gh`** | GitHub | `https://github.com/search?q={q}` |
| **`ddg`** | DuckDuckGo | `https://duckduckgo.com/?q={q}` |
| **`rs`** | Docs.rs | `https://docs.rs/releases/search?query={q}` |
| **`crates`** | Crates.io | `https://crates.io/search?q={q}` |
| **`so`** | Stack Overflow | `https://stackoverflow.com/search?q={q}` |
| **`wiki`** | Wikipedia | `https://en.wikipedia.org/w/index.php?search={q}` |
| **`r`** | Reddit | `https://www.reddit.com/search/?q={q}` |
| **`maps`** | Google Maps | `https://www.google.com/maps/search/{q}` |

---

## 🗂️ Project Structure

```text
cmdpal-rs/
├── .github/
│   └── workflows/
│       └── build.yml     # CI release build workflow (x86_64-pc-windows-msvc)
├── Cargo.toml            # Crate dependencies & size-optimized release profile
├── build.rs              # Compiles Slint UI & embeds procedural .ico into .exe
├── config.toml           # Persistent TOML configuration file
├── ui/
│   └── palette.slint     # PaletteWindow & SettingsWindow Slint UI definitions
└── src/
    ├── main.rs           # Application state, event loop, callbacks & theme sync
    ├── icon.rs           # Procedural RGBA/BGRA app & tray icon generator
    ├── system.rs         # Win32 tray icon, live hotkey rebinding, startup registry & focus monitor
    ├── models.rs         # Core data structures, categories & execution targets
    ├── config.rs         # TOML serialization/deserialization, exe-relative path resolution & normalization
    ├── indexer.rs        # Start Menu app indexer & targeted custom folder file scanner
    ├── engine.rs         # SkimV2 fuzzy search, inline calculator, direct URL/path detector & bang evaluator
    └── executor.rs       # Native Win32 ShellExecuteW launcher, clipboard copy, UAC elevation & process runner
```

---

## 🛠️ Building from Source

### Prerequisites
* **Windows 10 / 11** (`x86_64-pc-windows-msvc`)
* [**Rust Toolchain**](https://rustup.rs/) (stable)

### Build & Run
```powershell
# Run in development mode
cargo run

# Build size-optimized release binary
cargo build --release
```
The compiled binary will be located at `target\release\cmdpal-rs.exe`.
