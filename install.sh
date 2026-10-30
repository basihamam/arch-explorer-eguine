#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Arch Explorer v2 — egui rewrite setup script
# Çalıştır: chmod +x install.sh && ./install.sh
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

# ── Renkler ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

ok()   { echo -e "${GREEN}  ✓${RESET}  $1"; }
info() { echo -e "${CYAN}  →${RESET}  $1"; }
warn() { echo -e "${YELLOW}  !${RESET}  $1"; }
die()  { echo -e "${RED}  ✗${RESET}  $1"; exit 1; }
step() { echo -e "\n${BOLD}${CYAN}━━  $1${RESET}"; }

# ── Banner ───────────────────────────────────────────────────────────────────
echo -e "${BOLD}"
cat << 'EOF'
   ___              _       ___          _
  / _ |  ________  / /     / __| __ __ | |  ___   _ _  ___  _ _
 / __ | / __/ __/ / _ \   | _|  \ \ / |_| / _ \ | '_|/ -_)| '_|
/_/ |_|/_/  \__/ /_//_/   |___| /_\_\      \___/ |_|  \___||_|
                                      v2 — egui native, AEX theming
EOF
echo -e "${RESET}"
echo -e "${DIM}  webkit'siz, ~50mb RAM, tam özelleştirilebilir${RESET}\n"

# ── Proje dizini ─────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="${1:-$SCRIPT_DIR}"

info "Proje dizini: ${BOLD}$PROJECT_DIR${RESET}"

cd "$PROJECT_DIR"

# Cargo.toml yoksa büyük ihtimalle yanlış dizindeyiz
if [[ ! -f "Cargo.toml" && ! -f "Cargo_new.toml" ]]; then
    warn "Cargo.toml bulunamadı — yeni proje dizini olarak devam ediliyor"
fi

# ── Bağımlılık kontrolleri ────────────────────────────────────────────────────
step "Bağımlılıklar kontrol ediliyor"

check_cmd() {
    if command -v "$1" &>/dev/null; then
        ok "$1 mevcut $(${2:-} 2>/dev/null | head -1 || true)"
    else
        die "$1 bulunamadı — kurun: $3"
    fi
}

check_cmd "rustc"   "rustc --version"    "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
check_cmd "cargo"   "cargo --version"    "rustup ile gelir"
check_cmd "git"     "git --version"      "sudo pacman -S git"

# egui için Wayland build deps
if ! pkg-config --exists wayland-client 2>/dev/null; then
    warn "wayland-client dev headers eksik"
    warn "Kur: sudo pacman -S wayland wayland-protocols libxkbcommon"
fi

if ! pkg-config --exists libxkbcommon 2>/dev/null; then
    warn "libxkbcommon eksik: sudo pacman -S libxkbcommon"
fi

# xdg-mime (open_with.rs için)
command -v xdg-mime &>/dev/null && ok "xdg-mime mevcut" || warn "xdg-mime eksik: sudo pacman -S xdg-utils"

# gtk-launch (open_with.rs için)
command -v gtk-launch &>/dev/null && ok "gtk-launch mevcut" || warn "gtk-launch eksik: sudo pacman -S gtk3"

# ── Dizin yapısı ─────────────────────────────────────────────────────────────
step "Dizin yapısı oluşturuluyor"

dirs=(
    "src/theme"
    "src/state"
    "src/ui"
    "src/backend"
    "themes"
    "assets/fonts"
    "assets/icons"
)

for d in "${dirs[@]}"; do
    mkdir -p "$d"
    ok "src/$d" 2>/dev/null || ok "$d"
done

# ── Dosya yerleştirme (eğer output dosyaları varsa) ──────────────────────────
step "Kaynak dosyalar yerleştiriliyor"

place_file() {
    local src="$1"
    local dst="$2"
    if [[ -f "$src" ]]; then
        cp "$src" "$dst"
        ok "$(basename $src) → $dst"
    else
        warn "Kaynak bulunamadı: $src  (manuel kopyalama gerekli)"
    fi
}

# Theme engine
place_file "theme_types.rs"   "src/theme/types.rs"
place_file "theme_mod.rs"     "src/theme/mod.rs"

# State
place_file "state_mod.rs"     "src/state/mod.rs"

# Main + App
if [[ -f "main_app.rs" ]]; then
    # main.rs ve app.rs ayrı dosyalara bölünüyor
    # (main_app.rs ikisini birlikte içeriyor, bölüyoruz)
    info "main_app.rs → src/main.rs + src/app.rs bölünüyor"

    # main.rs: ilk "// src/app.rs" satırına kadar
    awk '/^\/\/ src\/app\.rs/{exit} {print}' main_app.rs > src/main.rs

    # app.rs: "// src/app.rs" satırından sonrası
    awk '/^\/\/ src\/app\.rs/{found=1; next} found{print}' main_app.rs > src/app.rs

    ok "src/main.rs"
    ok "src/app.rs"
fi

# UI modülleri
place_file "ui_mod.rs"                "src/ui/mod.rs"
place_file "ui_titlebar_toolbar.rs"   "src/ui/_combined_titlebar_toolbar.rs"  # bölünecek
place_file "ui_sidebar.rs"            "src/ui/sidebar.rs"
place_file "ui_file_grid.rs"          "src/ui/file_grid.rs"
place_file "ui_ctx_status_notif.rs"   "src/ui/_combined_ctx_status_notif.rs"  # bölünecek

# Combined dosyaları bölme
split_combined_ui() {
    local combined="$1"
    local sep_comment="$2"    # örn: "// src/ui/toolbar.rs"
    local file_a="$3"
    local file_b="$4"

    if [[ -f "$combined" ]]; then
        awk "/^${sep_comment//\//\\/}/{exit} {print}" "$combined" > "$file_a"
        awk "/^${sep_comment//\//\\/}/{found=1; next} found{print}" "$combined" > "$file_b"
        ok "$(basename $file_a)"
        ok "$(basename $file_b)"
        rm "$combined"
    fi
}

split_combined_ui \
    "src/ui/_combined_titlebar_toolbar.rs" \
    "// src/ui/toolbar.rs" \
    "src/ui/titlebar.rs" \
    "src/ui/toolbar.rs"

split_combined_ui \
    "src/ui/_combined_ctx_status_notif.rs" \
    "// src/ui/statusbar.rs" \
    "src/ui/context_menu.rs" \
    "src/ui/_rest_status_notif.rs"

if [[ -f "src/ui/_rest_status_notif.rs" ]]; then
    split_combined_ui \
        "src/ui/_rest_status_notif.rs" \
        "// src/ui/notifications.rs" \
        "src/ui/statusbar.rs" \
        "src/ui/notifications.rs"
fi

# Backend dosyaları — eski projeden kopyala (varsa)
step "Backend dosyaları kopyalanıyor (src-tauri/src → src/backend)"

backend_files=(
    "fs_ops.rs"
    "mounts.rs"
    "clipboard.rs"
    "commands.rs"
    "watcher.rs"
    "preview.rs"
)

OLD_BACKEND=""
for candidate in "src-tauri/src" "../arch-explorer/src-tauri/src" "src"; do
    if [[ -f "$candidate/mounts.rs" ]]; then
        OLD_BACKEND="$candidate"
        break
    fi
done

if [[ -n "$OLD_BACKEND" ]]; then
    info "Eski backend bulundu: $OLD_BACKEND"
    for f in "${backend_files[@]}"; do
        if [[ -f "$OLD_BACKEND/$f" ]]; then
            cp "$OLD_BACKEND/$f" "src/backend/$f"
            ok "src/backend/$f"
        fi
    done
    # open_with.rs yeni dosya
    [[ -f "open_with.rs" ]] && cp "open_with.rs" "src/backend/open_with.rs" && ok "src/backend/open_with.rs"
else
    warn "Eski backend bulunamadı — src/backend/ dosyalarını manuel kopyala"
    warn "Kaynak: <eski_proje>/src-tauri/src/{fs_ops,mounts,clipboard,commands,watcher,preview}.rs"
fi

# Backend mod.rs oluştur
cat > src/backend/mod.rs << 'BACKENDMOD'
// src/backend/mod.rs
// Tüm backend modüller — #[tauri::command] kaldırıldı, doğrudan çağrılıyor

pub mod fs_ops;
pub mod mounts;
pub mod clipboard;
pub mod commands;
pub mod watcher;
pub mod preview;
pub mod open_with;
BACKENDMOD
ok "src/backend/mod.rs"

# Placeholder UI dosyaları (henüz yazılmadı)
step "Placeholder UI dosyaları oluşturuluyor"

create_placeholder() {
    local file="$1"
    local module="$2"
    if [[ ! -f "$file" ]]; then
        cat > "$file" << PLACEHOLDER
// $file — TODO: Session 3'te yazılacak
use egui::*;
use crate::theme::types::AexTheme;
use crate::state::AppState;

pub fn render(_ctx: &Context, _t: &AexTheme, _state: &mut AppState) {
    // placeholder
}
PLACEHOLDER
        ok "$file (placeholder)"
    fi
}

create_placeholder "src/ui/mount_panel.rs"   "mount_panel"
create_placeholder "src/ui/open_with.rs"     "open_with"
create_placeholder "src/ui/settings.rs"      "settings"
create_placeholder "src/ui/preview_panel.rs" "preview_panel"

# ── Cargo.toml güncelle ───────────────────────────────────────────────────────
step "Cargo.toml güncelleniyor"

if [[ -f "Cargo_new.toml" ]]; then
    if [[ -f "Cargo.toml" ]]; then
        cp Cargo.toml Cargo.toml.bak
        ok "Cargo.toml.bak yedeği oluşturuldu"
    fi
    cp Cargo_new.toml Cargo.toml
    ok "Cargo.toml güncellendi (egui/eframe)"
else
    warn "Cargo_new.toml bulunamadı — Cargo.toml manuel güncellenmeli"
fi

# ── .aex Tema dosyaları ───────────────────────────────────────────────────────
step "Built-in .aex tema dosyaları oluşturuluyor"

write_theme() {
    local file="themes/$1.aex"
    cat > "$file"
    ok "$file"
}

write_theme "default" << 'AEX'
[meta]
name        = "Arch Explorer Dark"
author      = "lordblue"
version     = "1.0.0"
description = "Default dark theme"

[colors]
bg_base      = "#0f0f17"
bg_mantle    = "#13131e"
bg_crust     = "#0a0a10"
bg_surface0  = "#1e1e2e"
bg_surface1  = "#2a2a3e"
bg_surface2  = "#3a3a52"
accent       = "#7c6af7"
accent_dim   = "#5a4cc9"
accent_glow  = "#7c6af730"
text_primary   = "#e0e0f0"
text_secondary = "#9090b0"
text_hint      = "#60607a"
text_disabled  = "#3a3a52"
success = "#6af7b0"
warning = "#f7c96a"
danger  = "#f76a8c"
info    = "#6ac8f7"
border       = "#2a2a3e"
border_focus = "#7c6af7"

[effects]
window_opacity = 0.92
blur_radius    = 12
glass_panels   = true
shadow_enabled = true

[icons]
style   = "nerd"
AEX

write_theme "catppuccin-mocha" << 'AEX'
[meta]
name        = "Catppuccin Mocha"
author      = "catppuccin"
version     = "1.0.0"
description = "Soothing pastel theme"

[colors]
bg_base      = "#1e1e2e"
bg_mantle    = "#181825"
bg_crust     = "#11111b"
bg_surface0  = "#313244"
bg_surface1  = "#45475a"
bg_surface2  = "#585b70"
accent       = "#cba6f7"
accent_dim   = "#9874c9"
accent_glow  = "#cba6f730"
text_primary   = "#cdd6f4"
text_secondary = "#bac2de"
text_hint      = "#6c7086"
text_disabled  = "#45475a"
success = "#a6e3a1"
warning = "#f9e2af"
danger  = "#f38ba8"
info    = "#89dceb"
border       = "#45475a"
border_focus = "#cba6f7"
AEX

write_theme "nord" << 'AEX'
[meta]
name        = "Nord"
author      = "arcticicestudio"
version     = "1.0.0"
description = "An arctic, north-bluish color palette"

[colors]
bg_base      = "#2e3440"
bg_mantle    = "#242933"
bg_crust     = "#1c2028"
bg_surface0  = "#3b4252"
bg_surface1  = "#434c5e"
bg_surface2  = "#4c566a"
accent       = "#88c0d0"
accent_dim   = "#5e97ad"
accent_glow  = "#88c0d030"
text_primary   = "#eceff4"
text_secondary = "#d8dee9"
text_hint      = "#616e88"
text_disabled  = "#434c5e"
success = "#a3be8c"
warning = "#ebcb8b"
danger  = "#bf616a"
info    = "#81a1c1"
border       = "#3b4252"
border_focus = "#88c0d0"
AEX

write_theme "gruvbox" << 'AEX'
[meta]
name        = "Gruvbox Dark"
author      = "morhetz"
version     = "1.0.0"
description = "Retro groove color scheme"

[colors]
bg_base      = "#1d2021"
bg_mantle    = "#282828"
bg_crust     = "#141617"
bg_surface0  = "#3c3836"
bg_surface1  = "#504945"
bg_surface2  = "#665c54"
accent       = "#d79921"
accent_dim   = "#b57614"
accent_glow  = "#d7992130"
text_primary   = "#ebdbb2"
text_secondary = "#d5c4a1"
text_hint      = "#928374"
text_disabled  = "#504945"
success = "#b8bb26"
warning = "#fabd2f"
danger  = "#fb4934"
info    = "#83a598"
border       = "#3c3836"
border_focus = "#d79921"
AEX

write_theme "rose-pine" << 'AEX'
[meta]
name        = "Rosé Pine"
author      = "rose-pine"
version     = "1.0.0"
description = "All natural pine, faux fur and a bit of soho vibes"

[colors]
bg_base      = "#191724"
bg_mantle    = "#1f1d2e"
bg_crust     = "#26233a"
bg_surface0  = "#2a2837"
bg_surface1  = "#403d52"
bg_surface2  = "#524f67"
accent       = "#c4a7e7"
accent_dim   = "#9678bf"
accent_glow  = "#c4a7e730"
text_primary   = "#e0def4"
text_secondary = "#908caa"
text_hint      = "#6e6a86"
text_disabled  = "#403d52"
success = "#31748f"
warning = "#f6c177"
danger  = "#eb6f92"
info    = "#9ccfd8"
border       = "#403d52"
border_focus = "#c4a7e7"
AEX

# ── Hyprland window rules ──────────────────────────────────────────────────────
step "Hyprland config snippet oluşturuluyor"

cat > "hyprland_rules.conf" << 'HYPR'
# Arch Explorer — Hyprland window rules
# ~/.config/hypr/hyprland.conf içine ekle (source ile veya direk yapıştır)

windowrulev2 = opacity 0.92 0.88,       class:^(arch-explorer)$
windowrulev2 = blur,                     class:^(arch-explorer)$
windowrulev2 = rounding 12,             class:^(arch-explorer)$
windowrulev2 = noshadow,                class:^(arch-explorer)$
windowrulev2 = animation popin 80%,     class:^(arch-explorer)$

# Blur parametreleri (decoration:blur bloğuna ekle):
# blur {
#     enabled = true
#     size = 12
#     passes = 3
#     new_optimizations = true
#     xray = false
# }
HYPR
ok "hyprland_rules.conf"

# ── .gitignore güncelle ────────────────────────────────────────────────────────
if [[ -f ".gitignore" ]]; then
    if ! grep -q "Cargo.toml.bak" .gitignore 2>/dev/null; then
        echo -e "\n# egui rewrite backups\nCargo.toml.bak\nCargo_new.toml\n*.bak" >> .gitignore
        ok ".gitignore güncellendi"
    fi
fi

# ── Özet ─────────────────────────────────────────────────────────────────────
echo -e "\n${BOLD}${GREEN}━━  Kurulum tamamlandı  ━━${RESET}\n"

echo -e "${DIM}Oluşturulan yapı:${RESET}"
tree -L 3 src/ themes/ 2>/dev/null || find src themes -type f | sort | sed 's/^/  /'

echo -e "\n${BOLD}Sonraki adımlar:${RESET}"
echo -e "  ${CYAN}1.${RESET} Backend'deki ${BOLD}#[tauri::command]${RESET} makrolarını kaldır:"
echo -e "     ${DIM}sed -i '/#\[tauri::command\]/d' src/backend/*.rs${RESET}"
echo ""
echo -e "  ${CYAN}2.${RESET} Tauri import'larını temizle:"
echo -e "     ${DIM}sed -i '/use tauri::/d' src/backend/*.rs${RESET}"
echo ""
echo -e "  ${CYAN}3.${RESET} Test derleme:"
echo -e "     ${DIM}cargo check 2>&1 | head -50${RESET}"
echo ""
echo -e "  ${CYAN}4.${RESET} Çalıştır:"
echo -e "     ${DIM}cargo run${RESET}"
echo ""
echo -e "  ${CYAN}5.${RESET} Hyprland kuralları:"
echo -e "     ${DIM}cat hyprland_rules.conf >> ~/.config/hypr/hyprland.conf${RESET}"
echo ""
echo -e "${DIM}Temalar: themes/*.aex  —  kendi temanı ekle, uygulama canlı yükler${RESET}"
echo -e "${DIM}Session 3: mount_panel, open_with, settings, preview yazılacak${RESET}\n"
