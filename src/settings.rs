//! Versioned per-user preference file (`dead-letter/settings.json`).
//!
//! Format (version 1): a JSON object with `"version": 1` plus one boolean per
//! setting. Keys this build does not know are preserved on save.

use serde_json::{Map, Value};
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const VERSION: u64 = 1;
const APP_DIR: &str = "dead-letter";
const FILE_NAME: &str = "settings.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Settings {
    pub(crate) show_aim_marker: bool,
    pub(crate) show_control_hints: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_aim_marker: true,
            show_control_hints: true,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SettingsStore {
    path: Option<PathBuf>,
}

/// Directory-resolution inputs, injectable so tests do not touch the process environment.
fn resolve_path(env: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let abs = |name: &str| env(name).map(PathBuf::from).filter(|p| p.is_absolute());
    let base = if cfg!(windows) {
        abs("APPDATA")?
    } else if cfg!(target_os = "macos") {
        abs("HOME")?.join("Library").join("Application Support")
    } else {
        match abs("XDG_CONFIG_HOME") {
            Some(dir) => dir,
            None => abs("HOME")?.join(".config"),
        }
    };
    Some(base.join(APP_DIR).join(FILE_NAME))
}

impl SettingsStore {
    pub(crate) fn for_user() -> Self {
        Self {
            path: resolve_path(&|name| std::env::var_os(name)),
        }
    }

    #[cfg(test)]
    fn at(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    /// Returns the stored settings, or full defaults when the file is missing,
    /// unreadable, truncated/corrupt, or of an unsupported (older/newer) version.
    pub(crate) fn load(&self) -> Settings {
        self.path
            .as_deref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|bytes| parse(&bytes))
            .unwrap_or_default()
    }

    /// Persists `settings`, keeping any unrelated keys already in the file.
    pub(crate) fn save(&self, settings: Settings) -> io::Result<()> {
        let path = self.path.as_deref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "no per-user settings location available",
            )
        })?;
        let mut object = match std::fs::read(path) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(Value::Object(map)) => map,
                _ => Map::new(),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => Map::new(),
            Err(e) => return Err(e),
        };
        object.insert("version".into(), Value::from(VERSION));
        object.insert(
            "show_aim_marker".into(),
            Value::Bool(settings.show_aim_marker),
        );
        object.insert(
            "show_control_hints".into(),
            Value::Bool(settings.show_control_hints),
        );
        let mut bytes = serde_json::to_vec_pretty(&Value::Object(object))
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        bytes.push(b'\n');
        write_atomic(path, &bytes)
    }
}

fn parse(bytes: &[u8]) -> Option<Settings> {
    let Value::Object(map) = serde_json::from_slice::<Value>(bytes).ok()? else {
        return None;
    };
    if map.get("version")?.as_u64()? != VERSION {
        return None;
    }
    let defaults = Settings::default();
    let flag = |key: &str, default: bool| match map.get(key) {
        None => Some(default),
        Some(Value::Bool(b)) => Some(*b),
        Some(_) => None,
    };
    Some(Settings {
        show_aim_marker: flag("show_aim_marker", defaults.show_aim_marker)?,
        show_control_hints: flag("show_control_hints", defaults.show_control_hints)?,
    })
}

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "settings path has no parent")
    })?;
    std::fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(FILE_NAME);

    let (tmp_path, mut file) = loop {
        let tmp = dir.join(format!(
            ".{name}.{}.{}.tmp",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
        {
            Ok(file) => break (tmp, file),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    };

    let result = file
        .write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .and_then(|()| {
            drop(file);
            std::fs::rename(&tmp_path, path)
        });
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let dir = std::env::temp_dir().join(format!(
                "dead-letter-settings-{tag}-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn store(dir: &TempDir) -> SettingsStore {
        SettingsStore::at(dir.0.join("nested").join(FILE_NAME))
    }

    fn off() -> Settings {
        Settings {
            show_aim_marker: false,
            show_control_hints: false,
        }
    }

    #[test]
    fn missing_file_loads_defaults() {
        let dir = TempDir::new("missing");
        assert_eq!(store(&dir).load(), Settings::default());
    }

    #[test]
    fn unavailable_location_loads_defaults_and_save_errors() {
        let s = SettingsStore { path: None };
        assert_eq!(s.load(), Settings::default());
        assert!(s.save(off()).is_err());
    }

    #[test]
    fn settings_persist_across_store_instances() {
        let dir = TempDir::new("persist");
        let changed = Settings {
            show_aim_marker: false,
            show_control_hints: true,
        };
        store(&dir).save(changed).unwrap();
        assert_eq!(store(&dir).load(), changed);
        store(&dir).save(off()).unwrap();
        assert_eq!(store(&dir).load(), off());
        store(&dir).save(Settings::default()).unwrap();
        assert_eq!(store(&dir).load(), Settings::default());
    }

    #[test]
    fn truncated_file_loads_defaults() {
        let dir = TempDir::new("truncated");
        let s = store(&dir);
        s.save(off()).unwrap();
        let path = s.path.clone().unwrap();
        let full = std::fs::read(&path).unwrap();
        std::fs::write(&path, &full[..full.len() / 2]).unwrap();
        assert_eq!(s.load(), Settings::default());
        std::fs::write(&path, b"").unwrap();
        assert_eq!(s.load(), Settings::default());
    }

    #[test]
    fn older_or_unsupported_version_loads_defaults() {
        let dir = TempDir::new("version");
        let s = store(&dir);
        let path = s.path.clone().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        for body in [
            r#"{"show_aim_marker":false,"show_control_hints":false}"#,
            r#"{"version":0,"show_aim_marker":false,"show_control_hints":false}"#,
            r#"{"version":2,"show_aim_marker":false,"show_control_hints":false}"#,
            r#"{"version":1,"show_aim_marker":"no"}"#,
            r#"[1,2]"#,
        ] {
            std::fs::write(&path, body).unwrap();
            assert_eq!(s.load(), Settings::default(), "{body}");
        }
    }

    #[test]
    fn unrelated_preferences_are_retained_on_save() {
        let dir = TempDir::new("retain");
        let s = store(&dir);
        let path = s.path.clone().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"version":1,"show_aim_marker":true,"master_volume":0.25,"name":"x"}"#,
        )
        .unwrap();
        s.save(off()).unwrap();
        let value: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["master_volume"], 0.25);
        assert_eq!(value["name"], "x");
        assert_eq!(value["version"], 1);
        assert_eq!(s.load(), off());
    }

    #[test]
    fn save_replaces_corrupt_file_and_leaves_no_temp_files() {
        let dir = TempDir::new("clean");
        let s = store(&dir);
        let path = s.path.clone().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{\"version\":1,").unwrap();
        s.save(off()).unwrap();
        assert_eq!(s.load(), off());
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from(FILE_NAME)]);
    }

    #[test]
    fn save_failure_is_reported_and_keeps_previous_file() {
        let dir = TempDir::new("fail");
        let s = store(&dir);
        s.save(Settings::default()).unwrap();
        let path = s.path.clone().unwrap();
        // Replace the target with a directory: the existing entry cannot be read,
        // so save must fail without replacing it.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("keep"), b"x").unwrap();
        assert!(s.save(off()).is_err());
        assert_eq!(std::fs::read(path.join("keep")).unwrap(), b"x");
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from(FILE_NAME)]);
        assert_eq!(s.load(), Settings::default());
    }

    #[test]
    fn path_resolution_ignores_relative_and_missing_env() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| OsString::from(*v))
            }
        };
        if cfg!(windows) {
            assert!(resolve_path(&env(&[("APPDATA", "rel")])).is_none());
        } else if cfg!(target_os = "macos") {
            assert!(resolve_path(&env(&[("HOME", "rel")])).is_none());
        } else {
            assert_eq!(
                resolve_path(&env(&[("XDG_CONFIG_HOME", "/x"), ("HOME", "/h")])),
                Some(PathBuf::from("/x/dead-letter/settings.json"))
            );
            assert_eq!(
                resolve_path(&env(&[("XDG_CONFIG_HOME", "rel"), ("HOME", "/h")])),
                Some(PathBuf::from("/h/.config/dead-letter/settings.json"))
            );
            assert!(resolve_path(&env(&[("XDG_CONFIG_HOME", "rel"), ("HOME", "rel")])).is_none());
        }
        assert!(resolve_path(&env(&[])).is_none());
    }
}
