# ◈ Arch Explorer (egui v2 Native)

<p align="center">
  <img src="docs/screenshots/arch_explorer_multiselection_transparent.gif" alt="Arch Explorer Transparent Multiselection Demo" width="92%">
</p>

<p align="center">
  <strong>Blazing fast, featherweight, hardware-accelerated pure native file manager for Arch Linux & Wayland.</strong><br>
  Built with 100% Native Rust + egui + wgpu · Zero WebKit/Chromium/Tauri overhead · ~40MB RAM · True Desktop Glassmorphism
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Platform-Linux%20%2F%20Wayland%20%2F%20X11-blue?style=flat-square" alt="Platform">
  <img src="https://img.shields.io/badge/UI%20Engine-Pure%20egui%20(wgpu)-purple?style=flat-square" alt="Engine">
  <img src="https://img.shields.io/badge/Memory-~40MB%20RAM-brightgreen?style=flat-square" alt="RAM">
  <img src="https://img.shields.io/badge/Release-v0.2.0--2-orange?style=flat-square" alt="Release">
  <img src="https://img.shields.io/badge/License-MIT-teal?style=flat-square" alt="License">
</p>

---

## ⚡ Direct Download & Instant Run (No `cargo build` Needed!)

You don't need to install Rust or compile anything from source! Prebuilt binaries and native packages are maintained directly in this repository:

### 1. Instant Run (Standalone Binary):
```bash
git clone https://github.com/basihamam/arch-explorer-eguine.git
cd arch-explorer-eguine
chmod +x ./bin/arch-explorer
./bin/arch-explorer
```

### 2. Native Arch Linux Install (pacman):
```bash
sudo pacman -U bin/arch-explorer-bin-0.2.0-2-x86_64.pkg.tar.zst
```

### 3. AUR (Arch User Repository):
```bash
yay -S arch-explorer-bin
# or: paru -S arch-explorer-bin
```

---

## 🚀 Building From Source (Developers)

If you wish to compile locally, all Wayland extensions and our custom Wayland DnD engine are self-contained in `crates/winit`:

### Prerequisites (Arch Linux):
```bash
sudo pacman -S --needed rustup base-devel wayland wayland-protocols libxkbcommon gtk3 hicolor-icon-theme
```

### Build & Run:
```bash
git clone https://github.com/basihamam/arch-explorer-eguine.git
cd arch-explorer-eguine
cargo run --release
```
*The resulting binary is generated at `target/release/arch-explorer`.*

---

## ✨ Key Features & Architecture

- **⚡ Blazing Fast & Ultra Lightweight:** Consumes only ~40–50 MB RAM with instantaneous startup (<50ms). Pure hardware GPU rendering via `wgpu`.
- **🎯 Full Native Wayland Drag & Drop (`wl_data_device`):**
  - Outgoing drag attaches a 64×64 neon-bordered thumbnail/icon surface directly to the Wayland cursor.
  - Seamless bidirectional drag-and-drop between **Arch Explorer, Nemo, Nautilus, Firefox, Discord, and workspaces**.
  - **Self-drop isolation:** Dropping files back into the same application window never triggers redundant file transfers or clipboard pollution.
  - **System Clipboard Preservation:** Drag & drop operations never tamper with your <kbd>Ctrl+C</kbd> / <kbd>Ctrl+X</kbd> clipboard.
- **🪟 True Desktop Transparency & Glassmorphism:**
  - Real alpha-channel compositor pass-through.
  - Full compatibility with Hyprland & Sway blur shaders.
- **📁 Dual-Pane & Multi-Tab Workspace:**
  - Tab caching prevents redundant re-scans and stops tab hopping.
  - Split-view (dual-pane) mode with visual parity across active and inactive panels.
- **💻 Integrated Terminal Drawer:**
  - Toggleable embedded terminal synced with current working directory.
- **👁 Rich File Preview Panel:**
  - Dynamic responsive scroll area for code with syntax highlighting, hex dump, image viewing, and video playback.
- **📱 Android ADB Device Explorer:**
  - Inspect, transfer, and manage files on connected Android devices over USB or Wi-Fi.
- **🗑 Full Trash Management & Ctrl+Z Undo:**
  - Freedesktop trash compliance with instant undo support.

---

## 📸 Visual Showcase

<p align="center">
  <img src="docs/screenshots/arch_explorer_transparent_demo.gif" alt="Arch Explorer Desktop Flow Demo" width="90%">
</p>

### 1. Main Transparent Workspace & Dual Pane
Arch Explorer blends with your desktop wallpaper and system themes with full hardware-accelerated transparency.

<p align="center">
  <img src="docs/screenshots/01_arch_explorer_main_transparent.png" alt="Main Window Transparency" width="88%">
</p>

### 2. Live Appearance & Theme Settings
Customize accent colors, window blur (0–40px), card transparency, and UI scaling on the fly.

<p align="center">
  <img src="docs/screenshots/02_settings_appearance_transparent.png" alt="Settings & Glass Blur" width="88%">
</p>

### 3. Folder Previews & Dynamic Layout
Interactive preview modes (Always, Automatic, Manual, Custom) with instant metadata inspection.

<p align="center">
  <img src="docs/screenshots/04_settings_layout_folder_preview.png" alt="Layout & Folder Preview Controls" width="88%">
</p>

### 4. Transparent Context Menu & Quick Actions
Context menus featuring fast Open With recommendations, Archive operations, and Trash management.

<p align="center">
  <img src="docs/screenshots/05_context_menu_transparent.png" alt="Transparent Context Menu" width="88%">
</p>

---

## 🪟 Hyprland Blur & Transparency Configuration

Add the following window rules to your `~/.config/hypr/hyprland.conf` for blur and rounding:

```ini
windowrulev2 = opacity 0.88 0.82,       class:^(arch-explorer)$
windowrulev2 = rounding 12,             class:^(arch-explorer)$
windowrulev2 = noshadow,                class:^(arch-explorer)$
windowrulev2 = animation popin 80%,     class:^(arch-explorer)$
```

---

## 🤝 Upstream Contribution
Our custom Wayland data device backport patch implemented for `winit 0.30.13` is documented in [`patches/winit-0.30.13-wayland-dnd.patch`](patches/winit-0.30.13-wayland-dnd.patch) for upstream `rust-windowing/winit` reference.

---

## 📄 License
Licensed under the [MIT License](LICENSE).
