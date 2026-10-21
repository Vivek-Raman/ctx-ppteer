use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter};

use crate::paths;

pub const PROJECT_DIRECTORY_ASC: &str = "project-directory-asc";
pub const UPDATED_AT_DESC: &str = "updated-at-desc";
pub const UPDATED_AT_ASC: &str = "updated-at-asc";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ProjectSortOrder {
    #[serde(rename = "project-directory-asc")]
    ProjectDirectoryAsc,
    #[serde(rename = "updated-at-desc")]
    UpdatedAtDesc,
    #[serde(rename = "updated-at-asc")]
    UpdatedAtAsc,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_theme")]
    pub theme: Theme,
    #[serde(default = "default_primary_color")]
    pub primary_color: String,
    #[serde(default = "default_text_scale")]
    pub text_scale: f64,
    #[serde(default = "default_pinned")]
    pub pinned: bool,
    #[serde(default = "default_project_sort_order")]
    pub project_sort_order: ProjectSortOrder,
    #[serde(default)]
    pub auto_clear_after_minutes: Option<u64>,
}

fn default_theme() -> Theme {
    Theme::System
}
fn default_primary_color() -> String {
    "#c98c5a".into()
}
fn default_text_scale() -> f64 {
    1.0
}
fn default_pinned() -> bool {
    true
}
fn default_project_sort_order() -> ProjectSortOrder {
    ProjectSortOrder::UpdatedAtDesc
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            primary_color: default_primary_color(),
            text_scale: default_text_scale(),
            pinned: default_pinned(),
            project_sort_order: default_project_sort_order(),
            auto_clear_after_minutes: None,
        }
    }
}

impl Theme {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "system" => Ok(Self::System),
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err("Theme must be system, light, or dark".into()),
        }
    }
}

impl ProjectSortOrder {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value { PROJECT_DIRECTORY_ASC => Ok(Self::ProjectDirectoryAsc), UPDATED_AT_DESC => Ok(Self::UpdatedAtDesc), UPDATED_AT_ASC => Ok(Self::UpdatedAtAsc), _ => Err("Project sort order must be project-directory-asc, updated-at-desc, or updated-at-asc".into()) }
    }
}

fn validate(settings: &Settings) -> Result<(), String> {
    if !(0.85..=1.35).contains(&settings.text_scale) {
        return Err("Text scale must be between 0.85 and 1.35".into());
    }
    if !is_hex_color(&settings.primary_color) {
        return Err("Primary color must be a six-digit hex color".into());
    }
    if settings
        .auto_clear_after_minutes
        .is_some_and(|minutes| !(1..=43_200).contains(&minutes))
    {
        return Err("Auto-clear must be between 1 minute and 30 days".into());
    }
    Ok(())
}

pub fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.bytes().skip(1).all(|byte| byte.is_ascii_hexdigit())
}

fn persist(path: &Path, settings: &Settings) -> Result<(), String> {
    let directory = path.parent().ok_or("Settings directory unavailable")?;
    fs::create_dir_all(directory)
        .map_err(|error| format!("Unable to create {}: {error}", directory.display()))?;
    let contents = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Unable to encode settings: {error}"))?;
    let mut temporary = tempfile::Builder::new()
        .prefix("settings-")
        .suffix(".tmp")
        .tempfile_in(directory)
        .map_err(|error| format!("Unable to create settings temporary file: {error}"))?;
    std::io::Write::write_all(&mut temporary, &contents)
        .map_err(|error| format!("Unable to write settings: {error}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| format!("Unable to flush settings: {error}"))?;
    temporary
        .persist(path)
        .map_err(|error| format!("Unable to save settings: {}", error.error))?;
    Ok(())
}

fn has_removed_window_preferences(contents: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(contents)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .is_some_and(|settings| settings.contains_key("window"))
}

pub struct SettingsOwner {
    path: PathBuf,
    settings: Mutex<Settings>,
    publisher: Option<Arc<dyn Fn(&Settings) + Send + Sync>>,
}

impl SettingsOwner {
    pub fn load(app: &AppHandle) -> Result<Arc<Self>, String> {
        let path = paths::settings_path(app)?;
        let settings = match fs::read(&path) {
            Ok(contents) => {
                let settings: Settings = serde_json::from_slice(&contents)
                    .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
                validate(&settings)?;
                if has_removed_window_preferences(&contents) {
                    persist(&path, &settings)?;
                }
                settings
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let settings = Settings::default();
                persist(&path, &settings)?;
                settings
            }
            Err(error) => return Err(format!("Unable to read {}: {error}", path.display())),
        };
        Ok(Arc::new(Self {
            path,
            settings: Mutex::new(settings),
            publisher: Some(Arc::new({
                let app = app.clone();
                move |settings| {
                    let _ = app.emit("settings-update", settings);
                }
            })),
        }))
    }

    #[cfg(test)]
    pub fn from_path(path: PathBuf) -> Result<Arc<Self>, String> {
        let settings = match fs::read(&path) {
            Ok(contents) => {
                let settings: Settings = serde_json::from_slice(&contents)
                    .map_err(|error| format!("Unable to read {}: {error}", path.display()))?;
                validate(&settings)?;
                settings
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let settings = Settings::default();
                persist(&path, &settings)?;
                settings
            }
            Err(error) => return Err(format!("Unable to read {}: {error}", path.display())),
        };
        Ok(Arc::new(Self {
            path,
            settings: Mutex::new(settings),
            publisher: None,
        }))
    }

    #[cfg(test)]
    pub fn from_path_with_publisher(
        path: PathBuf,
        publisher: Arc<dyn Fn(&Settings) + Send + Sync>,
    ) -> Result<Arc<Self>, String> {
        let owner = Self::from_path(path)?;
        Ok(Arc::new(Self {
            path: owner.path.clone(),
            settings: Mutex::new(owner.snapshot()),
            publisher: Some(publisher),
        }))
    }

    pub fn snapshot(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    pub fn update<F>(&self, change: F) -> Result<Settings, String>
    where
        F: FnOnce(&mut Settings) -> Result<(), String>,
    {
        let mut current = self.settings.lock().map_err(|_| "Settings unavailable")?;
        let mut candidate = current.clone();
        change(&mut candidate)?;
        validate(&candidate)?;
        if candidate == *current {
            return Ok(current.clone());
        }
        persist(&self.path, &candidate)?;
        *current = candidate.clone();
        if let Some(publish) = &self.publisher {
            publish(&candidate);
        }
        Ok(candidate)
    }

    pub fn reset(&self) -> Result<Settings, String> {
        self.update(|settings| {
            *settings = Settings::default();
            Ok(())
        })
    }
    pub fn set_pinned(&self, pinned: bool) -> Result<Settings, String> {
        self.update(|settings| {
            settings.pinned = pinned;
            Ok(())
        })
    }
    pub fn set_appearance(
        &self,
        theme: &str,
        primary_color: String,
        text_scale: f64,
    ) -> Result<Settings, String> {
        let theme = Theme::parse(theme)?;
        self.update(|settings| {
            settings.theme = theme;
            settings.primary_color = primary_color;
            settings.text_scale = text_scale;
            Ok(())
        })
    }
    pub fn set_sort_order(&self, value: &str) -> Result<Settings, String> {
        let value = ProjectSortOrder::parse(value)?;
        self.update(|settings| {
            settings.project_sort_order = value;
            Ok(())
        })
    }
    pub fn set_retention(&self, value: Option<u64>) -> Result<Settings, String> {
        if value.is_some_and(|minutes| !(1..=43_200).contains(&minutes)) {
            return Err("Auto-clear must be between 1 minute and 30 days".into());
        }
        self.update(|settings| {
            settings.auto_clear_after_minutes = value;
            Ok(())
        })
    }
}

pub fn settings_owner(app: &AppHandle) -> Result<Arc<SettingsOwner>, String> {
    SettingsOwner::load(app)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ignores_removed_window_preferences() {
        let settings: Settings = serde_json::from_str(r#"{"sourcePath":"/tmp/status.md","window":{"monitor_name":"Display","relative_x":0.25,"relative_y":0.5}}"#).unwrap();
        assert_eq!(settings, Settings::default());
    }
    #[test]
    fn recognizes_removed_window_preferences() {
        assert!(has_removed_window_preferences(
            br#"{"window":{"relative_x":0.25}}"#
        ));
        assert!(!has_removed_window_preferences(br#"{"theme":"dark"}"#));
    }
    #[test]
    fn concurrent_disjoint_updates_retain_both() {
        let directory = tempfile::tempdir().unwrap();
        let owner = SettingsOwner::from_path(directory.path().join("settings.json")).unwrap();
        let one = owner.clone();
        let a =
            std::thread::spawn(move || one.set_appearance("dark", "#c98c5a".into(), 1.0).unwrap());
        let two = owner.clone();
        let b = std::thread::spawn(move || two.set_retention(Some(10)).unwrap());
        a.join().unwrap();
        b.join().unwrap();
        let settings = owner.snapshot();
        assert_eq!(settings.theme, Theme::Dark);
        assert_eq!(settings.auto_clear_after_minutes, Some(10));
    }
    #[test]
    fn invalid_patch_leaves_memory_and_disk_unchanged() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let owner = SettingsOwner::from_path(path.clone()).unwrap();
        let before = owner.snapshot();
        assert!(owner.set_appearance("dark", "bad".into(), 1.0).is_err());
        assert_eq!(owner.snapshot(), before);
        assert_eq!(
            serde_json::from_slice::<Settings>(&fs::read(path).unwrap()).unwrap(),
            before
        );
    }
    #[test]
    fn persistence_failure_keeps_last_state() {
        let directory = tempfile::tempdir().unwrap();
        let owner = SettingsOwner::from_path(directory.path().join("settings.json")).unwrap();
        let before = owner.snapshot();
        let owner = SettingsOwner {
            path: directory.path().join("missing/settings.json"),
            settings: Mutex::new(before.clone()),
            publisher: None,
        };
        fs::write(directory.path().join("missing"), "file").unwrap();
        assert!(owner.set_retention(Some(10)).is_err());
        assert_eq!(owner.snapshot(), before);
    }

    #[test]
    fn settings_publication_is_serialized_with_the_commit() {
        let directory = tempfile::tempdir().unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let seen = events.clone();
        let owner = SettingsOwner::from_path_with_publisher(
            directory.path().join("settings.json"),
            Arc::new(move |settings| {
                seen.lock().unwrap().push(settings.clone());
            }),
        )
        .unwrap();
        let one = owner.clone();
        let a =
            std::thread::spawn(move || one.set_appearance("dark", "#c98c5a".into(), 1.0).unwrap());
        let two = owner.clone();
        let b = std::thread::spawn(move || two.set_retention(Some(10)).unwrap());
        a.join().unwrap();
        b.join().unwrap();
        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events.last().unwrap(), &owner.snapshot());
    }
}
