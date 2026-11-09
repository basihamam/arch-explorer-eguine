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
}
