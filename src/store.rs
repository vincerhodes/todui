use crate::model::Store;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub struct StoreFile {
    pub store: Store,
    pub recovered: bool,
    path: PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl StoreFile {
    pub fn load() -> io::Result<Self> {
        let path = data_file_path();
        if !path.exists() {
            return Ok(Self {
                store: Store::new(),
                recovered: false,
                path,
            });
        }

        let contents = fs::read_to_string(&path)?;
        match serde_json::from_str::<Store>(&contents) {
            Ok(store) => Ok(Self {
                store,
                recovered: false,
                path,
            }),
            Err(_) => {
                let bak = bak_path(&path);
                let _ = fs::remove_file(&bak);
                fs::rename(&path, &bak)?;
                Ok(Self {
                    store: Store::new(),
                    recovered: true,
                    path,
                })
            }
        }
    }

    pub fn save(&mut self) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let tmp_path = tmp_path_for(&self.path);
        let json = serde_json::to_string_pretty(&self.store)?;
        {
            let mut file = File::create(&tmp_path)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
        }
        fs::rename(&tmp_path, &self.path)?;
        Ok(())
    }
}

fn bak_path(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.bak", path.display()))
}

fn tmp_path_for(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.tmp", path.display()))
}

fn data_file_path() -> PathBuf {
    if let Ok(p) = std::env::var("TODUI_DATA_FILE") {
        return PathBuf::from(p);
    }
    directories::ProjectDirs::from("dev", "todui", "todui")
        .expect("valid project dirs")
        .data_dir()
        .join("todos.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Task;
    use chrono::{NaiveDate, Utc};
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_lock() -> &'static Mutex<()> {
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        prev: Option<String>,
    }

    impl EnvGuard {
        fn set_data_file(path: &Path) -> Self {
            let lock = env_lock().lock().unwrap();
            let prev = std::env::var("TODUI_DATA_FILE").ok();
            std::env::set_var("TODUI_DATA_FILE", path);
            Self {
                _lock: lock,
                prev,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => std::env::set_var("TODUI_DATA_FILE", v),
                None => std::env::remove_var("TODUI_DATA_FILE"),
            }
        }
    }

    #[test]
    fn load_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("todos.json");
        let _guard = EnvGuard::set_data_file(&path);

        let sf = StoreFile::load().unwrap();
        assert!(!sf.recovered);
        assert_eq!(sf.store.version, 1);
        assert!(sf.store.tasks.is_empty());
    }

    #[test]
    fn load_corrupt_creates_bak() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("todos.json");
        fs::write(&path, b"not json").unwrap();
        let _guard = EnvGuard::set_data_file(&path);

        let sf = StoreFile::load().unwrap();
        assert!(sf.recovered);
        assert!(sf.store.tasks.is_empty());
        assert!(!path.exists());
        let bak = bak_path(&path);
        assert!(bak.exists());
    }

    #[test]
    fn save_load_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("todos.json");
        let _guard = EnvGuard::set_data_file(&path);

        let mut sf = StoreFile::load().unwrap();
        sf.store.tasks.push(Task {
            id: 1,
            title: "test task".into(),
            project: Some("proj".into()),
            contexts: vec!["ctx".into()],
            priority: None,
            due: Some(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap()),
            notes: None,
            next_action: true,
            recurrence: None,
            created: Utc::now(),
            completed: None,
        });
        sf.save().unwrap();

        let loaded = StoreFile::load().unwrap();
        assert!(!loaded.recovered);
        assert_eq!(loaded.store.tasks.len(), 1);
        assert_eq!(loaded.store.tasks[0].title, "test task");
    }

    #[test]
    fn save_creates_parent_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("dir").join("todos.json");
        let _guard = EnvGuard::set_data_file(&path);

        let mut sf = StoreFile::load().unwrap();
        sf.save().unwrap();
        assert!(path.exists());
    }
}
