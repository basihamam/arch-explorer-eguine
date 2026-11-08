// watcher.rs — inotify file watcher
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Duration;

pub fn watch_directory(path: &str, tx: Sender<String>) -> anyhow::Result<RecommendedWatcher> {
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                for p in &event.paths {
                    let dir = if p.is_dir() {
                        p.to_string_lossy().to_string()
                    } else if let Some(parent) = p.parent() {
                        parent.to_string_lossy().to_string()
                    } else {
                        p.to_string_lossy().to_string()
                    };
                    tx.send(dir).ok();
                    if let Some(parent) = p.parent() {
                        tx.send(parent.to_string_lossy().to_string()).ok();
                    }
                }
            }
        },
        Config::default().with_poll_interval(Duration::from_millis(100)),
    )?;

    watcher.watch(Path::new(path), RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::sync::mpsc::channel;

    #[test]
    fn test_watch_directory_file_creation() {
        let unique = format!("aex_watch_test_file_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let temp_path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_path).expect("create_dir_all failed");
        let dir_path = temp_path.to_str().unwrap().to_string();

        let (tx, rx) = channel::<String>();
        let _watcher = watch_directory(&dir_path, tx).expect("watcher setup failed");

        let file_path = temp_path.join("new_file.txt");
        let mut f = File::create(&file_path).expect("file create failed");
        writeln!(f, "hello auto refresh").expect("write failed");

        // inotify event should arrive and contain the watched directory path
        let mut detected = false;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(3) {
            if let Ok(received) = rx.recv_timeout(std::time::Duration::from_millis(100)) {
                if received == dir_path {
                    detected = true;
                    break;
                }
            }
        }
        let _ = std::fs::remove_dir_all(&temp_path);
        assert!(detected, "Inotify did not notify containing directory upon file creation");
    }

    #[test]
    fn test_watch_directory_folder_creation() {
        let unique = format!("aex_watch_test_dir_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
        let temp_path = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&temp_path).expect("create_dir_all failed");
        let dir_path = temp_path.to_str().unwrap().to_string();

        let (tx, rx) = channel::<String>();
        let _watcher = watch_directory(&dir_path, tx).expect("watcher setup failed");

        let subfolder = temp_path.join("new_subfolder");
        std::fs::create_dir(&subfolder).expect("mkdir failed");

        let mut detected = false;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(3) {
            if let Ok(received) = rx.recv_timeout(std::time::Duration::from_millis(100)) {
                if received == dir_path {
                    detected = true;
                    break;
                }
            }
        }
        let _ = std::fs::remove_dir_all(&temp_path);
        assert!(detected, "Inotify did not notify containing directory upon folder creation");
    }
}
