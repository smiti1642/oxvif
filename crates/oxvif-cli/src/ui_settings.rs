//! Human-terminal preferences; deliberately outside the registry and Agent contract.

use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use atomic_write_file::AtomicWriteFile;
use clap::ValueEnum;
use fs2::FileExt;
use oxvif_cli::AppError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LineNumbers {
    Absolute,
    Relative,
    #[default]
    Hybrid,
    Off,
}

impl LineNumbers {
    pub(crate) const ALL: [Self; 4] = [Self::Absolute, Self::Relative, Self::Hybrid, Self::Off];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Absolute => "absolute",
            Self::Relative => "relative",
            Self::Hybrid => "hybrid",
            Self::Off => "off",
        }
    }

    pub(crate) fn number(self, index: usize, selected: usize) -> Option<usize> {
        match self {
            Self::Absolute => Some(index.saturating_add(1)),
            Self::Relative => Some(index.abs_diff(selected)),
            Self::Hybrid => Some(crate::navigation::relative_number(index, selected)),
            Self::Off => None,
        }
    }
}

struct Preferences {
    path: PathBuf,
    // Lazy loading: a table/JSON command must never read UI preferences.
    current: Option<LineNumbers>,
}

impl Preferences {
    fn resolve(&mut self) -> Result<LineNumbers, AppError> {
        if let Some(mode) = self.current {
            return Ok(mode);
        }
        let mode = load(&self.path)?;
        self.current = Some(mode);
        Ok(mode)
    }

    fn apply(&mut self, mode: LineNumbers, persist: bool) -> Result<(), AppError> {
        if persist {
            save(&self.path, mode)?;
        }
        self.current = Some(mode);
        Ok(())
    }
}

static SESSION: Mutex<Option<Preferences>> = Mutex::new(None);

pub(crate) fn configure(directory: &Path, value: Option<LineNumbers>) -> Result<(), AppError> {
    *SESSION
        .lock()
        .map_err(|_| AppError::internal("UI settings lock failed."))? = Some(Preferences {
        path: directory.join("ui-preferences.json"),
        current: value,
    });
    Ok(())
}

pub(crate) fn current() -> Result<LineNumbers, AppError> {
    let mut state = SESSION
        .lock()
        .map_err(|_| AppError::internal("UI settings lock failed."))?;
    let Some(settings) = state.as_mut() else {
        return Ok(LineNumbers::default());
    };
    settings.resolve()
}

pub(crate) fn apply(mode: LineNumbers, persist: bool) -> Result<(), AppError> {
    let mut state = SESSION
        .lock()
        .map_err(|_| AppError::internal("UI settings lock failed."))?;
    let settings = state
        .as_mut()
        .ok_or_else(|| AppError::internal("UI settings are not initialized."))?;
    settings.apply(mode, persist)
}

fn read_object(path: &Path) -> Result<serde_json::Map<String, serde_json::Value>, AppError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
        Err(error) => return Err(preference_error(error)),
    };
    serde_json::from_slice::<serde_json::Value>(&bytes)
        .map_err(preference_error)?
        .as_object()
        .cloned()
        .ok_or_else(|| preference_error("expected a JSON object"))
}

fn load(path: &Path) -> Result<LineNumbers, AppError> {
    let object = read_object(path)?;
    object
        .get("line_numbers")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(preference_error)
        .map(|mode| mode.unwrap_or_default())
}

fn save(path: &Path, mode: LineNumbers) -> Result<(), AppError> {
    let directory = path
        .parent()
        .ok_or_else(|| preference_error("missing settings directory"))?;
    fs::create_dir_all(directory).map_err(preference_error)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(".ui-preferences.lock"))
        .map_err(preference_error)?;
    // Do not hang an interactive UI behind another process's settings dialog.
    lock.try_lock_exclusive().map_err(preference_error)?;
    let mut object = read_object(path)?;
    object.insert(
        "line_numbers".into(),
        serde_json::Value::String(mode.name().into()),
    );
    let bytes = serde_json::to_vec_pretty(&object).map_err(preference_error)?;
    let mut destination = AtomicWriteFile::options()
        .open(path)
        .map_err(preference_error)?;
    destination.write_all(&bytes).map_err(preference_error)?;
    destination.commit().map_err(preference_error)?;
    // Closing the lock file releases it on every success/error path.
    Ok(())
}

fn preference_error(error: impl std::fmt::Display) -> AppError {
    AppError::registry_io(format!(
        "UI preferences (ui-preferences.json): {error}. Use --line-numbers hybrid for a session-only override; repair the file before saving."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_modes_keep_identity_and_relative_semantics_distinct() {
        assert_eq!(LineNumbers::Absolute.number(20, 20), Some(21));
        assert_eq!(LineNumbers::Absolute.number(23, 20), Some(24));
        assert_eq!(LineNumbers::Relative.number(20, 20), Some(0));
        assert_eq!(LineNumbers::Relative.number(17, 20), Some(3));
        assert_eq!(LineNumbers::Hybrid.number(20, 20), Some(21));
        assert_eq!(LineNumbers::Hybrid.number(23, 20), Some(3));
        assert_eq!(LineNumbers::Off.number(20, 20), None);
    }

    #[test]
    fn session_override_and_apply_are_temporary_until_save_succeeds() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ui-preferences.json");
        save(&path, LineNumbers::Relative).unwrap();
        let mut preferences = Preferences {
            path: path.clone(),
            current: Some(LineNumbers::Absolute),
        };
        assert_eq!(preferences.resolve().unwrap(), LineNumbers::Absolute);
        assert_eq!(load(&path).unwrap(), LineNumbers::Relative);
        preferences.apply(LineNumbers::Off, false).unwrap();
        assert_eq!(preferences.resolve().unwrap(), LineNumbers::Off);
        assert_eq!(load(&path).unwrap(), LineNumbers::Relative);
        preferences.apply(LineNumbers::Hybrid, true).unwrap();
        let mut next_process = Preferences {
            path: path.clone(),
            current: None,
        };
        assert_eq!(next_process.resolve().unwrap(), LineNumbers::Hybrid);
        fs::write(&path, "broken json").unwrap();
        // An explicit override remains usable, but unsuccessful persistence cannot
        // change either the current session or the original file.
        assert_eq!(preferences.resolve().unwrap(), LineNumbers::Hybrid);
        assert!(
            preferences
                .apply(LineNumbers::Off, true)
                .unwrap_err()
                .message
                .contains("expected value")
        );
        assert_eq!(preferences.resolve().unwrap(), LineNumbers::Hybrid);
        assert_eq!(fs::read_to_string(&path).unwrap(), "broken json");
    }

    #[test]
    fn preferences_roundtrip_preserves_unrelated_fields_and_missing_defaults() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ui-preferences.json");
        assert_eq!(load(&path).unwrap(), LineNumbers::Hybrid);
        assert!(!path.exists());
        fs::write(&path, r#"{"future_field":{"enabled":true}}"#).unwrap();
        for mode in LineNumbers::ALL {
            save(&path, mode).unwrap();
            assert_eq!(load(&path).unwrap(), mode);
            assert_eq!(read_object(&path).unwrap()["future_field"]["enabled"], true);
        }
    }

    #[test]
    fn corrupt_preferences_are_not_silently_overwritten_and_locks_fail_promptly() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ui-preferences.json");
        fs::write(&path, "broken json").unwrap();
        let error = save(&path, LineNumbers::Off).unwrap_err();
        assert!(error.message.contains("expected value"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "broken json");
        fs::write(&path, r#"{"line_numbers":"unknown"}"#).unwrap();
        assert!(load(&path).unwrap_err().message.contains("unknown variant"));
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(directory.path().join(".ui-preferences.lock"))
            .unwrap();
        lock.lock_exclusive().unwrap();
        let error = save(&path, LineNumbers::Absolute).unwrap_err();
        assert!(
            error
                .message
                .starts_with("UI preferences (ui-preferences.json):")
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            r#"{"line_numbers":"unknown"}"#
        );
    }
}
