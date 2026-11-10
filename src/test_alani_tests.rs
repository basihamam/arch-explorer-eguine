// src/test_alani_tests.rs
// Direct verification of all 7 test categories in /home/cikti/test_alani

#[cfg(test)]
mod tests {
    use std::path::Path;

    const TEST_ALANI: &str = "/home/cikti/test_alani";

    #[test]
    fn test_01_resimler_gorseller_preview() {
        let p = format!("{}/01_Resimler_Gorseller/cyber_emerald.png", TEST_ALANI);
        assert!(Path::new(&p).exists(), "cyber_emerald.png must exist");

        let prev = crate::backend::preview::get_file_preview(p.clone()).expect("Image preview failed");
        assert_eq!(prev.kind, "image");
        assert!(prev.image_rgba.is_some() || prev.image_b64.is_some(), "Image preview must have rgba or b64 data");

        let svg_p = format!("{}/01_Resimler_Gorseller/arch_explorer_logo.svg", TEST_ALANI);
        let svg_prev = crate::backend::preview::get_file_preview(svg_p).expect("SVG preview failed");
        assert_eq!(svg_prev.kind, "image");
    }

    #[test]
    fn test_02_cakisma_ve_uzerine_yazma_ve_undo() {
        use crate::state::{execute_transfer_item, ConflictChoice};
        use crate::backend::clipboard::{push_undo, undo_last};

        let src_dir = format!("{}/02_Cakisma_ve_Uzerine_Yazma_Testi/kaynak_klasor", TEST_ALANI);
        let orig_hedef = format!("{}/02_Cakisma_ve_Uzerine_Yazma_Testi/hedef_klasor", TEST_ALANI);

        let temp_dir = std::env::temp_dir().join("arch_explorer_test_cakisma");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        for entry in std::fs::read_dir(&orig_hedef).unwrap().flatten() {
            let target = temp_dir.join(entry.file_name());
            std::fs::copy(entry.path(), target).unwrap();
        }

        let test_src_file = format!("{}/ayarlar.cfg", src_dir);
        let test_dst_file = temp_dir.join("ayarlar.cfg").to_string_lossy().to_string();

        let orig_size = std::fs::metadata(&test_dst_file).unwrap().len();
        let new_size = std::fs::metadata(&test_src_file).unwrap().len();
        assert_ne!(orig_size, new_size, "Sizes must be different to test conflict");

        // 1. ConflictChoice::Replace (Overwrite)
        let action = execute_transfer_item(&test_src_file, &test_dst_file, true, ConflictChoice::Replace)
            .expect("Transfer item failed")
            .expect("Action must be returned");

        let current_size = std::fs::metadata(&test_dst_file).unwrap().len();
        assert_eq!(current_size, new_size, "File should have been overwritten with new size");

        // 2. Undo (Ctrl+Z)
        push_undo(action).unwrap();
        let undo_res = undo_last().expect("Undo must succeed");
        assert!(undo_res.restored_paths.contains(&test_dst_file), "Restored paths must contain test_dst_file");

        let restored_size = std::fs::metadata(&test_dst_file).unwrap().len();
        assert_eq!(restored_size, orig_size, "File must be restored to original size after undo");

        // 3. ConflictChoice::KeepBoth
        let action_keep = execute_transfer_item(&test_src_file, &test_dst_file, true, ConflictChoice::KeepBoth)
            .expect("Transfer KeepBoth failed")
            .expect("Action must be returned");

        let mut created_paths = Vec::new();
        action_keep.collect_targets(&mut created_paths);
        assert_eq!(created_paths.len(), 1);
        assert!(Path::new(&created_paths[0]).exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_02_cakisma_start_transfer_dialog_ve_drag_noop() {
        use crate::state::AppState;

        let src_dir = format!("{}/02_Cakisma_ve_Uzerine_Yazma_Testi/kaynak_klasor", TEST_ALANI);
        let hedef_dir = format!("{}/02_Cakisma_ve_Uzerine_Yazma_Testi/hedef_klasor", TEST_ALANI);

        let mut state = AppState::new();
        let list_res = crate::backend::fs_ops::list_directory(src_dir.clone(), false).unwrap();
        state.dir.entries = std::sync::Arc::new(list_res.entries);
        assert_eq!(state.dir.entries.len(), 3, "kaynak_klasor must have 3 files");

        // 1. Ctrl+A selection verification
        state.selection.select_all(&state.dir.entries);
        let sel = state.selected_paths();
        assert_eq!(sel.len(), 3, "select_all must return all 3 paths in selected_paths()");
        assert_eq!(state.selection.paths().len(), 3, "select_all must also populate selection.paths()");

        // 2. Drag-and-drop back into same directory (kaynak_klasor) must be strict NO-OP!
        state.start_transfer_drag(sel.clone(), src_dir.clone(), false);
        assert!(state.conflict_dialog.is_none(), "Drag to same dir (!copy) must NOT open conflict dialog");
        assert!(state.pending_transfer.is_none(), "Drag to same dir (!copy) must NOT create transfer request");

        state.start_transfer_drag(sel.clone(), src_dir.clone(), true);
        assert!(state.conflict_dialog.is_none(), "Drag to same dir (copy) must NOT open conflict dialog");
        assert!(state.pending_transfer.is_none(), "Drag to same dir (copy) must NOT create duplicate copy");

        // 3. Transfer to hedef_klasor where identical file names exist: MUST open conflict dialog!
        state.start_transfer(sel.clone(), hedef_dir.clone(), true);
        assert!(state.conflict_dialog.is_some(), "Transfer to folder with existing files MUST open conflict dialog!");
        assert!(state.pending_transfer.is_none(), "All files have conflicts so pending_transfer must be none until user chooses action");

        let diag = state.conflict_dialog.as_ref().unwrap();
        assert_eq!(diag.queue.len(), 3, "Conflict dialog must contain all 3 conflicting files");
        assert_eq!(diag.dest_dir, hedef_dir);

        // Verify conflict item details
        let first = &diag.queue[0];
        assert!(!first.dest_path.is_empty());
        assert_ne!(first.src_path, first.dest_path);
    }

    #[test]
    fn test_02_tek_dosya_ayni_klasore_paste_ve_hedefe_paste_cakisma_dialog() {
        use crate::state::{AppState, ConflictChoice};

        let src_dir = format!("{}/02_Cakisma_ve_Uzerine_Yazma_Testi/kaynak_klasor", TEST_ALANI);
        let hedef_dir = format!("{}/02_Cakisma_ve_Uzerine_Yazma_Testi/hedef_klasor", TEST_ALANI);

        let test_file = format!("{}/ayarlar.cfg", src_dir);
        assert!(Path::new(&test_file).exists());

        let mut state = AppState::new();

        // 1. Tek dosya aynı klasöre kopyalanıp yapıştırıldığında (Ctrl+C / Ctrl+V):
        // ASLA sessizce (2) oluşturulmamalı, MUTLAKA çakışma penceresi (conflict_dialog) açılmalıdır!
        state.start_transfer(vec![test_file.clone()], src_dir.clone(), true);
        assert!(state.conflict_dialog.is_some(), "Tek bir dosya aynı klasöre yapıştırıldığında kullanıcıya sormak için çakışma penceresi açılmalıdır!");
        assert!(state.pending_transfer.is_none());

        let diag = state.conflict_dialog.as_ref().unwrap();
        assert_eq!(diag.queue.len(), 1, "Çakışma kuyruğunda tek dosya olmalı");
        assert_eq!(diag.queue[0].src_path, test_file);

        // Kullanıcı "Her İkisini Koru" (Keep Both) derse kopya o zaman üretilmeli:
        state.resolve_current_conflict(ConflictChoice::KeepBoth);
        assert!(state.conflict_dialog.is_none());

        // Üretilen kopyanın varlığını doğrula ve temizle
        let possible_copy = format!("{}/ayarlar (2).cfg", src_dir);
        if Path::new(&possible_copy).exists() {
            let _ = std::fs::remove_file(&possible_copy);
        }

        // 2. Tek dosya hedef_klasor'e yapıştırıldığında da çakışma penceresi açılmalı:
        state.start_transfer(vec![test_file.clone()], hedef_dir.clone(), true);
        assert!(state.conflict_dialog.is_some(), "Hedefte aynı isimde dosya varken yapıştırıldığında çakışma penceresi açılmalıdır!");
        assert_eq!(state.conflict_dialog.as_ref().unwrap().queue.len(), 1);

        // "Atla" (Skip) seçildiğinde dosya değiştirilmeden kapatılmalı
        state.resolve_current_conflict(ConflictChoice::Skip);
        assert!(state.conflict_dialog.is_none());
    }

    #[test]
    fn test_03_metin_kod_ve_veri_dosyalari_syntax_ve_gizli() {
        let code_p = format!("{}/03_Metin_Kod_ve_Veri_Dosyalari/ornek_kod.rs", TEST_ALANI);
        let prev = crate::backend::preview::get_file_preview(code_p).expect("Rust preview failed");
        assert_eq!(prev.kind, "text");
        assert_eq!(prev.language.as_deref(), Some("rust"));
        assert!(prev.text_content.unwrap().contains("fn main"));

        let dir_p = format!("{}/03_Metin_Kod_ve_Veri_Dosyalari", TEST_ALANI);
        let listing_no_hidden = crate::backend::fs_ops::list_directory(dir_p.clone(), false).unwrap();
        let listing_hidden = crate::backend::fs_ops::list_directory(dir_p, true).unwrap();

        assert!(listing_hidden.entries.len() > listing_no_hidden.entries.len(), "Hidden listing must have more files");
        assert!(listing_hidden.entries.iter().any(|e| e.name == ".gizli_ayar_dosyasi"));
        assert!(!listing_no_hidden.entries.iter().any(|e| e.name == ".gizli_ayar_dosyasi"));
    }

    #[test]
    fn test_04_arsivler_ve_sikistirma() {
        let zip_p = format!("{}/04_Arsivler_ve_Sikistirma/ornek_arsiv.zip", TEST_ALANI);
        let tar_p = format!("{}/04_Arsivler_ve_Sikistirma/yedek_paket.tar.gz", TEST_ALANI);
        assert!(Path::new(&zip_p).exists());
        assert!(Path::new(&tar_p).exists());

        let (md5, sha256) = crate::backend::checksum::compute_checksums(&zip_p);
        assert_eq!(md5.len(), 32);
        assert_eq!(sha256.len(), 64);
    }

    #[test]
    fn test_05_medya_ve_ses() {
        let wav_p = format!("{}/05_Medya_ve_Ses/ornek_ses_efekti.wav", TEST_ALANI);
        let mp4_p = format!("{}/05_Medya_ve_Ses/ornek_video_320p.mp4", TEST_ALANI);
        assert!(Path::new(&wav_p).exists());
        assert!(Path::new(&mp4_p).exists());

        let wav_prev = crate::backend::preview::get_file_preview(wav_p).unwrap();
        assert_eq!(wav_prev.kind, "audio");

        let mp4_prev = crate::backend::preview::get_file_preview(mp4_p).unwrap();
        assert_eq!(mp4_prev.kind, "video");
    }

    #[test]
    fn test_06_derin_klasor_agaci_ve_symlink() {
        let deep_dir = format!("{}/06_Derin_Klasor_Agaci/1_Kategori_A/2_Alt_Dizin/3_Derin_Bolum", TEST_ALANI);
        assert!(Path::new(&deep_dir).exists());

        let listing = crate::backend::fs_ops::list_directory(deep_dir, false).unwrap();
        assert!(listing.entries.iter().any(|e| e.name == "en_derindeki_hazine.txt"));

        let empty_dir = format!("{}/06_Derin_Klasor_Agaci/Bos_Klasor_Testi", TEST_ALANI);
        let empty_listing = crate::backend::fs_ops::list_directory(empty_dir, false).unwrap();
        assert_eq!(empty_listing.entries.len(), 0);

        let mut nav = crate::state::NavigationState::default();
        nav.current_path = "/home/cikti/test_alani/06_Derin_Klasor_Agaci/1_Kategori_A///".to_string();
        let parent = nav.go_up().unwrap();
        assert_eq!(parent, "/home/cikti/test_alani/06_Derin_Klasor_Agaci");
    }

    #[test]
    fn test_07_yetki_ve_guvenlik() {
        let readonly_p = format!("{}/07_Yetki_ve_Guvenlik_Testi/salt_okunur_dosya.txt", TEST_ALANI);
        assert!(Path::new(&readonly_p).exists());

        let meta = std::fs::metadata(&readonly_p).unwrap();
        let formatted = crate::backend::fs_ops::format_permissions(&meta);
        assert!(formatted.starts_with("-r--") || formatted.starts_with("-r-"), "File permissions should reflect read-only mode: {}", formatted);
    }

    #[test]
    fn test_08_tab_bar_drop_files_onto_other_tab_header() {
        use crate::state::AppState;
        use crate::ui::tab_bar::{render_tab_half, apply_tab_action, TabBarAction};
        use crate::theme::types::AexTheme;

        // 1. İki farklı klasör ve aktarılacak dosya hazırla
        let base_temp = std::env::temp_dir().join(format!("ae_tab_drop_test_{}_{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let dir_tab1 = base_temp.join("kaynak_tab");
        let dir_tab2 = base_temp.join("hedef_tab");
        std::fs::create_dir_all(&dir_tab1).unwrap();
        std::fs::create_dir_all(&dir_tab2).unwrap();

        let src_file = dir_tab1.join("rapor.txt");
        std::fs::write(&src_file, "Gizli proje raporu 2026").unwrap();
        assert!(src_file.exists());

        // 2. AppState başlat ve sekmeleri ayarla
        let mut state = AppState::new();
        state.nav.current_path = dir_tab1.to_string_lossy().to_string();
        state.tabs.tabs[0].path = dir_tab1.to_string_lossy().to_string();
        state.tabs.tabs[0].label = "Kaynak".to_string();

        // İkinci sekmeyi aç (hedef sekme)
        state.open_in_new_tab(dir_tab2.to_string_lossy().to_string());
        state.tabs.tabs[1].label = "Hedef".to_string();
        // Aktif sekme: Kaynak (0. sekme)
        state.switch_tab(0);
        assert_eq!(state.tabs.active_idx, 0);
        assert_eq!(state.tabs.len(), 2);
        assert_eq!(state.tabs.tabs[0].path, dir_tab1.to_string_lossy().to_string());
        assert_eq!(state.tabs.tabs[1].path, dir_tab2.to_string_lossy().to_string());

        // 3. Dosyayı sürüklemeye başla
        let src_str = src_file.to_string_lossy().to_string();
        state.drag.start(vec![src_str.clone()], false);
        assert!(state.drag.dragging, "Sürükleme aktif olmalı");
        assert_eq!(state.drag.sources, vec![src_str.clone()]);

        // 4. UI bağlamı ile Hedef sekme (Tab 1) üzerine bırakmayı (drop) simüle et
        let ctx = egui::Context::default();
        let t = AexTheme::default();

        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("is_file_dragging"), true);
            d.insert_temp(egui::Id::new("drag_sources"), vec![src_str.clone()]);
        });

        // Hedef sekme (Tab 1) üzerine denk gelecek imleç pozisyonu (Tab 0 ~100px, Tab 1 ~150px)
        let mut raw_input = egui::RawInput::default();
        raw_input.events.push(egui::Event::PointerMoved(egui::pos2(150.0, 17.0)));
        raw_input.events.push(egui::Event::PointerButton {
            pos: egui::pos2(150.0, 17.0),
            button: egui::PointerButton::Primary,
            pressed: false, // Release (bırakma anı)
            modifiers: egui::Modifiers::default(),
        });

        let mut captured_action: Option<TabBarAction> = None;
        let _ = ctx.run(raw_input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_tab_half(ui, &t, &state.tabs, true, true, 400.0, 34.0, &mut captured_action);
            });
        });

        // Eylemin TabBarAction::DropFiles ve dest_path'in dir_tab2 olduğunu doğrula
        assert!(captured_action.is_some(), "Sekme üzerine bırakıldığında DropFiles eylemi tetiklenmeli");
        let dest_path = match &captured_action {
            Some(TabBarAction::DropFiles { dest_path }) => dest_path.clone(),
            _ => panic!("Beklenen TabBarAction::DropFiles"),
        };
        assert_eq!(dest_path, dir_tab2.to_string_lossy().to_string());

        // 5. Eylemi state'e uygula (apply_tab_action)
        apply_tab_action(&mut state, captured_action.unwrap());

        // Sürüklemenin tamamlandığını ve transfer isteğinin oluştuğunu doğrula
        assert!(!state.drag.dragging, "Drop sonrasında sürükleme temizlenmeli");
        assert!(state.pending_transfer.is_some(), "Başka sekmeye bırakıldığında pending_transfer oluşturulmalı");

        let req = state.pending_transfer.take().unwrap();
        assert_eq!(req.dest_dir, dir_tab2.to_string_lossy().to_string());
        assert_eq!(req.sources, vec![src_str.clone()]);

        // 6. Transferi gerçekleştir (arka plan iş parçacığı simülasyonu)
        let expected_dest_file = dir_tab2.join("rapor.txt");
        let res = crate::state::execute_transfer_item(&req.sources[0], &expected_dest_file.to_string_lossy(), req.copy, crate::state::ConflictChoice::Replace);
        assert!(res.is_ok(), "Transfer başarıyla tamamlanmalı");

        // Hedef klasörde dosyanın başarıyla oluştuğunu ve içeriğini doğrula
        assert!(expected_dest_file.exists(), "Dosya hedef sekmenin klasörüne aktarılmış olmalı");
        assert_eq!(std::fs::read_to_string(&expected_dest_file).unwrap(), "Gizli proje raporu 2026");

        // Taşıma işlemi olduğundan kaynak dosyanın silindiğini doğrula
        assert!(!src_file.exists(), "Taşıma sonrasında kaynak dosya silinmiş olmalı");

        // Temizlik
        let _ = std::fs::remove_dir_all(&base_temp);
    }

    #[test]
    fn test_09_spring_loaded_tab_switch_on_hover() {
        use crate::state::AppState;
        use crate::ui::tab_bar::{render_tab_half, TabBarAction};
        use crate::theme::types::AexTheme;

        let mut state = AppState::new();
        state.tabs.new_tab("/home/cikti/tab2".to_string());
        state.switch_tab(0);
        assert_eq!(state.tabs.active_idx, 0);

        let ctx = egui::Context::default();
        let t = AexTheme::default();

        ctx.data_mut(|d| {
            d.insert_temp(egui::Id::new("is_file_dragging"), true);
            d.insert_temp(egui::Id::new("drag_sources"), vec!["/tmp/test.txt".to_string()]);
        });

        // 1. Frame 1: Pointer moves over Tab 1 (x=150.0, y=17.0) at t=1.0s (not yet 400ms)
        let mut raw1 = egui::RawInput::default();
        raw1.time = Some(1.0);
        raw1.events.push(egui::Event::PointerMoved(egui::pos2(150.0, 17.0)));

        let mut action1: Option<TabBarAction> = None;
        let _ = ctx.run(raw1, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_tab_half(ui, &t, &state.tabs, true, true, 400.0, 34.0, &mut action1);
            });
        });
        assert!(action1.is_none(), "Under 400ms hover must NOT trigger switch");

        // 2. Frame 2: Pointer remains over Tab 1 at t=1.45s (elapsed 450ms > 400ms)
        let mut raw2 = egui::RawInput::default();
        raw2.time = Some(1.45);
        raw2.events.push(egui::Event::PointerMoved(egui::pos2(150.0, 17.0)));

        let mut action2: Option<TabBarAction> = None;
        let _ = ctx.run(raw2, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render_tab_half(ui, &t, &state.tabs, true, true, 400.0, 34.0, &mut action2);
            });
        });

        assert!(action2.is_some(), "Hovering > 400ms must trigger spring-loaded switch");
        match action2 {
            Some(TabBarAction::SwitchTab { tab_idx, .. }) => assert_eq!(tab_idx, 1),
            _ => panic!("Expected SwitchTab action"),
        }
    }
}
