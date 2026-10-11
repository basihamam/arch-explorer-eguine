// ─────────────────────────────────────────────────────────────────────────────
// src/app.rs
// ─────────────────────────────────────────────────────────────────────────────

use eframe::egui::{self, Color32};
use std::sync::mpsc::{channel, Receiver, Sender};

use crate::backend::fs_ops::{get_bookmarks, get_drives, list_directory};
use crate::backend::mounts::list_block_devices;
use crate::state::AppState;
use crate::theme::ThemeEngine;

// Async task'lardan gelen mesajlar
pub enum BackendMsg {
    DirectoryLoaded(crate::backend::fs_ops::DirectoryListing),
    DirectoryError(String),
    DirectoryPermissionDenied(String),
    BookmarksLoaded(Vec<crate::backend::fs_ops::Bookmark>),
    DrivesLoaded(Vec<crate::backend::fs_ops::DriveInfo>),
    BlockDevicesLoaded(Vec<crate::backend::mounts::BlockDevice>),
    AdbDevicesLoaded(Vec<crate::backend::adb::AdbDevice>),
    // Mount paneli mesajları
    MountDone {
        success: bool,
        message: String,
        mount_point: String,
    },
    UnmountDone {
        success: bool,
        message: String,
        target: String,
    },
    MountNeedsPassword {
        device: String,
        label: String,
        error: String,
    },
    MountWrongPassword {
        device: String,
        label: String,
    },
    TerminalOutput {
        lines: Vec<crate::state::TerminalLine>,
        sync_nav: Option<String>,
    },
    // inotify — dizin değişti, reload tetikle
    DirectoryChanged,
    // Checksum sonucu
    ChecksumDone { md5: String, sha256: String, path: String },
    // Du cache — klasör boyutu hesaplandı
    DuSizeDone { path: String, size: u64 },
    // Recursive search sonucu
    SearchDone(Vec<crate::backend::fs_ops::FileEntry>),
    // Undo sonucu
    UndoDone { message: String, success: bool, restored_paths: Vec<String> },
    // Archive işlemi
    ArchiveDone { success: bool, message: String },
    // Root / Polkit / Sudo yetkilendirme
    RootAuthDone { success: bool, message: String, reload: bool },
    RootAuthWrongPassword,
    // Async dosya önizleme
    PreviewLoaded {
        path: String,
        result: Result<crate::backend::preview::FilePreview, String>,
    },
    // Git repo durumu
    GitStatusLoaded(crate::backend::git::GitStatus),
    // Single-instance IPC: Dışarıdan gelen yeni pencere / sekme yolu
    IpcOpenPath(String),
    // Async dosya aktarımı tamamlandı
    TransferDone {
        success: bool,
        message: String,
        target_dir: String,
        selected_paths: Vec<String>,
    },
    // Async dosya aktarım ilerleme durumu (id, mesaj, 0.0..=1.0)
    TransferProgress {
        id: String,
        message: String,
        progress: f32,
    },
}

pub struct ArchExplorer {
    pub theme: ThemeEngine,
    pub state: AppState,

    tx: Sender<BackendMsg>,
    rx: Receiver<BackendMsg>,
    runtime: tokio::runtime::Runtime,

    // inotify watcher — drop edilirse izleme durur
    _watcher: Option<notify::RecommendedWatcher>,
    // Son izlenen dizin
    watched_path: String,
    // Periyodik sidebar (disk & adb) taraması
    last_sidebar_refresh: std::time::Instant,
}

impl ArchExplorer {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        // Tema motoru başlat
        let theme = ThemeEngine::init();

        // egui_extras image loaders — SVG, gif, webp vs. için şart
        egui_extras::install_image_loaders(&cc.egui_ctx);

        // Font yükleme
        let font_ok = crate::font_loader::load_fonts(&cc.egui_ctx);
        if !font_ok {
            log::warn!("Nerd Font yüklenemedi — ikonlar eksik görünecek");
            log::warn!("Kur: sudo pacman -S ttf-jetbrains-mono-nerd");
        }

        // egui style'ı hemen uygula
        cc.egui_ctx
            .set_style(crate::theme::egui_bridge::aex_to_egui_style(&theme.active));

        // Async channel
        let (tx, rx) = channel::<BackendMsg>();

        // Tokio runtime (mounts.rs için)
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Tokio runtime başlatılamadı");

        let mut app = Self {
            theme,
            state: AppState::new(),
            tx, rx, runtime,
            _watcher: None,
            watched_path: String::new(),
            last_sidebar_refresh: std::time::Instant::now(),
        };

        // İlk yükleme
        if app.state.always_root {
            app.state.pending_root_auth = Some(crate::state::RootAuthRequest {
                title: "Her zaman root olarak aç etkin — lütfen şifrenizi girin".to_string(),
                cmd: "true".to_string(),
                args: vec![],
                reload: true,
                auth_type: Some("become_root".to_string()),
            });
            app.state.pending_root_wrong = false;
        }

        // Single-instance IPC Listener: Başka süreçlerden gelen açma komutlarını karşıla
        let ipc_tx = app.tx.clone();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            use std::os::unix::net::UnixListener;

            let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| dirs::runtime_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp")));
            let sock_path = runtime_dir.join("arch-explorer-ipc.sock");

            let listener = match UnixListener::bind(&sock_path) {
                Ok(l) => Some(l),
                Err(_) => {
                    if std::os::unix::net::UnixStream::connect(&sock_path).is_err() {
                        let _ = std::fs::remove_file(&sock_path);
                        UnixListener::bind(&sock_path).ok()
                    } else {
                        None
                    }
                }
            };

            if let Some(listener) = listener {
                for stream in listener.incoming().flatten() {
                    let mut reader = BufReader::new(stream);
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_ok() {
                        let path = line.trim().to_string();
                        if !path.is_empty() {
                            ipc_tx.send(BackendMsg::IpcOpenPath(path)).ok();
                        }
                    }
                }
            }
        });

        app.load_current_directory();
        app.load_sidebar_data();

        app
    }

    // ── Backend Tasks ────────────────────────────────────────────────────────

    pub fn load_current_directory(&mut self) {
        let path = self.state.nav.current_path.clone();
        let hidden = self.state.dir.show_hidden;
        let tx = self.tx.clone();

        self.state.dir.is_loading = true;
        self.state.needs_reload = false;

        let git_path = path.clone();
        let git_tx = self.tx.clone();
        std::thread::spawn(move || {
            let status = crate::backend::git::get_git_status_sync(&git_path);
            git_tx.send(BackendMsg::GitStatusLoaded(status)).ok();
        });

        std::thread::spawn(move || {
            if path.starts_with("adb://") {
                if let Some((serial, device_path)) = crate::backend::adb::parse_adb_path(&path) {
                    match crate::backend::adb::list_adb_directory(&serial, &device_path, hidden) {
                        Ok(listing) => {
                            tx.send(BackendMsg::DirectoryLoaded(listing)).ok();
                        }
                        Err(e) => {
                            tx.send(BackendMsg::DirectoryError(e)).ok();
                        }
                    }
                    return;
                }
            }

            // Önce normal dene
            match list_directory(path.clone(), hidden) {
                Ok(listing) => {
                    tx.send(BackendMsg::DirectoryLoaded(listing)).ok();
                }
                Err(e) => {
                    let lower = e.to_lowercase();
                    let is_perm = lower.contains("permission denied")
                        || lower.contains("erişim engellendi")
                        || lower.contains("yetki")
                        || lower.contains("os { code: 13");

                    if is_perm {
                        // Eğer önceden girilmiş ve geçerli bir root şifresi varsa doğrudan dene (alt klasörlere kesintisiz erişim)
                        if let Some(pw) = crate::backend::fs_ops::get_root_password() {
                            match crate::backend::fs_ops::list_directory_with_sudo(path.clone(), hidden, &pw) {
                                Ok(listing) => {
                                    tx.send(BackendMsg::DirectoryLoaded(listing)).ok();
                                    return;
                                }
                                Err(err) if err == "WRONG_PASSWORD" => {
                                    // Şifre geçersizleşmiş
                                    crate::backend::fs_ops::clear_root_password();
                                }
                                Err(_) => {}
                            }
                        }

                        // Henüz şifre girilmedi veya yanlış: Kullanıcıdan Root şifresi iste
                        tx.send(BackendMsg::DirectoryPermissionDenied(path)).ok();
                    } else {
                        tx.send(BackendMsg::DirectoryError(e)).ok();
                    }
                }
            }
        });
    }

    // ── inotify watcher ──────────────────────────────────────────────────────
    // Dizin değişince 400ms sonra otomatik reload

    fn start_watcher(&mut self, path: &str) {
        use crate::backend::watcher::watch_directory;

        if self.watched_path == path { return; } // Zaten izleniyor
        self.watched_path = path.to_string();

        let tx = self.tx.clone();
        // mpsc bridge — notify → BackendMsg
        let (wtx, wrx) = std::sync::mpsc::channel::<String>();

        let watcher = watch_directory(path, wtx).ok();
        self._watcher = watcher;

        // Debounce thread: 400ms beklendikten sonra DirectoryChanged gönder
        std::thread::spawn(move || {
            let mut last = std::time::Instant::now();
            loop {
                match wrx.recv_timeout(std::time::Duration::from_millis(50)) {
                    Ok(_) => { last = std::time::Instant::now(); }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if last.elapsed() > std::time::Duration::from_millis(400) {
                            tx.send(BackendMsg::DirectoryChanged).ok();
                            // Bir sonraki değişikliği bekle
                            last = std::time::Instant::now()
                                + std::time::Duration::from_secs(9999);
                        }
                    }
                    Err(_) => break, // channel kapandı
                }
            }
        });
    }

    // ── Du cache ─────────────────────────────────────────────────────────────

    pub fn request_du_size(&self, path: String) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            // Önce cache'den dene
            if let Some(size) = crate::backend::du_cache::get_cached_size(&path) {
                tx.send(BackendMsg::DuSizeDone { path, size }).ok();
                return;
            }
            // Cache miss — hesapla ve cache'e yaz
            let size = crate::backend::du_cache::compute_and_cache(path.clone());
            tx.send(BackendMsg::DuSizeDone { path, size }).ok();
        });
    }

    // ── Checksum ─────────────────────────────────────────────────────────────

    pub fn request_checksum(&self, path: String) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let (md5, sha256) = crate::backend::checksum::compute_checksums(&path);
            tx.send(BackendMsg::ChecksumDone { md5, sha256, path }).ok();
        });
    }

    // ── Archive ops ──────────────────────────────────────────────────────────

    pub fn extract_archive(&self, archive_path: String, dest_dir: String) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let ext = std::path::Path::new(&archive_path)
                .extension().map(|e| e.to_str().unwrap_or("").to_lowercase())
                .unwrap_or_default();
            let name = std::path::Path::new(&archive_path)
                .file_stem().map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "extracted".into());
            // .tar.gz gibi double extension
            let name = if name.ends_with(".tar") { name[..name.len()-4].to_string() } else { name };

            // Hedef klasör — çakışma varsa numarala
            let mut out_dir = format!("{}/{}", dest_dir, name);
            let mut i = 2;
            while std::path::Path::new(&out_dir).exists() {
                out_dir = format!("{}/{} ({})", dest_dir, name, i);
                i += 1;
            }
            std::fs::create_dir_all(&out_dir).ok();

            let result = match ext.as_str() {
                "zip" => std::process::Command::new("unzip")
                    .args(["-q", &archive_path, "-d", &out_dir]).status(),
                "gz" | "tgz" | "bz2" | "xz" | "zst" | "tar" =>
                    std::process::Command::new("tar")
                        .args(["xf", &archive_path, "-C", &out_dir]).status(),
                "7z" | "rar" => std::process::Command::new("7z")
                    .args(["x", &archive_path, &format!("-o{}", out_dir), "-y"]).status(),
                _ => std::process::Command::new("7z")
                    .args(["x", &archive_path, &format!("-o{}", out_dir), "-y"]).status(),
            };
            match result {
                Ok(s) if s.success() =>
                    tx.send(BackendMsg::ArchiveDone { success: true,
                        message: format!("Çıkarıldı: {}", out_dir) }).ok(),
                Ok(s) =>
                    tx.send(BackendMsg::ArchiveDone { success: false,
                        message: format!("Çıkarma başarısız (exit {})", s.code().unwrap_or(-1)) }).ok(),
                Err(e) =>
                    tx.send(BackendMsg::ArchiveDone { success: false,
                        message: format!("Komut bulunamadı: {}", e) }).ok(),
            };
        });
    }

    pub fn compress_files(&self, paths: Vec<String>, dest: String, format: &str) {
        let tx = self.tx.clone();
        let format = format.to_string();
        std::thread::spawn(move || {
            let files: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
            let result = match format.as_str() {
                "zip" => std::process::Command::new("zip")
                    .arg("-r").arg(&dest).args(&files).status(),
                "tar.gz" => std::process::Command::new("tar")
                    .arg("czf").arg(&dest).args(&files).status(),
                "tar.bz2" => std::process::Command::new("tar")
                    .arg("cjf").arg(&dest).args(&files).status(),
                "tar.xz" => std::process::Command::new("tar")
                    .arg("cJf").arg(&dest).args(&files).status(),
                "7z" => std::process::Command::new("7z")
                    .arg("a").arg(&dest).args(&files).status(),
                _ => std::process::Command::new("tar")
                    .arg("czf").arg(&dest).args(&files).status(),
            };
            match result {
                Ok(s) if s.success() =>
                    tx.send(BackendMsg::ArchiveDone { success: true,
                        message: format!("Oluşturuldu: {}", dest) }).ok(),
                Ok(_) =>
                    tx.send(BackendMsg::ArchiveDone { success: false,
                        message: "Sıkıştırma başarısız".into() }).ok(),
                Err(e) =>
                    tx.send(BackendMsg::ArchiveDone { success: false,
                        message: format!("Komut bulunamadı: {}", e) }).ok(),
            };
        });
    }

    pub fn execute_async_transfer(&self, req: crate::state::TransferRequest) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let total_items = req.sources.len().max(1);
            let dest_is_adb = req.dest_dir.starts_with("adb://");
            let any_src_adb = req.sources.iter().any(|s| s.starts_with("adb://"));

            tx.send(BackendMsg::TransferProgress {
                id: "file_transfer".into(),
                message: format!("Aktarım başlıyor… (0/{})", total_items),
                progress: 0.05,
            }).ok();

            if dest_is_adb || any_src_adb {
                let mut transferred = 0;
                let mut err_msg = String::new();
                let mut created_paths = Vec::new();

                for (idx, src) in req.sources.iter().enumerate() {
                    let src_is_adb = src.starts_with("adb://");

                    let current_idx = idx + 1;
                    let prog = (current_idx as f32 / total_items as f32).clamp(0.05, 0.98);
                    let fname = std::path::Path::new(src)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "öğe".to_string());

                    tx.send(BackendMsg::TransferProgress {
                        id: "file_transfer".into(),
                        message: format!("{}/{} aktarılıyor: {}", current_idx, total_items, fname),
                        progress: prog,
                    }).ok();

                    if !src_is_adb && dest_is_adb {
                        // Local -> ADB (adb push)
                        if let Some((serial, dev_dir)) = crate::backend::adb::parse_adb_path(&req.dest_dir) {
                            match crate::backend::adb::adb_push(&serial, src, &dev_dir) {
                                Ok(_) => {
                                    transferred += 1;
                                    if !req.copy {
                                        let _ = std::fs::remove_file(src).or_else(|_| std::fs::remove_dir_all(src));
                                    }
                                    let full_dev = format!("{}/{}", dev_dir.trim_end_matches('/'), fname);
                                    created_paths.push(crate::backend::adb::make_adb_path(&serial, &full_dev));
                                }
                                Err(e) => {
                                    err_msg = e;
                                }
                            }
                        }
                    } else if src_is_adb && !dest_is_adb {
                        // ADB -> Local (adb pull)
                        if let Some((serial, dev_path)) = crate::backend::adb::parse_adb_path(src) {
                            match crate::backend::adb::adb_pull(&serial, &dev_path, &req.dest_dir) {
                                Ok(_) => {
                                    transferred += 1;
                                    if !req.copy {
                                        let _ = crate::backend::adb::adb_delete(&serial, &dev_path);
                                    }
                                    let local_target = format!("{}/{}", req.dest_dir.trim_end_matches('/'), fname);
                                    created_paths.push(local_target);
                                }
                                Err(e) => {
                                    err_msg = e;
                                }
                            }
                        }
                    } else if src_is_adb && dest_is_adb {
                        // ADB -> ADB (telefon içi kopyalama veya taşıma)
                        if let (Some((src_serial, src_dev_p)), Some((dst_serial, dst_dev_p))) = (
                            crate::backend::adb::parse_adb_path(src),
                            crate::backend::adb::parse_adb_path(&req.dest_dir),
                        ) {
                            if src_serial == dst_serial {
                                let full_dst = format!("{}/{}", dst_dev_p.trim_end_matches('/'), fname);
                                let res = if req.copy {
                                    let esc_src = src_dev_p.replace('\'', "'\\''");
                                    let esc_dst = full_dst.replace('\'', "'\\''");
                                    let cmd_str = format!("cp -r -a '{}' '{}' 2>/dev/null || cp -r '{}' '{}'", esc_src, esc_dst, esc_src, esc_dst);
                                    let out = std::process::Command::new("adb")
                                        .args(["-s", &src_serial, "shell", &cmd_str])
                                        .output();
                                    match out {
                                        Ok(o) if o.status.success() => Ok(()),
                                        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
                                        Err(e) => Err(e.to_string()),
                                    }
                                } else {
                                    crate::backend::adb::adb_rename(&src_serial, &src_dev_p, &full_dst)
                                };
                                match res {
                                    Ok(_) => {
                                        transferred += 1;
                                        created_paths.push(crate::backend::adb::make_adb_path(&src_serial, &full_dst));
                                    }
                                    Err(e) => {
                                        err_msg = e;
                                    }
                                }
                            }
                        }
                    }
                }

                if transferred > 0 {
                    let msg = format!("{} öğe başarıyla {}", transferred, if req.copy { "kopyalandı" } else { "aktarıldı" });
                    tx.send(BackendMsg::TransferDone {
                        success: true,
                        message: msg,
                        target_dir: req.dest_dir,
                        selected_paths: created_paths,
                    }).ok();
                } else {
                    let msg = if err_msg.is_empty() { "Aktarım başarısız".to_string() } else { format!("Aktarım hatası: {}", err_msg) };
                    tx.send(BackendMsg::TransferDone {
                        success: false,
                        message: msg,
                        target_dir: req.dest_dir,
                        selected_paths: vec![],
                    }).ok();
                }
                return;
            }

            // Yerel dosya aktarımı — arka planda yüzdeli bildirim ile
            let mut undos = Vec::new();
            let mut created_paths = Vec::new();
            let mut direct_count = 0;
            let mut err_msg = String::new();

            for (idx, src) in req.sources.iter().enumerate() {
                let current_idx = idx + 1;
                let prog = (current_idx as f32 / total_items as f32).clamp(0.05, 0.98);

                let src_path = std::path::Path::new(src);
                if !src_path.exists() { continue; }
                let fname = match src_path.file_name() {
                    Some(n) => n.to_string_lossy().to_string(),
                    None => continue,
                };

                tx.send(BackendMsg::TransferProgress {
                    id: "file_transfer".into(),
                    message: format!("{}/{} aktarılıyor: {}", current_idx, total_items, fname),
                    progress: prog,
                }).ok();

                let dst = format!("{}/{}", req.dest_dir.trim_end_matches('/'), fname);

                match crate::state::execute_transfer_item(src, &dst, req.copy, crate::state::ConflictChoice::Replace) {
                    Ok(Some(action)) => {
                        action.collect_targets(&mut created_paths);
                        undos.push(action);
                        direct_count += 1;
                    }
                    Ok(None) => {}
                    Err(e) => {
                        err_msg = e;
                    }
                }
            }

            if !undos.is_empty() {
                let _ = crate::backend::clipboard::push_undo_batch(undos);
            }

            if direct_count > 0 {
                let msg = format!("{} öğe {} içine {}", direct_count, req.dest_dir, if req.copy { "kopyalandı" } else { "taşındı" });
                tx.send(BackendMsg::TransferDone {
                    success: true,
                    message: msg,
                    target_dir: req.dest_dir,
                    selected_paths: created_paths,
                }).ok();
            } else {
                let msg = if err_msg.is_empty() { "İşlem tamamlanamadı".to_string() } else { format!("Hata: {}", err_msg) };
                tx.send(BackendMsg::TransferDone {
                    success: false,
                    message: msg,
                    target_dir: req.dest_dir,
                    selected_paths: vec![],
                }).ok();
            }
        });
    }

    // ── Search debounce ──────────────────────────────────────────────────────

    pub fn load_sidebar_data(&mut self) {
        let tx = self.tx.clone();

        std::thread::spawn(move || {
            // Bookmarks
            let bms = get_bookmarks();
            tx.send(BackendMsg::BookmarksLoaded(bms)).ok();

            // Drives
            let drives = get_drives();
            tx.send(BackendMsg::DrivesLoaded(drives)).ok();
        });

        // Block devices — async (zbus)
        let tx2 = self.tx.clone();
        self.runtime.spawn(async move {
            let devs = list_block_devices();
            tx2.send(BackendMsg::BlockDevicesLoaded(devs)).ok();
        });

        // ADB devices
        let tx3 = self.tx.clone();
        std::thread::spawn(move || {
            let adb_devs = crate::backend::adb::list_adb_devices();
            tx3.send(BackendMsg::AdbDevicesLoaded(adb_devs)).ok();
        });
    }

    // ── Process incoming backend messages ────────────────────────────────────

    fn process_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                BackendMsg::DirectoryLoaded(listing) => {
                    self.state.pending_listing = Some(listing);
                }
                BackendMsg::DirectoryError(e) => {
                    self.state.dir.is_loading = false;
                    let cur = self.state.nav.current_path.clone();
                    if cur.starts_with("adb://") {
                        if let Some((serial, dev_path)) = crate::backend::adb::parse_adb_path(&cur) {
                            if dev_path != "/sdcard" && dev_path != "/" {
                                self.state.notify.warning("Klasör bulunamadı veya silindi, telefon ana dizinine dönülüyor…".into());
                                let fallback = crate::backend::adb::make_adb_path(&serial, "/sdcard");
                                self.state.navigate(fallback);
                                return;
                            }
                        }
                    } else if !std::path::Path::new(&cur).exists() {
                        if let Some(home) = dirs::home_dir() {
                            self.state.notify.warning("Klasör bulunamadı veya silindi, Ana dizine dönülüyor…".into());
                            self.state.navigate(home.to_string_lossy().to_string());
                            return;
                        }
                    }
                    self.state.pending_error = Some(e);
                }
                BackendMsg::DirectoryPermissionDenied(path) => {
                    self.state.dir.is_loading = false;
                    self.state.pending_root_auth = Some(crate::state::RootAuthRequest {
                        title: format!("\"{}\" dizinine erişmek için Root (Sudo) yetkisi gerekiyor", path),
                        cmd: "ls".to_string(),
                        args: vec![path.clone()],
                        reload: true,
                        auth_type: Some(format!("dir_access:{}", path)),
                    });
                    self.state.pending_root_wrong = false;
                }
                BackendMsg::BookmarksLoaded(bms) => {
                    self.state.sidebar.bookmarks = bms;
                }
                BackendMsg::DrivesLoaded(drives) => {
                    self.state.sidebar.drives = drives;
                }
                BackendMsg::BlockDevicesLoaded(devs) => {
                    self.state.sidebar.block_devices = devs;
                }
                BackendMsg::AdbDevicesLoaded(adb_devs) => {
                    self.state.sidebar.adb_devices = adb_devs;
                }
                BackendMsg::MountDone { success, message, mount_point } => {
                    if success {
                        // "Zaten mounted" mesajını info olarak göster
                        if message.contains("Zaten mounted") {
                            self.state.notify.info(format!("Already mounted: {}", mount_point));
                        } else {
                            self.state.notify.success(format!("Mounted: {}", mount_point));
                        }
                        self.state.mount_flow_reset = true;
                        self.load_sidebar_data();
                    } else if message == "HIBERNATION: Windows hızlı başlatma açık — Windows'ta kapatın" {
                        self.state.notify.warning(message);
                        self.state.mount_flow_reset = true;
                    } else {
                        self.state.notify.error(message);
                    }
                }
                BackendMsg::UnmountDone { success, message, target } => {
                    if success {
                        self.state.notify.success(format!("Unmounted: {}", target));
                        self.state.mount_flow_reset = true;
                        self.load_sidebar_data();
                        if self.state.nav.current_path.starts_with(&target) {
                            if let Some(home) = dirs::home_dir() {
                                self.state.navigate(home.to_string_lossy().to_string());
                            }
                        }
                    } else {
                        self.state.notify.error(format!("Unmount failed: {}", message));
                    }
                }
                BackendMsg::TerminalOutput { lines, sync_nav } => {
                    self.state.terminal.is_running = false;
                    for line in lines {
                        self.state.terminal.history.push(line);
                    }
                    self.state.terminal.scroll_to_bottom = true;
                    if let Some(path) = sync_nav {
                        self.state.navigate(path);
                    }
                }
                BackendMsg::MountNeedsPassword {
                    device,
                    label,
                    error,
                } => {
                    self.state.pending_mount_password = Some((device, label, error));
                }
                BackendMsg::MountWrongPassword { device, label } => {
                    self.state.pending_mount_wrong = Some((device, label));
                }
                BackendMsg::DirectoryChanged => {
                    // inotify — dizin değişti, reload tetikle
                    if !self.state.dir.is_loading {
                        self.state.needs_reload = true;
                    }
                    // Du cache'i invalidate et
                    crate::backend::du_cache::invalidate(&self.state.nav.current_path);
                }
                BackendMsg::ChecksumDone { md5, sha256, path } => {
                    self.state.pending_checksum = Some((path, md5, sha256));
                }
                BackendMsg::DuSizeDone { path, size } => {
                    self.state.du_sizes.insert(path, size);
                }
                BackendMsg::SearchDone(results) => {
                    self.state.dir.entries = std::sync::Arc::new(results);
                    self.state.dir.is_loading = false;
                }
                BackendMsg::UndoDone { message, success, restored_paths } => {
                    if success {
                        self.state.notify.success(message);
                        self.state.needs_reload = true;
                        if !restored_paths.is_empty() {
                            let cur = self.state.nav.current_path.trim_end_matches('/');
                            let matching_in_cur: Vec<String> = restored_paths.iter()
                                .filter(|p| {
                                    std::path::Path::new(p)
                                        .parent()
                                        .map(|parent| parent.to_string_lossy().trim_end_matches('/') == cur)
                                        .unwrap_or(false)
                                })
                                .cloned()
                                .collect();

                            if !matching_in_cur.is_empty() {
                                self.state.select_paths_and_queue(matching_in_cur);
                            } else if let Some(first) = restored_paths.first() {
                                if let Some(parent) = std::path::Path::new(first).parent() {
                                    let parent_str = parent.to_string_lossy().to_string();
                                    self.state.navigate(parent_str);
                                    self.state.select_paths_and_queue(restored_paths);
                                }
                            }
                        }
                    } else {
                        self.state.notify.warning(message);
                    }
                }
                BackendMsg::ArchiveDone { success, message } => {
                    if success {
                        self.state.notify.success(message);
                        self.state.needs_reload = true;
                    } else {
                        self.state.notify.error(message);
                    }
                }
                BackendMsg::RootAuthDone { success, message, reload } => {
                    if success {
                        self.state.notify.success(message);
                        if reload {
                            self.state.refresh();
                        }
                    } else {
                        self.state.notify.error(message);
                    }
                    self.state.pending_root_auth = None;
                    self.state.pending_root_wrong = false;
                }
                BackendMsg::RootAuthWrongPassword => {
                    self.state.pending_root_wrong = true;
                }
                BackendMsg::PreviewLoaded { path, result } => {
                    if self.state.preview_loading.as_deref() == Some(&path) {
                        self.state.preview_loading = None;
                    }
                    self.state.preview_cache.insert(path, result);
                }
                BackendMsg::GitStatusLoaded(status) => {
                    self.state.git_status = status;
                }
                BackendMsg::IpcOpenPath(path) => {
                    self.state.open_in_new_tab(path);
                    self.state.notify.info("Mevcut oturumda yeni sekme açıldı".into());
                }
                BackendMsg::TransferProgress { id, message, progress } => {
                    self.state.notify.update_progress(&id, message, progress);
                }
                BackendMsg::TransferDone { success, message, target_dir, selected_paths } => {
                    self.state.notify.finish_progress("file_transfer", message, success);
                    if success {
                        let cur = self.state.nav.current_path.trim_end_matches('/');
                        let tgt = target_dir.trim_end_matches('/');
                        if cur == tgt && !selected_paths.is_empty() {
                            self.state.select_paths_and_queue(selected_paths);
                        }
                    }
                    self.state.refresh();
                }
            }
        }
    }
}

impl eframe::App for ArchExplorer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.style_mut(|s| {
            s.interaction.resize_grab_radius_side = 12.0_f32;
        });

        // Scroll hızını 2.5 kat artır — ctrl+scroll zoom'u etkilememek için
        // normal scroll'a özel uygula
        ctx.input_mut(|i| {
            if !i.modifiers.ctrl {
                i.smooth_scroll_delta *= 2.5;
            }
        });

        // 1. Gelen backend mesajlarını işle
        self.process_messages();

        // 2. Pending state'leri uygula
        self.state.apply_pending();

        if self.state.terminal.is_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        if self.state.preview_loading.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(60));
        }

        // ── Ctrl+Scroll zoom ─────────────────────────────────────────────
        // ui.input() içinde değil, ctx.input() ile — her frame bir kere çalışır
        let old_scale = self.state.view_scale;
        ctx.input(|i| {
            if i.modifiers.ctrl {
                let delta = i.raw_scroll_delta.y;
                if delta.abs() > 0.1 {
                    // raw_scroll_delta genellikle 10-120 arası gelir (mouse wheel = ~53)
                    // normalleştirip 0.05-0.15 arası değişim sağla
                    let step = (delta / 53.0).clamp(-0.5, 0.5) * 0.3;
                    self.state.view_scale = (self.state.view_scale + step).clamp(0.4, 3.0);
                }
                let zd = i.zoom_delta();
                if (zd - 1.0).abs() > 0.001 {
                    self.state.view_scale = (self.state.view_scale * zd).clamp(0.4, 3.0);
                }
            }
        });
        if (self.state.view_scale - old_scale).abs() > 0.001 {
            crate::theme::save_view_scale(self.state.view_scale);
        }

        // 3. Yeniden yükleme gerekiyor mu?
        if self.state.needs_reload {
            let path = self.state.nav.current_path.clone();
            if !path.starts_with("adb://") {
                self.start_watcher(&path);  // inotify — sadece yerel dosya sistemini izle
            }
            self.load_current_directory();
        }

        // Periyodik cihaz/sidebar taraması (3 saniyede bir ADB & diskler)
        if self.last_sidebar_refresh.elapsed() > std::time::Duration::from_secs(3) {
            self.last_sidebar_refresh = std::time::Instant::now();
            self.load_sidebar_data();
        }

        // 3b. Search debounce — 280ms sonra tetikle
        if let Some(t) = self.state.pending_search_changed {
            if t.elapsed() > std::time::Duration::from_millis(280) {
                self.state.pending_search_changed = None;
                let q = self.state.pending_search_query.take().unwrap_or_default();
                if !q.is_empty() {
                    let tx    = self.tx.clone();
                    let dir   = self.state.nav.current_path.clone();
                    let query = q.clone();
                    self.state.dir.is_loading = true;
                    std::thread::spawn(move || {
                        let results = crate::backend::fs_ops::search_files(dir, query, Some(300));
                        tx.send(BackendMsg::SearchDone(results)).ok();
                    });
                    self.state.search.query = q;
                } else {
                    self.state.search.query.clear();
                    self.state.needs_reload = true;
                }
            }
        }

        // 3c. Checksum request
        if let Some(path) = self.state.pending_checksum_request.take() {
            self.request_checksum(path);
        }

        // 3d. Pending extract
        if let Some((archive, dest)) = self.state.pending_extract.take() {
            self.extract_archive(archive, dest);
        }

        // 3e. Du size request
        if let Some(path) = self.state.pending_du_request.take() {
            self.request_du_size(path);
        }

        // 3e2. Async Transfer request
        if let Some(req) = self.state.pending_transfer.take() {
            self.execute_async_transfer(req);
        }

        // 3f. Checksum result → open dialog (unless Properties panel is handling it)
        if !matches!(self.state.panels.active, crate::state::ActivePanel::Properties(_)) {
            if let Some((path, md5, sha256)) = self.state.pending_checksum.take() {
                self.state.panels.open_checksum(path, md5, sha256);
            }
        }

        // 4. Bildirim süreleri tick
        self.state.notify.tick();

        // 5. Tema hot-reload (dev modda)
        #[cfg(debug_assertions)]
        self.theme.tick_hot_reload(ctx);

        // 6. Klavye kısayolları
        self.handle_shortcuts(ctx);

        // Sync global dragging flag into ctx data
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("is_file_dragging"), self.state.drag.dragging));

        // 7. UI render
        self.render_ui(ctx);

        // 8. Sürekli yenile — inotify eventleri için gerekli
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Transparan — Hyprland blur için
        egui::Rgba::TRANSPARENT.to_array()
    }
}

impl ArchExplorer {
    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        use egui::Key;

        ctx.input(|i| {
            // Back/Forward
            if i.key_pressed(Key::ArrowLeft) && i.modifiers.alt {
                self.state.go_back();
            }
            if i.key_pressed(Key::ArrowRight) && i.modifiers.alt {
                self.state.go_forward();
            }
            if i.key_pressed(Key::ArrowUp) && i.modifiers.alt {
                self.state.go_up();
            }

            // Ctrl+P — Dual Pane toggle
            if i.key_pressed(Key::P) && i.modifiers.ctrl {
                if self.state.dual_pane {
                    self.state.close_dual_pane();
                } else {
                    self.state.open_dual_pane();
                }
            }

            // F5 — Refresh / dual pane'de karşı panele kopyala
            if i.key_pressed(Key::F5) {
                if self.state.dual_pane {
                    // F5: aktif pane'den karşı pane'e kopyala
                    if let Some(dest) = self.state.other_pane_path() {
                        for src in self.state.selected_paths() {
                            let fname = std::path::Path::new(&src)
                                .file_name().map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            crate::backend::fs_ops::copy_item(src, format!("{}/{}", dest, fname)).ok();
                        }
                        self.state.refresh();
                    }
                } else {
                    self.state.refresh();
                }
            }

            // Toggle hidden
            if i.key_pressed(Key::H) && i.modifiers.ctrl {
                self.state.dir.show_hidden = !self.state.dir.show_hidden;
                self.state.refresh();
            }

            // Ctrl+Shift+N — New Folder
            if i.key_pressed(Key::N) && i.modifiers.ctrl && i.modifiers.shift {
                let path = crate::ui::context_menu::unique_new_folder_path(&self.state.nav.current_path);
                crate::ui::context_menu::try_create_dir_or_elevate(&mut self.state, path);
            }

            // Ctrl+N — New File
            if i.key_pressed(Key::N) && i.modifiers.ctrl && !i.modifiers.shift {
                let path = crate::ui::context_menu::unique_new_file_path(&self.state.nav.current_path);
                crate::ui::context_menu::try_create_file_or_elevate(&mut self.state, path);
            }

            // Ctrl+T — yeni sekme
            if i.key_pressed(Key::T) && i.modifiers.ctrl {
                let path = self.state.nav.current_path.clone();
                self.state.tabs.new_tab(path);
                let new_idx = self.state.tabs.active_idx;
                self.state.switch_tab(new_idx);
            }
            // Ctrl+W — sekmeyi kapat
            if i.key_pressed(Key::W) && i.modifiers.ctrl {
                self.state.close_current_tab();
            }
            // Ctrl+Tab — sonraki
            if i.key_pressed(Key::Tab) && i.modifiers.ctrl && !i.modifiers.shift {
                let next = (self.state.tabs.active_idx + 1) % self.state.tabs.len();
                self.state.switch_tab(next);
            }
            // Ctrl+Shift+Tab — önceki
            if i.key_pressed(Key::Tab) && i.modifiers.ctrl && i.modifiers.shift {
                let n = self.state.tabs.len();
                let prev = if self.state.tabs.active_idx == 0 { n - 1 } else { self.state.tabs.active_idx - 1 };
                self.state.switch_tab(prev);
            }

            // Ctrl+C / Event::Copy — Copy
            let is_copy = (i.key_pressed(Key::C) && i.modifiers.ctrl)
                || i.events.iter().any(|e| matches!(e, egui::Event::Copy));
            if is_copy && self.state.rename_target.is_none() && !self.state.search.active && !self.state.terminal.open {
                let paths = self.state.selected_paths();
                if !paths.is_empty() {
                    let count = paths.len();
                    crate::backend::clipboard::clipboard_copy(paths.clone()).ok();
                    self.state.notify.info(format!("{} öğe kopyalandı", count));
                }
            }

            // Ctrl+X / Event::Cut — Cut
            let is_cut = (i.key_pressed(Key::X) && i.modifiers.ctrl)
                || i.events.iter().any(|e| matches!(e, egui::Event::Cut));
            if is_cut && self.state.rename_target.is_none() && !self.state.search.active && !self.state.terminal.open {
                let paths = self.state.selected_paths();
                if !paths.is_empty() {
                    let count = paths.len();
                    crate::backend::clipboard::clipboard_cut(paths.clone()).ok();
                    self.state.notify.info(format!("{} öğe kesildi", count));
                }
            }

            // Ctrl+V / Event::Paste — Paste
            let is_paste = (i.key_pressed(Key::V) && i.modifiers.ctrl)
                || i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)));
            if is_paste && self.state.rename_target.is_none() && !self.state.search.active && !self.state.terminal.open {
                let target_dir = self.state.active_context_path();
                if let Some(cb) = crate::backend::clipboard::clipboard_get() {
                    let is_copy = cb.op == crate::backend::clipboard::ClipboardOp::Copy;
                    if !is_copy {
                        let _ = crate::backend::clipboard::clipboard_clear();
                    }
                    self.state.start_transfer(cb.paths, target_dir, is_copy);
                }
            }

            // Select all
            if i.key_pressed(Key::A) && i.modifiers.ctrl {
                let entries = self.state.dir.entries.clone();
                self.state.selection.select_all(&entries);
            }

            // Arrow keys for file selection and Shift+Arrow for range selection
            if !self.state.terminal.open && !self.state.search.active && self.state.rename_target.is_none() {
                let entries_len = self.state.dir.entries.len();
                if entries_len > 0 {
                    let mut move_dir: Option<isize> = None;
                    if i.key_pressed(Key::ArrowDown) && !i.modifiers.alt {
                        move_dir = Some(1);
                    } else if i.key_pressed(Key::ArrowUp) && !i.modifiers.alt {
                        move_dir = Some(-1);
                    }

                    if let Some(delta) = move_dir {
                        let visible_entries = self.state.dir.visible_entries(&self.state.search.query);
                        let v_len = visible_entries.len();
                        if v_len > 0 {
                            let cur_path = self.state.selection.last_clicked.clone()
                                .or_else(|| self.state.selection.paths().first().cloned());
                            let cur_idx = cur_path.and_then(|p| visible_entries.iter().position(|e| e.path == p));

                            let next_idx = match cur_idx {
                                Some(idx) => (idx as isize + delta).clamp(0, v_len as isize - 1) as usize,
                                None => if delta > 0 { 0 } else { v_len - 1 },
                            };
                            let target_path = visible_entries[next_idx].path.clone();

                            if i.modifiers.shift {
                                let anchor = self.state.selection.anchor.clone()
                                    .or_else(|| self.state.selection.last_clicked.clone())
                                    .unwrap_or_else(|| visible_entries[0].path.clone());
                                self.state.selection.select_range(&visible_entries, &anchor, &target_path, i.modifiers.ctrl);
                            } else {
                                self.state.selection.select_only(target_path);
                            }
                        }
                    }
                }
            }

            // Delete & Shift+Delete — Dosyaları Sil / Çöp Kutusuna Taşı
            if i.key_pressed(Key::Delete) && self.state.rename_target.is_none() && !self.state.search.active && !self.state.terminal.open {
                let paths = self.state.selected_paths();
                if !paths.is_empty() {
                    let permanent = i.modifiers.shift;
                    crate::ui::context_menu::try_delete_or_elevate(&mut self.state, paths, permanent);
                }
            }

            // Left / Right Arrow Video Seek & Space Play/Pause (Medya oynatılıyorsa)
            if (self.state.media_player.is_playing || self.state.media_player.is_paused) 
                && self.state.rename_target.is_none() && !self.state.search.active && !self.state.terminal.open 
            {
                if i.key_pressed(Key::ArrowLeft) && !i.modifiers.alt {
                    self.state.media_player.seek_relative(-5.0);
                }
                if i.key_pressed(Key::ArrowRight) && !i.modifiers.alt {
                    self.state.media_player.seek_relative(5.0);
                }
                if i.key_pressed(Key::Space) {
                    self.state.media_player.toggle_pause();
                }
            }

            // Search focus
            if i.key_pressed(Key::F) && i.modifiers.ctrl {
                self.state.search.active = true;
            }

            // Escape — panelleri kapat, seçimi temizle
            // Ctrl+Z — Undo
            if i.key_pressed(Key::Z) && i.modifiers.ctrl {
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    match crate::backend::clipboard::undo_last() {
                        Ok(res) => tx.send(BackendMsg::UndoDone {
                            message: res.message,
                            success: true,
                            restored_paths: res.restored_paths,
                        }).ok(),
                        Err(e)  => tx.send(BackendMsg::UndoDone {
                            message: e,
                            success: false,
                            restored_paths: Vec::new(),
                        }).ok(),
                    };
                });
            }

            // Ctrl+D — Duplicate
            if i.key_pressed(Key::D) && i.modifiers.ctrl && self.state.rename_target.is_none() && !self.state.search.active && !self.state.terminal.open {
                self.state.duplicate_selected();
            }

            // F2 — inline rename
            if i.key_pressed(Key::F2) {
                let sel = self.state.selection.paths();
                if sel.len() == 1 {
                    self.state.start_rename(&sel[0]);
                }
            }

            // Escape — rename iptal
            if i.key_pressed(Key::Escape) {
                if self.state.rename_target.is_some() {
                    self.state.cancel_rename();
                }
                if self.state.panels.active != crate::state::ActivePanel::None {
                    self.state.panels.close();
                } else if self.state.search.active {
                    self.state.search.clear();
                } else {
                    self.state.selection.clear();
                }
            }

            // Settings
            if i.key_pressed(Key::Comma) && i.modifiers.ctrl {
                self.state.panels.open_settings();
            }

            // Preview toggle — F3
            if i.key_pressed(Key::F3) {
                if self.state.panels.active == crate::state::ActivePanel::Preview {
                    self.state.media_player.stop();
                    self.state.panels.close();
                } else {
                    self.state.panels.open_preview();
                }
            }

            // Terminal toggle — Ctrl + ` or F4
            if (i.key_pressed(Key::Backtick) && i.modifiers.ctrl) || i.key_pressed(Key::F4) {
                let cur = self.state.active_context_path();
                self.state.terminal.toggle(Some(cur));
            }

            // Enter — rename varsa onayla, yoksa seçili öğeyi aç
            if i.key_pressed(Key::Enter) && !self.state.search.active {
                if self.state.rename_target.is_some() {
                    self.state.commit_rename();
                } else {
                    let sel = self.state.selection.paths();
                    if sel.len() == 1 {
                        let p = &sel[0];
                        if let Some(entry) = self.state.dir.entries.iter().find(|e| e.path == *p).cloned() {
                            crate::ui::file_grid::open_entry(&mut self.state, &entry);
                        }
                    }
                }
            }
        });
    }

    fn render_ui(&mut self, ctx: &egui::Context) {
        // self.theme.active klonlanarak borçlanma (borrow) çakışması önlendi
        let theme = self.theme.active.clone();
        let t = &theme;

        // ── Custom Titlebar ──────────────────────────────────────────────
        crate::ui::titlebar::render(ctx, t, &mut self.state);

        // ── Top TabBar (under titlebar, full window width) ───────────────
        let show_tab_bar = self.state.dual_pane || self.state.tabs.tabs.len() > 1;
        if show_tab_bar {
            crate::ui::tab_bar::render_top_panel(ctx, t, &mut self.state);
        }

        // ── Ana layout: sidebar | content | preview ──────────────────────
        egui::SidePanel::left("sidebar")
            .width_range(120.0..=300.0)
            .default_width(t.layout.sidebar_width)
            .resizable(true)
            .show_separator_line(false)
            .show(ctx, |ui| {
                crate::ui::sidebar::render(ui, t, &mut self.state, &self.tx, &self.runtime);
            });

        // ── Preview Auto Open / Close based on preview_mode ──────────────
        let sel_paths = self.state.selected_paths();
        let sel_changed = sel_paths != self.state.selection_prev;

        match self.state.preview_mode {
            crate::state::PreviewBehaviorMode::AlwaysOpen => {
                if self.state.is_rubber_banding || sel_paths.len() > 1 {
                    if self.state.panels.active == crate::state::ActivePanel::Preview {
                        self.state.panels.close();
                    }
                } else if self.state.panels.active == crate::state::ActivePanel::None {
                    self.state.panels.open_preview();
                }
            }
            crate::state::PreviewBehaviorMode::AutoOnSelect => {
                if self.state.is_rubber_banding || sel_paths.len() > 1 {
                    if self.state.panels.active == crate::state::ActivePanel::Preview {
                        self.state.panels.close();
                    }
                } else if sel_changed {
                    if sel_paths.len() == 1 {
                        if self.state.panels.active == crate::state::ActivePanel::None {
                            self.state.panels.open_preview();
                        }
                    } else if sel_paths.is_empty() {
                        if self.state.panels.active == crate::state::ActivePanel::Preview {
                            self.state.panels.close();
                        }
                    }
                }
            }
            crate::state::PreviewBehaviorMode::Custom => {
                if self.state.is_rubber_banding || sel_paths.len() > 1 {
                    if self.state.panels.active == crate::state::ActivePanel::Preview {
                        self.state.panels.close();
                    }
                } else if sel_changed {
                    if sel_paths.len() == 1 {
                        let p = &sel_paths[0];
                        let should_open = if let Some(e) = self.state.dir.entries.iter().find(|e| e.path == *p) {
                            if e.is_dir {
                                self.state.preview_custom_dir
                            } else {
                                let ext = e.extension.to_lowercase();
                                match ext.as_str() {
                                    "png"|"jpg"|"jpeg"|"webp"|"bmp"|"gif"|"ico"|"svg" => self.state.preview_custom_image,
                                    "mp4"|"mkv"|"avi"|"mov"|"webm"|"flv" => self.state.preview_custom_video,
                                    "mp3"|"flac"|"ogg"|"wav"|"aac"|"m4a" => self.state.preview_custom_audio,
                                    "rs"|"py"|"js"|"ts"|"go"|"c"|"cpp"|"h"|"lua"|"rb"|"sh"|"toml"|"json"|"yaml"|"yml"|"txt"|"log"|"md"|"css"|"html" => self.state.preview_custom_text,
                                    "pdf"|"doc"|"docx"|"odt"|"epub" => self.state.preview_custom_doc,
                                    _ => false,
                                }
                            }
                        } else {
                            false
                        };
                        if should_open {
                            if self.state.panels.active == crate::state::ActivePanel::None {
                                self.state.panels.open_preview();
                            }
                        } else if self.state.panels.active == crate::state::ActivePanel::Preview {
                            self.state.panels.close();
                        }
                    } else if sel_paths.is_empty() && self.state.panels.active == crate::state::ActivePanel::Preview {
                        self.state.panels.close();
                    }
                }
            }
        }
        self.state.selection_prev = sel_paths;

        // Medya oynatıcı kontrolü: Eğer önizleme paneli açık değilse veya seçili dosya çalınan medyadan farklıysa durdur!
        let preview_is_open = t.layout.preview_visible && self.state.panels.active == crate::state::ActivePanel::Preview;
        if !preview_is_open {
            if self.state.media_player.is_playing || self.state.media_player.is_paused {
                self.state.media_player.stop();
            }
        } else if let Some(ref current_media) = self.state.media_player.current_path.clone() {
            let cur_selected = self.state.selected_paths();
            if cur_selected.len() != 1 || cur_selected[0] != *current_media {
                self.state.media_player.stop();
            }
        }

        let mut preview_is_resizing = false;

        if t.layout.preview_visible
            && self.state.panels.active == crate::state::ActivePanel::Preview
        {
            let screen_w = ctx.screen_rect().width();
            let min_pw = 160.0_f32;
            let max_pw = (screen_w - 180.0).max(min_pw);
            let def_pw = 280.0_f32.min(max_pw);

            let pw = self.state.preview_width.unwrap_or(def_pw).clamp(min_pw, max_pw);

            let prev_fill = if t.effects.glass_panels {
                Color32::from_rgba_unmultiplied(
                    t.colors.bg_mantle().r(),
                    t.colors.bg_mantle().g(),
                    t.colors.bg_mantle().b(),
                    (t.effects.panel_opacity * 255.0) as u8,
                )
            } else {
                t.colors.bg_mantle()
            };

            let panel_resp = egui::SidePanel::right("preview_sidepanel")
                .width_range(min_pw..=max_pw)
                .default_width(pw)
                .resizable(true)
                .show_separator_line(false)
                .frame(egui::Frame {
                    fill: prev_fill,
                    ..Default::default()
                })
                .show(ctx, |ui| {
                    crate::ui::preview_panel::render(ui, t, &mut self.state, &self.tx);
                });

            let panel_rect = panel_resp.response.rect;
            let is_primary_down = ctx.input(|i| i.pointer.primary_down());
            let press_origin = ctx.input(|i| i.pointer.press_origin());

            let resizing_id = egui::Id::new("preview_resizing_in_progress");
            let mut was_resizing: bool = ctx.data_mut(|d| d.get_temp(resizing_id).unwrap_or(false));

            if is_primary_down {
                if !was_resizing {
                    if let Some(p) = press_origin {
                        if (p.x - panel_rect.left()).abs() < 24.0 || panel_rect.contains(p) {
                            was_resizing = true;
                        }
                    }
                }
            } else {
                was_resizing = false;
            }
            ctx.data_mut(|d| d.insert_temp(resizing_id, was_resizing));
            preview_is_resizing = was_resizing;

            // Visual resize grip handle on the left divider
            let handle_hovered = ctx.input(|i| {
                i.pointer.hover_pos().map(|p| (p.x - panel_rect.left()).abs() < 14.0 && p.y >= panel_rect.top() && p.y <= panel_rect.bottom()).unwrap_or(false)
            });
            if handle_hovered || was_resizing {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            let handle_color = if was_resizing {
                t.colors.accent()
            } else if handle_hovered {
                Color32::from_rgba_unmultiplied(t.colors.accent().r(), t.colors.accent().g(), t.colors.accent().b(), 200)
            } else {
                Color32::from_rgba_unmultiplied(t.colors.border().r(), t.colors.border().g(), t.colors.border().b(), 160)
            };
            let pill_h = 56.0_f32;
            let pill_w = if handle_hovered || was_resizing { 5.0 } else { 3.5 };
            let pill_rect = egui::Rect::from_center_size(
                egui::Pos2::new(panel_rect.left(), panel_rect.center().y),
                egui::Vec2::new(pill_w, pill_h)
            );
            ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("preview_grip_layer")))
                .rect_filled(pill_rect, egui::Rounding::same(2.0), handle_color);

            // Kullanıcı panel genişliğini değiştirdiğinde state'e yaz
            let actual_w = panel_resp.response.rect.width();
            if actual_w > 0.0 && (was_resizing || ctx.input(|i| i.pointer.is_decidedly_dragging()) || self.state.preview_width.is_some()) {
                self.state.preview_width = Some(actual_w.clamp(min_pw, max_pw));
            }
        }

        ctx.data_mut(|d| d.insert_temp(egui::Id::new("is_resizing_preview"), preview_is_resizing));

        // ── Bottom Terminal Panel (VS Codium style) ──────────────────────
        crate::ui::terminal_panel::render(ctx, t, &mut self.state, &self.tx);

        // ── Bottom Status Bar (Always docked and visible at bottom, Nemo style) ───
        crate::ui::statusbar::render_bottom_panel(ctx, t, &mut self.state);

        // ── Ana içerik ───────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.state.dual_pane {
                // ── Dual Pane layout ─────────────────────────────────────
                let avail_w  = ui.available_width();
                let splitter = 1.0_f32;
                let pane_w   = ((avail_w - splitter) / 2.0).max(80.0);

                // Shared toolbar
                crate::ui::toolbar::render(ui, t, &mut self.state);

                let body_h = ui.available_height().max(40.0);

                // Check pointer interaction BEFORE rendering either pane so active pane switch happens immediately
                let cursor_pos = ui.cursor().min;
                let left_rect = egui::Rect::from_min_size(cursor_pos, egui::Vec2::new(pane_w, body_h));
                let right_rect = egui::Rect::from_min_size(egui::pos2(cursor_pos.x + pane_w + splitter, cursor_pos.y), egui::Vec2::new(pane_w, body_h));

                let ptr_interact = ui.ctx().input(|i| {
                    if i.pointer.primary_clicked() || i.pointer.secondary_clicked() || i.pointer.primary_down() || i.pointer.secondary_down() {
                        i.pointer.interact_pos().or(i.pointer.latest_pos())
                    } else {
                        None
                    }
                });

                if let Some(pos) = ptr_interact {
                    if left_rect.contains(pos) && self.state.active_pane == crate::state::PaneId::Right {
                        self.state.switch_active_pane();
                    } else if right_rect.contains(pos) && self.state.active_pane == crate::state::PaneId::Left {
                        self.state.switch_active_pane();
                    }
                }

                let left_active = self.state.active_pane == crate::state::PaneId::Left;
                let snap_clone  = self.state.right_pane.clone();

                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

                    // ── Sol pane body ──────────────────────────────────────
                    let (left_body_rect, left_body_resp) = ui.allocate_exact_size(egui::Vec2::new(pane_w, body_h), egui::Sense::hover());

                    // Drag & Drop onto inactive left pane (evaluated before children run)
                    if !left_active && self.state.drag.dragging && left_body_resp.hovered() {
                        ui.painter().rect_stroke(left_body_rect, egui::Rounding::same(t.shape.radius_sm), egui::Stroke::new(1.5, t.colors.accent()));
                        if ui.input(|i| i.pointer.any_released()) {
                            if let Some(ref snap) = snap_clone {
                                let dest = snap.nav.current_path.clone();
                                self.state.drop_files_into_dir(dest);
                            }
                        }
                    }

                    let mut left_body_ui = ui.new_child(egui::UiBuilder::new().max_rect(left_body_rect).layout(egui::Layout::top_down(egui::Align::Min)));
                    if left_active {
                        render_active_pane(&mut left_body_ui, t, &mut self.state, "left", &self.tx);
                    } else {
                        if let Some(ref snap) = snap_clone {
                            render_inactive_pane(&mut left_body_ui, t, snap, "left", &mut self.state, &self.tx);
                        }
                    }

                    // Splitter — body
                    let sp_r = ui.cursor();
                    ui.painter().vline(
                        sp_r.min.x, sp_r.min.y..=sp_r.min.y + body_h,
                        egui::Stroke::new(1.0_f32, t.colors.border()),
                    );
                    ui.add_space(splitter);

                    // ── Sağ pane body ──────────────────────────────────────
                    let (right_body_rect, right_body_resp) = ui.allocate_exact_size(egui::Vec2::new(pane_w, body_h), egui::Sense::hover());

                    // Drag & Drop onto inactive right pane (evaluated before children run)
                    if left_active && self.state.drag.dragging && right_body_resp.hovered() {
                        ui.painter().rect_stroke(right_body_rect, egui::Rounding::same(t.shape.radius_sm), egui::Stroke::new(1.5, t.colors.warning()));
                        if ui.input(|i| i.pointer.any_released()) {
                            if let Some(ref snap) = snap_clone {
                                let dest = snap.nav.current_path.clone();
                                self.state.drop_files_into_dir(dest);
                            }
                        }
                    }

                    let mut right_body_ui = ui.new_child(egui::UiBuilder::new().max_rect(right_body_rect).layout(egui::Layout::top_down(egui::Align::Min)));
                    if !left_active {
                        render_active_pane(&mut right_body_ui, t, &mut self.state, "right", &self.tx);
                    } else {
                        if let Some(ref snap) = snap_clone {
                            render_inactive_pane(&mut right_body_ui, t, snap, "right", &mut self.state, &self.tx);
                        }
                    }
                });

            } else {
                // ── Single Pane (normal) ─────────────────────────────────
                crate::ui::toolbar::render(ui, t, &mut self.state);
                ui.add_space(t.spacing.sm);
                if self.state.panels.active == crate::state::ActivePanel::DevicesView {
                    crate::ui::devices_view::render(ui, t, &mut self.state, &self.tx);
                } else {
                    crate::ui::file_grid::render(ui, t, &mut self.state);
                }
            }
        });

        // ── Modal Paneller (overlay olarak) ─────────────────────────────
        match &self.state.panels.active.clone() {
            crate::state::ActivePanel::Mount => {
                crate::ui::mount_panel::render(ctx, t, &mut self.state, &self.tx, &self.runtime);
            }
            crate::state::ActivePanel::Settings => {
                crate::ui::settings::render(ctx, t, &mut self.state, &mut self.theme);
            }
            crate::state::ActivePanel::OpenWith(path) => {
                let path = path.clone();
                crate::ui::open_with::render(ctx, t, &mut self.state, &path);
            }
            crate::state::ActivePanel::Properties(path) => {
                let path = path.clone();
                crate::ui::properties::render(ctx, t, &mut self.state, &path);
            }
            crate::state::ActivePanel::Checksum(path, md5, sha256) => {
                let (path, md5, sha256) = (path.clone(), md5.clone(), sha256.clone());
                render_checksum_dialog(ctx, t, &mut self.state, &path, &md5, &sha256);
            }
            crate::state::ActivePanel::Compress => {
                let sel = self.state.selected_paths();
                let cur = self.state.nav.current_path.clone();
                render_compress_dialog(ctx, t, &mut self.state, sel, cur, &self.tx);
            }
            crate::state::ActivePanel::BulkRename => {
                crate::ui::bulk_rename::render(ctx, t, &mut self.state);
            }
            crate::state::ActivePanel::DevicesView => {
                // DevicesView CentralPanel içinde render edilir
            }
            crate::state::ActivePanel::Network => {
                crate::ui::network::render(ctx, t, &mut self.state);
            }
            crate::state::ActivePanel::Commands => {
                crate::ui::commands_panel::render(ctx, t, &mut self.state);
            }
            _ => {}
        }

        // ── Context Menu ─────────────────────────────────────────────────
        if self.state.panels.context_menu.is_some() {
            crate::ui::context_menu::render(ctx, t, &mut self.state, &self.tx, &self.runtime);
        }

        // ── Bildirimler ──────────────────────────────────────────────────
        crate::ui::notifications::render(ctx, t, &mut self.state);

        // ── Root Authentication Dialog ───────────────────────────────────
        if self.state.pending_root_auth.is_some() {
            render_root_auth_dialog(ctx, t, &mut self.state, &self.tx);
        }

        // ── File Conflict Dialog ─────────────────────────────────────────
        if self.state.conflict_dialog.is_some() {
            render_conflict_dialog(ctx, t, &mut self.state);
        }

        // ── Delete Error Dialog ──────────────────────────────────────────
        if self.state.delete_error_dialog.is_some() {
            render_delete_error_dialog(ctx, t, &mut self.state);
        }

        // ── Fullscreen Video Overlay (arch-explorer-sorcerer style) ──────
        if self.state.is_video_fullscreen {
            render_fullscreen_video(ctx, t, &mut self.state);
        }

        // ── End of frame drag cleanup ────────────────────────────────────
        // If mouse button was released anywhere and no drop handled it, cancel drag
        if self.state.drag.dragging && ctx.input(|i| i.pointer.any_released() || !i.pointer.any_down()) {
            self.state.drag.cancel();
        }
    }
}

// ── Pane helpers ──────────────────────────────────────────────────────────────

fn pane_indicator(ui: &mut egui::Ui, t: &crate::theme::types::AexTheme, state: &AppState) {
    use egui::{RichText, Color32, Layout, Align};
    let c = &t.colors;
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        for (id, label) in [(crate::state::PaneId::Left, "◀ Left"), (crate::state::PaneId::Right, "Right ▶")] {
            let active = state.active_pane == id;
            ui.label(RichText::new(label)
                .color(if active { c.accent() } else { c.text_disabled() })
                .size(t.typography.size_xs));
            if id == crate::state::PaneId::Left {
                ui.add_space(8.0);
            }
        }
    });
    ui.add_space(2.0);
}

pub(crate) fn render_active_pane(
    ui: &mut egui::Ui,
    t: &crate::theme::types::AexTheme,
    state: &mut AppState,
    pane_tag: &str,
    tx: &std::sync::mpsc::Sender<BackendMsg>,
) {
    if state.panels.active == crate::state::ActivePanel::DevicesView {
        crate::ui::devices_view::render(ui, t, state, tx);
    } else {
        crate::ui::file_grid::render_tagged(ui, t, state, pane_tag);
    }
}

pub(crate) fn render_inactive_pane(
    ui: &mut egui::Ui,
    t: &crate::theme::types::AexTheme,
    snap: &crate::state::PaneSnapshot,
    pane_tag: &str,
    state: &mut AppState,
    tx: &std::sync::mpsc::Sender<BackendMsg>,
) {
    if snap.is_devices_view {
        crate::ui::devices_view::render(ui, t, state, tx);
    } else {
        crate::ui::file_grid::render_snapshot_tagged(ui, t, snap, pane_tag);
    }
}

// ── Checksum Dialog ───────────────────────────────────────────────────────────

fn render_checksum_dialog(
    ctx: &egui::Context,
    t: &crate::theme::types::AexTheme,
    state: &mut AppState,
    path: &str,
    md5: &str,
    sha256: &str,
) {
    use egui::*;
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    let screen = ctx.screen_rect();
    egui::Area::new(Id::new("checksum_backdrop"))
        .fixed_pos(screen.min).order(Order::Background)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0,0,0,140));
            if ui.allocate_rect(screen, Sense::click()).clicked() { state.panels.close(); }
        });

    let pw = 480.0_f32; let ph = 220.0_f32;
    egui::Area::new(Id::new("checksum_dialog"))
        .fixed_pos(Pos2::new(screen.center().x - pw/2.0, screen.center().y - ph/2.0))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame {
                fill: c.bg_mantle(),
                rounding: Rounding::same(s.radius_lg),
                stroke: Stroke::new(1.0_f32, c.border()),
                inner_margin: Margin::same(sp.lg),
                shadow: epaint::Shadow { offset: Vec2::new(0.0,8.0), blur: 20.0,
                    spread: 0.0, color: Color32::from_rgba_unmultiplied(0,0,0,100) },
                ..Default::default()
            }.show(ui, |ui| {
                ui.set_width(pw - sp.lg * 2.0);
                // Header
                ui.horizontal(|ui| {
                    ui.label(RichText::new("# Checksum").color(c.accent())
                        .size(t.typography.size_lg).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("✕").clicked() { state.panels.close(); }
                    });
                });
                ui.label(RichText::new(
                    std::path::Path::new(path).file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                ).color(c.text_hint()).size(t.typography.size_xs));

                ui.add_space(sp.md);

                // MD5 row
                hash_row(ui, t, "MD5   ", md5);
                ui.add_space(sp.xs);
                hash_row(ui, t, "SHA256", sha256);

                ui.add_space(sp.md);
                ui.label(RichText::new("Click a hash to copy")
                    .color(c.text_disabled()).size(t.typography.size_xs).italics());
            });
        });

    if ctx.input(|i| i.key_pressed(Key::Escape)) { state.panels.close(); }
}

fn hash_row(ui: &mut egui::Ui, t: &crate::theme::types::AexTheme, label: &str, hash: &str) {
    use egui::*;
    let c = &t.colors; let s = &t.shape;
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).color(c.text_hint())
            .size(t.typography.size_xs).monospace());
        ui.add_space(4.0);
        let resp = ui.add(
            egui::Button::new(RichText::new(hash).color(c.text_secondary())
                .size(t.typography.size_xs).monospace())
                .fill(t.colors.bg_crust())
                .rounding(Rounding::same(s.radius_sm))
                .stroke(Stroke::NONE)
        );
        if resp.clicked() {
            crate::backend::clipboard::copy_to_system_clipboard_async(hash.to_string());
        }
        if resp.hovered() {
            egui::show_tooltip_text(ui.ctx(), ui.layer_id(), Id::new("hash_tt"), "Click to copy");
        }
    });
}

// ── Compress Dialog ───────────────────────────────────────────────────────────

fn render_compress_dialog(
    ctx: &egui::Context,
    t: &crate::theme::types::AexTheme,
    state: &mut AppState,
    selected: Vec<String>,
    current_dir: String,
    tx: &std::sync::mpsc::Sender<BackendMsg>,
) {
    use egui::*;
    let c  = &t.colors;
    let s  = &t.shape;
    let sp = &t.spacing;

    let screen = ctx.screen_rect();
    egui::Area::new(Id::new("compress_backdrop"))
        .fixed_pos(screen.min).order(Order::Background)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, Rounding::ZERO,
                Color32::from_rgba_unmultiplied(0,0,0,140));
            if ui.allocate_rect(screen, Sense::click()).clicked() { state.panels.close(); }
        });

    let name_id  = Id::new("compress_name");
    let fmt_id   = Id::new("compress_fmt");
    let mut name: String = ctx.data_mut(|d|
        d.get_temp(name_id).unwrap_or_else(|| {
            selected.first().and_then(|p|
                std::path::Path::new(p).file_stem()
                    .map(|s| s.to_string_lossy().to_string())
            ).unwrap_or_else(|| "archive".to_string())
        })
    );
    let mut fmt: String = ctx.data_mut(|d|
        d.get_temp(fmt_id).unwrap_or_else(|| "zip".to_string())
    );

    let pw = 400.0_f32; let ph = 260.0_f32;
    egui::Area::new(Id::new("compress_dialog"))
        .fixed_pos(Pos2::new(screen.center().x - pw/2.0, screen.center().y - ph/2.0))
        .order(Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame {
                fill: c.bg_mantle(),
                rounding: Rounding::same(s.radius_lg),
                stroke: Stroke::new(1.0_f32, c.border()),
                inner_margin: Margin::same(sp.lg),
                shadow: epaint::Shadow { offset: Vec2::new(0.0,8.0), blur: 20.0,
                    spread: 0.0, color: Color32::from_rgba_unmultiplied(0,0,0,100) },
                ..Default::default()
            }.show(ui, |ui| {
                ui.set_width(pw - sp.lg * 2.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Compress Files").color(c.accent())
                        .size(t.typography.size_lg).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("✕").clicked() { state.panels.close(); }
                    });
                });
                ui.label(RichText::new(format!("{} files selected", selected.len()))
                    .color(c.text_hint()).size(t.typography.size_xs));
                ui.add_space(sp.md);

                // Name
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Name:").color(c.text_secondary())
                        .size(t.typography.size_sm));
                    ui.add_space(sp.xs);
                    ui.add(egui::TextEdit::singleline(&mut name)
                        .desired_width(200.0)
                        .font(FontId::new(t.typography.size_sm, FontFamily::Proportional)));
                });
                ui.add_space(sp.sm);

                // Format
                ui.label(RichText::new("Format:").color(c.text_secondary())
                    .size(t.typography.size_sm));
                ui.add_space(sp.xs);
                ui.horizontal(|ui| {
                    for f in &["zip", "tar.gz", "tar.bz2", "tar.xz", "7z"] {
                        let active = fmt == *f;
                        let btn = ui.add(
                            egui::Button::new(RichText::new(*f)
                                .color(if active { Color32::WHITE } else { c.text_secondary() })
                                .size(t.typography.size_xs).monospace())
                                .fill(if active { c.accent() } else { c.bg_surface0() })
                                .rounding(Rounding::same(s.radius_sm))
                                .stroke(Stroke::new(1.0_f32,
                                    if active { c.accent() } else { c.border() }))
                        );
                        if btn.clicked() { fmt = f.to_string(); }
                    }
                });
                ui.add_space(sp.lg);

                // Buttons
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(
                        RichText::new("Cancel").color(c.text_secondary())
                            .size(t.typography.size_sm))
                        .fill(Color32::TRANSPARENT).stroke(Stroke::NONE)).clicked() {
                        state.panels.close();
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let dest = format!("{}/{}.{}", current_dir, name, fmt);
                        if ui.add(egui::Button::new(
                            RichText::new("Compress").color(Color32::WHITE)
                                .size(t.typography.size_sm))
                            .fill(c.accent())
                            .rounding(Rounding::same(s.radius_sm))
                            .stroke(Stroke::NONE)).clicked() {
                            let tx2 = tx.clone();
                            let sel = selected.clone();
                            let d = dest.clone(); let f = fmt.clone();
                            std::thread::spawn(move || {
                                // compress_files direkt
                                let files: Vec<&str> = sel.iter().map(|s| s.as_str()).collect();
                                let result = match f.as_str() {
                                    "zip" => std::process::Command::new("zip")
                                        .arg("-r").arg(&d).args(&files).status(),
                                    "7z" => std::process::Command::new("7z")
                                        .arg("a").arg(&d).args(&files).status(),
                                    _ => std::process::Command::new("tar")
                                        .arg(match f.as_str() {
                                            "tar.gz"  => "czf",
                                            "tar.bz2" => "cjf",
                                            "tar.xz"  => "cJf",
                                            _         => "czf",
                                        }).arg(&d).args(&files).status(),
                                };
                                match result {
                                    Ok(s) if s.success() =>
                                        tx2.send(BackendMsg::ArchiveDone { success: true,
                                            message: format!("Oluşturuldu: {}", d) }).ok(),
                                    _ => tx2.send(BackendMsg::ArchiveDone { success: false,
                                            message: "Sıkıştırma başarısız".into() }).ok(),
                                };
                            });
                            state.panels.close();
                        }
                    });
                });
            });
        });

    ctx.data_mut(|d| { d.insert_temp(name_id, name); d.insert_temp(fmt_id, fmt); });
    if ctx.input(|i| i.key_pressed(Key::Escape)) { state.panels.close(); }
}

// ── Root Authentication Dialog ────────────────────────────────────────────────

fn render_root_auth_dialog(
    ctx: &egui::Context,
    t: &crate::theme::types::AexTheme,
    state: &mut AppState,
    tx: &std::sync::mpsc::Sender<BackendMsg>,
) {
    use egui::*;
    let Some(req) = state.pending_root_auth.clone() else { return; };
    let c = &t.colors;
    let s = &t.shape;
    let sp = &t.spacing;

    let screen = ctx.screen_rect();
    let pw = 460.0_f32;
    let ph = 240.0_f32;
    let d_rect = Rect::from_center_size(screen.center(), Vec2::new(pw, ph));

    egui::Area::new(Id::new("root_auth_backdrop"))
        .fixed_pos(screen.min).order(Order::Background)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, Rounding::ZERO, Color32::from_rgba_unmultiplied(0, 0, 0, 140));
            let r = ui.allocate_rect(screen, Sense::click());
            let click_pos = ctx.input(|i| i.pointer.interact_pos());
            if r.clicked() && click_pos.map(|p| !d_rect.contains(p) && p.y > 36.0).unwrap_or(false) {
                state.pending_root_auth = None;
                state.pending_root_wrong = false;
            }
        });

    let pass_id = Id::new("root_auth_pass");
    let show_id = Id::new("root_auth_show");
    let mut password: String = ctx.data_mut(|d| d.get_temp(pass_id).unwrap_or_default());
    let mut show_pw: bool = ctx.data_mut(|d| d.get_temp(show_id).unwrap_or(false));
    let wrong = state.pending_root_wrong;

    egui::Area::new(Id::new("root_auth_dialog"))
        .fixed_pos(d_rect.min)
        .order(Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame {
                fill: c.bg_mantle(),
                rounding: Rounding::same(s.radius_lg),
                stroke: Stroke::new(1.0_f32, c.border()),
                inner_margin: Margin::same(sp.lg),
                shadow: epaint::Shadow { offset: Vec2::new(0.0, 8.0), blur: 24.0, spread: 0.0, color: Color32::from_rgba_unmultiplied(0, 0, 0, 100) },
                ..Default::default()
            }.show(ui, |ui| {
                ui.set_width(pw - sp.lg * 2.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔐 Root Authentication").color(c.accent()).size(t.typography.size_base).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("✕").clicked() {
                            state.pending_root_auth = None;
                            state.pending_root_wrong = false;
                        }
                    });
                });
                ui.add_space(sp.xs);
                ui.label(RichText::new(&req.title).color(c.text_secondary()).size(t.typography.size_xs));

                if wrong {
                    ui.add_space(sp.xs);
                    ui.label(RichText::new("Yanlış şifre — tekrar deneyin").color(c.danger()).size(t.typography.size_xs));
                }

                ui.add_space(sp.md);

                let mut submit = false;
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Password:").color(c.text_primary()).size(t.typography.size_sm));
                    ui.add_space(sp.xs);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut password)
                            .password(!show_pw)
                            .desired_width(260.0)
                            .hint_text("Sudo password…")
                    );
                    let foc_id = Id::new("root_pw_foc");
                    let foc: bool = ui.ctx().data_mut(|d| d.get_temp(foc_id).unwrap_or(false));
                    if !foc {
                        resp.request_focus();
                        ui.ctx().data_mut(|d| d.insert_temp(foc_id, true));
                    }
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        submit = true;
                    }
                });

                ui.add_space(sp.xs);
                ui.checkbox(&mut show_pw, RichText::new("Show password").color(c.text_hint()).size(t.typography.size_xs));

                ui.add_space(sp.lg);
                ui.horizontal(|ui| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(egui::Button::new(RichText::new("Authenticate").color(Color32::WHITE).size(t.typography.size_sm))
                            .fill(c.accent())
                            .rounding(Rounding::same(s.radius_sm))).clicked()
                        {
                            submit = true;
                        }
                        ui.add_space(sp.sm);
                        if ui.add(egui::Button::new(RichText::new("Cancel").color(c.text_secondary()).size(t.typography.size_sm))
                            .fill(c.bg_surface0())
                            .rounding(Rounding::same(s.radius_sm))).clicked()
                        {
                            state.pending_root_auth = None;
                            state.pending_root_wrong = false;
                        }
                    });
                });

                if submit && !password.is_empty() {
                    let pw_clone = password.clone();
                    let tx_clone = tx.clone();
                    let req_clone = req.clone();
                    let show_hidden = state.dir.show_hidden;

                    std::thread::spawn(move || {
                        use std::io::Write;
                        use std::process::{Command, Stdio};

                        // Özel durum: Dizin erişimi için root yetkisi
                        if let Some(ref atype) = req_clone.auth_type {
                            if atype == "become_root" {
                                // Sudo doğrulaması
                                let mut test_child = match Command::new("sudo")
                                    .args(["-S", "true"])
                                    .stdin(Stdio::piped())
                                    .stdout(Stdio::null())
                                    .stderr(Stdio::piped())
                                    .spawn()
                                {
                                    Ok(c) => c,
                                    Err(e) => {
                                        tx_clone.send(BackendMsg::RootAuthDone { success: false, message: format!("Spawn failed: {}", e), reload: false }).ok();
                                        return;
                                    }
                                };
                                if let Some(mut stdin) = test_child.stdin.take() {
                                    let _ = stdin.write_all(format!("{}\n", pw_clone).as_bytes());
                                }
                                if let Ok(out) = test_child.wait_with_output() {
                                    if out.status.success() {
                                        crate::backend::fs_ops::set_root_password(pw_clone.clone());
                                        tx_clone.send(BackendMsg::RootAuthDone {
                                            success: true,
                                            message: "Root yetkileri oturum için aktif edildi".to_string(),
                                            reload: req_clone.reload,
                                        }).ok();
                                    } else {
                                        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                                        if stderr.to_lowercase().contains("incorrect password")
                                            || stderr.to_lowercase().contains("try again")
                                            || stderr.to_lowercase().contains("yanlış")
                                        {
                                            tx_clone.send(BackendMsg::RootAuthWrongPassword).ok();
                                        } else {
                                            tx_clone.send(BackendMsg::RootAuthDone {
                                                success: false,
                                                message: format!("Yetkilendirme başarısız: {}", stderr.trim()),
                                                reload: false,
                                            }).ok();
                                        }
                                    }
                                }
                                return;
                            } else if let Some(target_dir) = atype.strip_prefix("dir_access:") {
                                match crate::backend::fs_ops::list_directory_with_sudo(target_dir.to_string(), show_hidden, &pw_clone) {
                                    Ok(listing) => {
                                        // Şifre doğru! Oturum boyunca hatırla ki alt klasörlere de rahatça girilsin
                                        crate::backend::fs_ops::set_root_password(pw_clone.clone());
                                        tx_clone.send(BackendMsg::DirectoryLoaded(listing)).ok();
                                        tx_clone.send(BackendMsg::RootAuthDone {
                                            success: true,
                                            message: format!("Yetki sağlandı: {}", target_dir),
                                            reload: false,
                                        }).ok();
                                        return;
                                    }
                                    Err(err) if err == "WRONG_PASSWORD" => {
                                        tx_clone.send(BackendMsg::RootAuthWrongPassword).ok();
                                        return;
                                    }
                                    Err(e) => {
                                        tx_clone.send(BackendMsg::RootAuthDone {
                                            success: false,
                                            message: format!("Dizin okunamadı: {}", e),
                                            reload: false,
                                        }).ok();
                                        return;
                                    }
                                }
                            }
                        }

                        // Genel sudo komutu çalıştırma
                        let is_large_rm = req_clone.cmd == "rm" && req_clone.args.len() > 500;
                        let run_result = if is_large_rm {
                            let flag = if req_clone.args.first().map_or(false, |a| a.starts_with('-')) {
                                Some(req_clone.args[0].clone())
                            } else {
                                None
                            };
                            let files = if flag.is_some() { &req_clone.args[1..] } else { &req_clone.args[..] };
                            let mut all_err = String::new();
                            let mut success = true;

                            for chunk in files.chunks(500) {
                                let mut sub_args = vec!["-S".to_string(), req_clone.cmd.clone()];
                                if let Some(ref f) = flag {
                                    sub_args.push(f.clone());
                                }
                                sub_args.extend_from_slice(chunk);

                                let mut child = match Command::new("sudo")
                                    .args(&sub_args)
                                    .stdin(Stdio::piped())
                                    .stdout(Stdio::null())
                                    .stderr(Stdio::piped())
                                    .spawn()
                                {
                                    Ok(c) => c,
                                    Err(e) => {
                                        all_err = format!("Spawn failed: {}", e);
                                        success = false;
                                        break;
                                    }
                                };
                                if let Some(mut stdin) = child.stdin.take() {
                                    let _ = stdin.write_all(format!("{}\n", pw_clone).as_bytes());
                                }
                                match child.wait_with_output() {
                                    Ok(out) => {
                                        if !out.status.success() {
                                            all_err = String::from_utf8_lossy(&out.stderr).to_string();
                                            success = false;
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        all_err = e.to_string();
                                        success = false;
                                        break;
                                    }
                                }
                            }
                            if success { Ok(()) } else { Err(all_err) }
                        } else {
                            let mut child = match Command::new("sudo")
                                .args(["-S", &req_clone.cmd])
                                .args(&req_clone.args)
                                .stdin(Stdio::piped())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::piped())
                                .spawn()
                            {
                                Ok(c) => c,
                                Err(e) => {
                                    tx_clone.send(BackendMsg::RootAuthDone { success: false, message: format!("Spawn failed: {}", e), reload: false }).ok();
                                    return;
                                }
                            };
                            if let Some(mut stdin) = child.stdin.take() {
                                let _ = stdin.write_all(format!("{}\n", pw_clone).as_bytes());
                            }
                            match child.wait_with_output() {
                                Ok(out) => {
                                    if out.status.success() {
                                        Ok(())
                                    } else {
                                        Err(String::from_utf8_lossy(&out.stderr).to_string())
                                    }
                                }
                                Err(e) => Err(e.to_string()),
                            }
                        };

                        match run_result {
                            Ok(()) => {
                                crate::backend::fs_ops::set_root_password(pw_clone.clone());
                                tx_clone.send(BackendMsg::RootAuthDone {
                                    success: true,
                                    message: format!("İşlem başarıyla tamamlandı: {}", req_clone.title),
                                    reload: req_clone.reload,
                                }).ok();
                            }
                            Err(stderr) => {
                                if stderr.to_lowercase().contains("incorrect password")
                                    || stderr.to_lowercase().contains("try again")
                                    || stderr.to_lowercase().contains("yanlış")
                                {
                                    tx_clone.send(BackendMsg::RootAuthWrongPassword).ok();
                                } else {
                                    tx_clone.send(BackendMsg::RootAuthDone {
                                        success: false,
                                        message: format!("Hata: {}", stderr.trim()),
                                        reload: false,
                                    }).ok();
                                }
                            }
                        }
                    });
                    password.clear();
                }
            });
        });

    ctx.data_mut(|d| {
        d.insert_temp(pass_id, password);
        d.insert_temp(show_id, show_pw);
    });

    if ctx.input(|i| i.key_pressed(Key::Escape)) {
        state.pending_root_auth = None;
        state.pending_root_wrong = false;
    }
}

// ── Fullscreen Video Overlay (arch-explorer-sorcerer style) ───────────────────

fn render_fullscreen_video(
    ctx: &egui::Context,
    _t: &crate::theme::types::AexTheme,
    state: &mut crate::state::AppState,
) {
    use egui::*;
    let screen = ctx.screen_rect();

    egui::Area::new(Id::new("fullscreen_video_overlay"))
        .fixed_pos(screen.min)
        .order(Order::Tooltip)
        .show(ctx, |ui| {
            // Pure black backdrop
            ui.painter().rect_filled(screen, Rounding::ZERO, Color32::BLACK);

            // Render live video frame centered, preserving aspect ratio (contain)
            let frame_opt = if let Ok(lock) = state.media_player.latest_frame.lock() {
                lock.as_ref().map(|f| (f.data.clone(), f.width, f.height))
            } else {
                None
            };

            if let Some((raw, fw, fh)) = frame_opt {
                let color_img = egui::ColorImage::from_rgba_unmultiplied([fw, fh], &raw);
                let tex_id = egui::Id::new("fs_video_tex_cache");
                let mut tex: Option<egui::TextureHandle> = ui.ctx().data_mut(|d| d.get_temp(tex_id));
                let tex_handle = match tex.as_mut() {
                    Some(existing) => {
                        existing.set(color_img, egui::TextureOptions::LINEAR);
                        existing.clone()
                    }
                    None => {
                        let created = ui.ctx().load_texture("fs_video_preview", color_img, egui::TextureOptions::LINEAR);
                        ui.ctx().data_mut(|d| d.insert_temp(tex_id, created.clone()));
                        created
                    }
                };

                let scale = (screen.width() / fw as f32).min(screen.height() / fh as f32);
                let dw = fw as f32 * scale;
                let dh = fh as f32 * scale;
                let video_rect = Rect::from_center_size(screen.center(), Vec2::new(dw, dh));

                ui.painter().image(
                    tex_handle.id(),
                    video_rect,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
            } else {
                ui.painter().text(
                    screen.center(),
                    Align2::CENTER_CENTER,
                    "🎬 Video Oynatılıyor...",
                    FontId::proportional(22.0),
                    Color32::WHITE,
                );
            }

            // Top bar with exit button
            let top_rect = Rect::from_min_size(
                screen.min + vec2(16.0, 16.0),
                vec2(screen.width() - 32.0, 40.0),
            );
            ui.allocate_ui_at_rect(top_rect, |ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let exit_btn = egui::Button::new(
                        RichText::new("✕ Çık (Esc)")
                            .color(Color32::WHITE)
                            .size(12.0)
                            .strong()
                    )
                    .fill(Color32::from_rgba_unmultiplied(30, 30, 40, 220))
                    .stroke(Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 60)))
                    .rounding(Rounding::same(6.0));

                    if ui.add(exit_btn).clicked() {
                        state.is_video_fullscreen = false;
                    }
                });
            });

            // Escape key exits fullscreen
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                state.is_video_fullscreen = false;
            }
        });
}

// ── File Conflict Dialog ──────────────────────────────────────────────────────

fn render_item_visual_badge(
    ui: &mut egui::Ui,
    t: &crate::theme::types::AexTheme,
    path_str: &str,
    is_dir: bool,
    size_px: f32,
) {
    use egui::*;
    let c = &t.colors;
    let s = &t.shape;
    let path = std::path::Path::new(path_str);
    let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
    let is_media = matches!(ext.as_str(),
        "png"|"jpg"|"jpeg"|"webp"|"bmp"|"gif"|"ico"|"svg"
        |"mp4"|"mkv"|"avi"|"mov"|"webm"
    );

    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size_px), Sense::hover());
    ui.painter().rect_filled(
        rect,
        Rounding::same(s.radius_sm),
        c.bg_surface1(),
    );
    ui.painter().rect_stroke(
        rect,
        Rounding::same(s.radius_sm),
        Stroke::new(1.0, c.border()),
    );

    if !is_dir && is_media && path.exists() {
        let cache_key = format!("conflict_thumb_{}", path_str);
        let thumb_id = Id::new(&cache_key);
        let handle: Option<TextureHandle> = ui.ctx().data_mut(|d| d.get_temp(thumb_id));

        if let Some(h) = handle {
            let tr = rect.shrink(3.0);
            ui.painter().image(
                h.id(),
                tr,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
            return;
        }

        // Asynchronously load thumbnail
        let req_id = Id::new(format!("treq_{}", cache_key));
        let loading: bool = ui.ctx().data_mut(|d| d.get_temp(req_id).unwrap_or(false));
        if !loading {
            ui.ctx().data_mut(|d| d.insert_temp(req_id, true));
            let path_c = path_str.to_string();
            let ctx_c = ui.ctx().clone();
            let k = cache_key.clone();
            std::thread::spawn(move || {
                if let Some(jpeg) = crate::backend::thumbnails::get_thumbnail(
                    &path_c,
                    crate::backend::thumbnails::ThumbQuality::Low,
                ) {
                    if let Some(ci) = crate::backend::thumbnails::jpeg_to_color_image(&jpeg) {
                        ctx_c.data_mut(|d| {
                            d.insert_temp(Id::new(format!("traw_{}", k)), ci);
                        });
                        ctx_c.request_repaint();
                    }
                }
            });
        }

        let raw: Option<ColorImage> = ui.ctx().data_mut(|d| d.get_temp(Id::new(format!("traw_{}", cache_key))));
        if let Some(ci) = raw {
            let h = ui.ctx().load_texture(
                format!("c_thumb_{}", path_str),
                ci,
                TextureOptions::LINEAR,
            );
            ui.ctx().data_mut(|d| {
                d.insert_temp(thumb_id, h);
                d.remove::<ColorImage>(Id::new(format!("traw_{}", cache_key)));
            });
            ui.ctx().request_repaint();
            let tr = rect.shrink(3.0);
            if let Some(h_loaded) = ui.ctx().data_mut(|d| d.get_temp::<TextureHandle>(thumb_id)) {
                ui.painter().image(
                    h_loaded.id(),
                    tr,
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                return;
            }
        }
    }

    // Fallback: icon badge
    let dummy_entry = crate::backend::fs_ops::FileEntry {
        name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path_str.to_string()),
        path: path_str.to_string(),
        is_dir,
        is_symlink: false,
        size: 0,
        modified: String::new(),
        created: String::new(),
        permissions: String::new(),
        mime_type: String::new(),
        extension: ext,
        hidden: false,
        is_executable: false,
    };
    let icon_str = crate::ui::file_icon(t, &dummy_entry);
    let icon_col = crate::ui::icon_color_for_entry(t, &dummy_entry);

    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon_str,
        FontId::new(size_px * 0.44, FontFamily::Monospace),
        icon_col,
    );
}

fn render_conflict_dialog(
    ctx: &egui::Context,
    t: &crate::theme::types::AexTheme,
    state: &mut AppState,
) {
    use egui::*;
    let c = &t.colors;
    let s = &t.shape;
    let sp = &t.spacing;

    let Some(diag) = state.conflict_dialog.as_ref() else { return; };
    let Some(item) = diag.queue.get(diag.current_idx).cloned() else { return; };

    let total = diag.queue.len();
    let current_num = diag.current_idx + 1;
    let file_name = std::path::Path::new(&item.dest_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| item.dest_path.clone());

    let screen = ctx.screen_rect();
    let pw = 560.0_f32;
    let ph = 370.0_f32;
    let d_rect = Rect::from_center_size(screen.center(), Vec2::new(pw, ph));

    // Backdrop
    egui::Area::new(Id::new("conflict_dialog_backdrop"))
        .fixed_pos(screen.min)
        .order(Order::Background)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, Rounding::ZERO, Color32::from_rgba_unmultiplied(0, 0, 0, 140));
        });

    let mut action_choice: Option<crate::state::ConflictChoice> = None;
    let mut cancel_clicked = false;
    let mut toggle_apply_all: Option<bool> = None;

    egui::Area::new(Id::new("conflict_dialog_modal"))
        .fixed_pos(d_rect.min)
        .order(Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame {
                fill: c.bg_mantle(),
                rounding: Rounding::same(s.radius_lg),
                stroke: Stroke::new(1.0_f32, c.border()),
                inner_margin: Margin::same(sp.lg),
                shadow: epaint::Shadow {
                    offset: Vec2::new(0.0, 10.0),
                    blur: 28.0,
                    spread: 0.0,
                    color: Color32::from_rgba_unmultiplied(0, 0, 0, 120),
                },
                ..Default::default()
            }.show(ui, |ui| {
                ui.set_width(pw - sp.lg * 2.0);

                // Header
                ui.horizontal(|ui| {
                    let title = if item.is_dir { "📁 Klasör Zaten Mevcut" } else { "⚠️ Dosya Zaten Mevcut" };
                    ui.label(RichText::new(title).color(c.warning()).size(t.typography.size_base).strong());

                    if total > 1 {
                        ui.label(RichText::new(format!("({}/{})", current_num, total)).color(c.text_secondary()).size(t.typography.size_xs));
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("✕").clicked() {
                            cancel_clicked = true;
                        }
                    });
                });

                ui.add_space(sp.xs);
                ui.label(
                    RichText::new(format!("Hedef konumda \"{}\" adlı bir {} zaten var.", file_name, if item.is_dir { "klasör" } else { "dosya" }))
                        .color(c.text_primary())
                        .size(t.typography.size_sm)
                );

                ui.add_space(sp.md);

                // Comparison Box
                egui::Frame {
                    fill: c.bg_surface0(),
                    rounding: Rounding::same(s.radius_md),
                    stroke: Stroke::new(1.0_f32, c.border()),
                    inner_margin: Margin::same(sp.md),
                    ..Default::default()
                }.show(ui, |ui| {
                    ui.columns(2, |cols| {
                        // Source
                        cols[0].vertical(|ui| {
                            ui.label(RichText::new("Kaynak (Aktarılan):").strong().color(c.accent()).size(t.typography.size_xs));
                            ui.add_space(sp.xs);
                            ui.horizontal(|ui| {
                                render_item_visual_badge(ui, t, &item.src_path, item.is_dir, 48.0);
                                ui.vertical(|ui| {
                                    if !item.is_dir {
                                        ui.label(RichText::new(format!("Boyut: {}", crate::ui::human_size(item.src_size))).color(c.text_secondary()).size(t.typography.size_xs));
                                    } else {
                                        ui.label(RichText::new("Klasör").color(c.text_secondary()).size(t.typography.size_xs));
                                    }
                                    ui.label(RichText::new(format!("Değiştirilme:\n{}", item.src_modified)).color(c.text_secondary()).size(t.typography.size_xs));
                                });
                            });
                        });

                        // Destination
                        cols[1].vertical(|ui| {
                            ui.label(RichText::new("Hedef (Mevcut Olan):").strong().color(c.text_primary()).size(t.typography.size_xs));
                            ui.add_space(sp.xs);
                            ui.horizontal(|ui| {
                                render_item_visual_badge(ui, t, &item.dest_path, item.is_dir, 48.0);
                                ui.vertical(|ui| {
                                    if !item.is_dir {
                                        ui.label(RichText::new(format!("Boyut: {}", crate::ui::human_size(item.dest_size))).color(c.text_secondary()).size(t.typography.size_xs));
                                    } else {
                                        ui.label(RichText::new("Klasör").color(c.text_secondary()).size(t.typography.size_xs));
                                    }
                                    ui.label(RichText::new(format!("Değiştirilme:\n{}", item.dest_modified)).color(c.text_secondary()).size(t.typography.size_xs));
                                });
                            });
                        });
                    });
                });

                ui.add_space(sp.md);

                // Checkbox: Apply to all
                let mut apply_val = diag.apply_to_all;
                if ui.checkbox(&mut apply_val, RichText::new("Bundan sonraki tüm çakışmalarda bu ayarı uygula").color(c.text_primary()).size(t.typography.size_sm)).changed() {
                    toggle_apply_all = Some(apply_val);
                }

                ui.add_space(sp.lg);

                // Action buttons
                ui.horizontal(|ui| {
                    if ui.add(
                        egui::Button::new(RichText::new("Üzerine Yaz").color(Color32::WHITE).size(t.typography.size_sm).strong())
                            .fill(c.accent())
                            .rounding(Rounding::same(s.radius_sm))
                    ).on_hover_text("Mevcut dosyanın/klasörün üzerine yazar").clicked() {
                        action_choice = Some(crate::state::ConflictChoice::Replace);
                    }

                    ui.add_space(sp.xs);

                    if ui.add(
                        egui::Button::new(RichText::new("Her İkisini Koru").color(c.text_primary()).size(t.typography.size_sm))
                            .fill(c.bg_surface1())
                            .rounding(Rounding::same(s.radius_sm))
                    ).on_hover_text("Yeni dosyayı/klasörü (1), (2) şeklinde yeniden adlandırır").clicked() {
                        action_choice = Some(crate::state::ConflictChoice::KeepBoth);
                    }

                    ui.add_space(sp.xs);

                    if ui.add(
                        egui::Button::new(RichText::new("Atla").color(c.text_secondary()).size(t.typography.size_sm))
                            .fill(c.bg_surface0())
                            .rounding(Rounding::same(s.radius_sm))
                    ).on_hover_text("Bu öğeyi aktarmadan bir sonrakine geçer").clicked() {
                        action_choice = Some(crate::state::ConflictChoice::Skip);
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(RichText::new("İptal").color(c.danger()).size(t.typography.size_sm))
                                .fill(c.bg_surface0())
                                .rounding(Rounding::same(s.radius_sm))
                        ).clicked() {
                            cancel_clicked = true;
                        }
                    });
                });
            });
        });

    if let Some(val) = toggle_apply_all {
        if let Some(d) = state.conflict_dialog.as_mut() {
            d.apply_to_all = val;
        }
    }

    if cancel_clicked || ctx.input(|i| i.key_pressed(Key::Escape)) {
        state.cancel_conflict_dialog();
    } else if let Some(choice) = action_choice {
        state.resolve_current_conflict(choice);
    }
}

// ── Delete Error Dialog ───────────────────────────────────────────────────────

fn render_delete_error_dialog(
    ctx: &egui::Context,
    t: &crate::theme::types::AexTheme,
    state: &mut AppState,
) {
    use egui::*;
    let c = &t.colors;
    let s = &t.shape;
    let sp = &t.spacing;

    let Some(err_diag) = state.delete_error_dialog.as_ref().cloned() else { return; };

    let screen = ctx.screen_rect();
    let pw = 480.0_f32;
    let ph = 260.0_f32;
    let d_rect = Rect::from_center_size(screen.center(), Vec2::new(pw, ph));

    // Backdrop
    egui::Area::new(Id::new("delete_error_backdrop"))
        .fixed_pos(screen.min)
        .order(Order::Background)
        .show(ctx, |ui| {
            ui.painter().rect_filled(screen, Rounding::ZERO, Color32::from_rgba_unmultiplied(0, 0, 0, 140));
        });

    let mut action_retry_root = false;
    let mut action_skip = false;
    let mut action_cancel = false;

    egui::Area::new(Id::new("delete_error_modal"))
        .fixed_pos(d_rect.min)
        .order(Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame {
                fill: c.bg_mantle(),
                rounding: Rounding::same(s.radius_lg),
                stroke: Stroke::new(1.0_f32, c.danger()),
                inner_margin: Margin::same(sp.lg),
                shadow: epaint::Shadow {
                    offset: Vec2::new(0.0, 10.0),
                    blur: 28.0,
                    spread: 0.0,
                    color: Color32::from_rgba_unmultiplied(0, 0, 0, 120),
                },
                ..Default::default()
            }.show(ui, |ui| {
                ui.set_width(pw - sp.lg * 2.0);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("🚫 Silme Hatası").color(c.danger()).size(t.typography.size_base).strong());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button("✕").clicked() {
                            action_cancel = true;
                        }
                    });
                });

                ui.add_space(sp.xs);
                ui.label(
                    RichText::new(format!("\"{}\" silinirken bir hata oluştu:", err_diag.failed_path))
                        .color(c.text_primary())
                        .size(t.typography.size_sm)
                );

                ui.add_space(sp.xs);
                egui::Frame {
                    fill: c.bg_surface0(),
                    rounding: Rounding::same(s.radius_sm),
                    inner_margin: Margin::same(sp.xs),
                    ..Default::default()
                }.show(ui, |ui| {
                    ui.label(RichText::new(&err_diag.error_msg).color(c.danger()).size(t.typography.size_xs));
                });

                ui.add_space(sp.lg);

                ui.horizontal(|ui| {
                    if ui.add(
                        egui::Button::new(RichText::new("🔐 Root Yetkisiyle Dene").color(Color32::WHITE).size(t.typography.size_sm).strong())
                            .fill(c.accent())
                            .rounding(Rounding::same(s.radius_sm))
                    ).clicked() {
                        action_retry_root = true;
                    }

                    ui.add_space(sp.xs);

                    if !err_diag.remaining_paths.is_empty() {
                        if ui.add(
                            egui::Button::new(RichText::new("Bu Dosyayı Atla").color(c.text_secondary()).size(t.typography.size_sm))
                                .fill(c.bg_surface0())
                                .rounding(Rounding::same(s.radius_sm))
                        ).clicked() {
                            action_skip = true;
                        }
                    }

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.add(
                            egui::Button::new(RichText::new("İptal").color(c.text_secondary()).size(t.typography.size_sm))
                                .fill(c.bg_surface0())
                                .rounding(Rounding::same(s.radius_sm))
                        ).clicked() {
                            action_cancel = true;
                        }
                    });
                });
            });
        });

    if action_retry_root {
        let mut paths_to_elevate = vec![err_diag.failed_path.clone()];
        paths_to_elevate.extend(err_diag.remaining_paths.clone());
        state.delete_error_dialog = None;

        let mut args = vec!["-rf".to_string()];
        args.extend(paths_to_elevate);
        let title = if err_diag.permanent { "Kalıcı Olarak Sil (Root)" } else { "Dosyaları Sil (Root)" };

        // 1. Try pkexec
        let status = std::process::Command::new("pkexec")
            .arg("rm")
            .args(&args)
            .status();
        match status {
            Ok(s) if s.success() => {
                state.notify.success("Dosyalar root yetkisiyle silindi".into());
                state.refresh();
                return;
            }
            _ => {}
        }

        // 2. Fallback to sudo password dialog
        state.pending_root_auth = Some(crate::state::RootAuthRequest {
            title: title.to_string(),
            cmd: "rm".to_string(),
            args,
            reload: true,
            auth_type: None,
        });
        state.pending_root_wrong = false;
    } else if action_skip {
        let remaining = err_diag.remaining_paths.clone();
        let perm = err_diag.permanent;
        state.delete_error_dialog = None;
        crate::ui::context_menu::try_delete_or_elevate(state, remaining, perm);
    } else if action_cancel || ctx.input(|i| i.key_pressed(Key::Escape)) {
        state.delete_error_dialog = None;
        state.notify.info("Silme işlemi durduruldu".into());
    }
}
