use crate::status::domain::{self, Document, Entry, Point};
use chrono::Utc;
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    thread,
    time::Duration,
};

pub const MAX_STATUS_BYTES: usize = 1024 * 1024;
#[derive(Debug)]
pub enum StatusError {
    InvalidInput(String),
    InvalidDocument(String),
    Io(String),
    Conflict(String),
}
impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(s) | Self::InvalidDocument(s) | Self::Io(s) | Self::Conflict(s) => {
                f.write_str(s)
            }
        }
    }
}
impl From<String> for StatusError {
    fn from(value: String) -> Self {
        Self::InvalidDocument(value)
    }
}
impl From<std::io::Error> for StatusError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

pub struct StatusStore {
    path: PathBuf,
}
struct SidecarLock {
    path: PathBuf,
    file: File,
}
impl Drop for SidecarLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        let _ = fs::remove_file(&self.path);
    }
}
impl StatusStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    fn parent(&self) -> Result<&std::path::Path, StatusError> {
        self.path
            .parent()
            .ok_or_else(|| StatusError::Io("Status directory unavailable".into()))
    }
    fn lock(&self) -> Result<SidecarLock, StatusError> {
        fs::create_dir_all(self.parent()?)?;
        let path = self.path.with_extension("lock");
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)?;
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(SidecarLock { path, file }),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(2))
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    fn read_unlocked(&self) -> Result<Document, StatusError> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Document::default())
            }
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::with_capacity(MAX_STATUS_BYTES.min(8192));
        file.take((MAX_STATUS_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_STATUS_BYTES {
            return Err(StatusError::InvalidDocument(
                "Status JSON exceeds the 1 MiB limit".into(),
            ));
        }
        let mut document: Document = serde_json::from_slice(&bytes).map_err(|error| {
            StatusError::InvalidDocument(format!("Invalid status JSON: {error}"))
        })?;
        domain::validate(&document).map_err(StatusError::InvalidDocument)?;
        if document.version == 1 {
            domain::assign_legacy_ids(&mut document);
            domain::validate(&document).map_err(StatusError::InvalidDocument)?;
        }
        Ok(document)
    }
    pub fn read(&self) -> Result<Document, StatusError> {
        self.read_unlocked()
    }
    pub fn read_project(&self, folder: &str, session: &str) -> Result<Vec<Entry>, StatusError> {
        if !domain::single_line(folder) || !domain::single_line(session) {
            return Err(StatusError::InvalidInput(
                "Project folder and session ID must be non-empty single lines".into(),
            ));
        }
        Ok(self
            .read()?
            .entries
            .into_iter()
            .filter(|entry| {
                entry.folder_name == folder && entry.session_id.as_deref() == Some(session)
            })
            .collect())
    }
    fn save_unlocked(&self, document: &Document) -> Result<usize, StatusError> {
        let bytes = serde_json::to_vec_pretty(document)
            .map_err(|error| StatusError::InvalidDocument(error.to_string()))?;
        if bytes.len() > MAX_STATUS_BYTES {
            return Err(StatusError::InvalidDocument(
                "Status JSON exceeds the 1 MiB limit".into(),
            ));
        }
        let mut temp = tempfile::NamedTempFile::new_in(self.parent()?)?;
        temp.write_all(&bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(&self.path)
            .map_err(|error| StatusError::Io(error.to_string()))?;
        Ok(bytes.len())
    }
    pub fn upsert(
        &self,
        folder: String,
        session: String,
        branch: String,
        points: Vec<Point>,
    ) -> Result<usize, StatusError> {
        let _lock = self.lock()?;
        if !domain::single_line(&folder)
            || !domain::single_line(&session)
            || !domain::single_line(&branch)
        {
            return Err(StatusError::InvalidInput(
                "Folder, session ID, and branch must be non-empty single lines".into(),
            ));
        }
        let mut document = self.read_unlocked()?;
        domain::upsert(&mut document, folder, session, branch, points, Utc::now())
            .map_err(StatusError::InvalidInput)?;
        domain::validate(&document).map_err(StatusError::InvalidDocument)?;
        self.save_unlocked(&document)
    }
    pub fn dismiss(
        &self,
        folder: &str,
        session: Option<&str>,
        branch: &str,
        updated_at: &str,
        point_id: &str,
        point_index: Option<usize>,
    ) -> Result<bool, StatusError> {
        let _lock = self.lock()?;
        let mut document = self.read_unlocked()?;
        if !domain::dismiss(
            &mut document,
            folder,
            session,
            branch,
            updated_at,
            point_id,
            point_index,
        ) {
            return Ok(false);
        }
        domain::validate(&document).map_err(StatusError::InvalidDocument)?;
        self.save_unlocked(&document)?;
        Ok(true)
    }
    pub fn prune(&self, age: Duration) -> Result<bool, StatusError> {
        let _lock = self.lock()?;
        let mut document = self.read_unlocked()?;
        let changed =
            domain::prune(&mut document, age, Utc::now()).map_err(StatusError::InvalidInput)?;
        self.save_unlocked(&document)?;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(text: &str) -> Point {
        Point {
            text: text.into(),
            done: false,
            link: None,
            submitted_at: None,
            id: None,
        }
    }

    #[test]
    fn migrates_v1_and_keeps_duplicate_ids_stable_across_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let old = serde_json::json!({"version":1,"entries":[{"folder_name":"p","session_id":"s","branch":"main","points":[{"text":"same"},{"text":"same"}],"updated_at":"2025-01-01T00:00:00Z"}]});
        fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        let store = StatusStore::new(path.clone());
        let first = store.read().unwrap().entries[0]
            .points
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>();
        store
            .upsert(
                "p".into(),
                "s".into(),
                "main".into(),
                vec![point("same"), point("changed"), point("same")],
            )
            .unwrap();
        let second = store.read().unwrap().entries[0].points.clone();
        assert_eq!(first[0], second[0].id);
        assert_eq!(first[1], second[2].id);
        assert_eq!(second[0].submitted_at, second[2].submitted_at);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(path).unwrap()).unwrap()
                ["version"],
            2
        );
    }
    #[test]
    fn invalid_input_does_not_touch_file_and_writers_keep_all_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let store = StatusStore::new(path.clone());
        store
            .upsert("p".into(), "a".into(), "main".into(), vec![point("old")])
            .unwrap();
        let before = fs::read(&path).unwrap();
        assert!(store
            .upsert("".into(), "a".into(), "main".into(), vec![point("bad")])
            .is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        std::thread::scope(|scope| {
            for i in 0..8 {
                let path = &path;
                scope.spawn(move || {
                    StatusStore::new(path.clone())
                        .upsert("p".into(), format!("{i}"), "main".into(), vec![point("x")])
                        .unwrap();
                });
            }
        });
        assert_eq!(store.read().unwrap().entries.len(), 9);
    }

    #[test]
    fn stale_lock_file_does_not_block_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        fs::write(path.with_extension("lock"), []).unwrap();

        StatusStore::new(path.clone())
            .upsert("p".into(), "s".into(), "main".into(), vec![point("x")])
            .unwrap();

        assert_eq!(StatusStore::new(path).read().unwrap().entries.len(), 1);
        assert!(!dir.path().join("status.lock").exists());
    }

    #[test]
    fn invalid_updates_matrix_keeps_saved_bytes_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let store = StatusStore::new(path.clone());
        store
            .upsert(
                "project".into(),
                "session".into(),
                "main".into(),
                vec![point("original")],
            )
            .unwrap();
        let before = fs::read(&path).unwrap();
        for (session, text, link) in [
            ("", "text", None),
            ("session", " ", None),
            ("session", "text", Some("javascript:alert(1)")),
            ("session", "text", Some("https://")),
        ] {
            assert!(store
                .upsert(
                    "project".into(),
                    session.into(),
                    "main".into(),
                    vec![Point {
                        text: text.into(),
                        done: false,
                        link: link.map(str::to_owned),
                        submitted_at: None,
                        id: None
                    }]
                )
                .is_err());
        }
        assert!(store
            .upsert("project".into(), "session".into(), "main".into(), vec![])
            .is_err());
        assert_eq!(fs::read(path).unwrap(), before);
    }
    #[test]
    fn ids_are_preserved_when_callers_return_them() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert("p".into(), "s".into(), "main".into(), vec![point("x")])
            .unwrap();
        let mut value = store.read().unwrap().entries[0].points[0].clone();
        let id = value.id.clone();
        value.text = "edited".into();
        store
            .upsert("p".into(), "s".into(), "main".into(), vec![value])
            .unwrap();
        assert_eq!(
            store.read().unwrap().entries[0].points[0].id.as_deref(),
            id.as_deref()
        );
    }
    #[test]
    fn parallel_updates_are_serialized_by_the_sidecar_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        std::thread::scope(|scope| {
            for i in 0..24 {
                let path = &path;
                scope.spawn(move || {
                    StatusStore::new(path.clone())
                        .upsert("p".into(), format!("s{i}"), "main".into(), vec![point("x")])
                        .unwrap();
                });
            }
        });
        assert_eq!(StatusStore::new(path).read().unwrap().entries.len(), 24);
    }
    #[test]
    fn scoped_reads_exclude_other_sessions_and_reject_empty_identity() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert("p".into(), "a".into(), "main".into(), vec![point("a")])
            .unwrap();
        assert_eq!(store.read_project("p", "a").unwrap().len(), 1);
        assert!(store.read_project("p", "").is_err());
    }
    #[test]
    fn anonymous_legacy_records_remain_readable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        fs::write(&path, r#"{"version":1,"entries":[{"folder_name":"p","session_id":null,"branch":"main","points":[{"text":"old"}],"updated_at":"2025-01-01T00:00:00Z"}]}"#).unwrap();
        assert_eq!(
            StatusStore::new(path).read().unwrap().entries[0].session_id,
            None
        );
    }
    #[test]
    fn branch_changes_preserve_other_entries() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert("p".into(), "a".into(), "main".into(), vec![point("a")])
            .unwrap();
        store
            .upsert("q".into(), "a".into(), "dev".into(), vec![point("q")])
            .unwrap();
        store
            .upsert("p".into(), "a".into(), "feature".into(), vec![point("new")])
            .unwrap();
        assert_eq!(store.read().unwrap().entries.len(), 2);
    }
    #[test]
    fn unchanged_timestamp_is_retained() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert("p".into(), "a".into(), "main".into(), vec![point("a")])
            .unwrap();
        let before = store.read().unwrap().entries[0].points[0]
            .submitted_at
            .clone();
        store
            .upsert("p".into(), "a".into(), "main".into(), vec![point("a")])
            .unwrap();
        assert_eq!(
            store.read().unwrap().entries[0].points[0].submitted_at,
            before
        );
    }
    #[test]
    fn dismiss_requires_the_snapshot_guard() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert("p".into(), "a".into(), "main".into(), vec![point("a")])
            .unwrap();
        let entry = store.read().unwrap().entries.remove(0);
        assert!(!store
            .dismiss(
                "p",
                Some("a"),
                "main",
                "wrong",
                entry.points[0].id.as_deref().unwrap(),
                Some(0)
            )
            .unwrap());
    }

    #[test]
    fn mismatched_id_never_falls_back_to_the_supplied_index() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let store = StatusStore::new(path.clone());
        store
            .upsert(
                "p".into(),
                "a".into(),
                "main".into(),
                vec![point("a"), point("b")],
            )
            .unwrap();
        let entry = store.read().unwrap().entries[0].clone();
        let before = fs::read(&path).unwrap();
        assert!(!store
            .dismiss(
                "p",
                Some("a"),
                "main",
                &entry.updated_at,
                "missing",
                Some(0)
            )
            .unwrap());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(store
            .dismiss("p", Some("a"), "main", &entry.updated_at, "", Some(0))
            .unwrap());
    }

    #[test]
    fn explicit_id_edit_preserves_old_age_and_unknown_ids_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert(
                "p".into(),
                "a".into(),
                "main".into(),
                vec![point("a"), point("b")],
            )
            .unwrap();
        let old = store.read().unwrap().entries[0].points.clone();
        let mut edited = old[0].clone();
        edited.text = "changed".into();
        edited.submitted_at = None;
        store
            .upsert(
                "p".into(),
                "a".into(),
                "main".into(),
                vec![edited, old[1].clone()],
            )
            .unwrap();
        let next = store.read().unwrap().entries[0].points.clone();
        assert_eq!(next[0].submitted_at, old[0].submitted_at);
        let mut unknown = point("x");
        unknown.id = Some("unknown".into());
        assert!(store
            .upsert("p".into(), "a".into(), "main".into(), vec![unknown])
            .is_err());
    }

    #[test]
    fn v2_requires_ids_and_dismiss_migrates_v1_on_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        fs::write(&path, r#"{"version":2,"entries":[{"folder_name":"p","session_id":"a","branch":"main","points":[{"text":"x"}],"updated_at":"2025-01-01T00:00:00Z"}]}"#).unwrap();
        assert!(StatusStore::new(path.clone()).read().is_err());
        fs::write(&path, r#"{"version":1,"entries":[{"folder_name":"p","session_id":"a","branch":"main","points":[{"text":"x"}],"updated_at":"2025-01-01T00:00:00Z"}]}"#).unwrap();
        let store = StatusStore::new(path.clone());
        let entry = store.read().unwrap().entries[0].clone();
        assert!(store
            .dismiss(
                "p",
                Some("a"),
                "main",
                &entry.updated_at,
                entry.points[0].id.as_deref().unwrap(),
                None
            )
            .unwrap());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(path).unwrap()).unwrap()
                ["version"],
            2
        );
    }
    #[test]
    fn prune_removes_old_points_and_keeps_recent_documents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let now = chrono::Utc::now();
        let document = Document {
            version: 2,
            entries: vec![Entry {
                folder_name: "p".into(),
                session_id: Some("a".into()),
                branch: "main".into(),
                points: vec![
                    Point {
                        text: "old".into(),
                        done: false,
                        link: None,
                        submitted_at: Some((now - chrono::Duration::hours(2)).to_rfc3339()),
                        id: Some("old".into()),
                    },
                    Point {
                        text: "new".into(),
                        done: false,
                        link: None,
                        submitted_at: Some(now.to_rfc3339()),
                        id: Some("new".into()),
                    },
                ],
                updated_at: now.to_rfc3339(),
            }],
        };
        fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        assert!(StatusStore::new(path.clone())
            .prune(Duration::from_secs(60))
            .unwrap());
        assert_eq!(
            StatusStore::new(path).read().unwrap().entries[0].points[0]
                .id
                .as_deref(),
            Some("new")
        );
    }
    #[test]
    fn malformed_and_future_documents_are_not_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        for (name, contents) in [
            ("bad", "broken"),
            ("future", r#"{"version":99,"entries":[]}"#),
        ] {
            let path = dir.path().join(name);
            fs::write(&path, contents).unwrap();
            let before = fs::read(&path).unwrap();
            assert!(StatusStore::new(path.clone())
                .upsert("p".into(), "a".into(), "main".into(), vec![point("x")])
                .is_err());
            assert_eq!(fs::read(path).unwrap(), before);
        }
    }
}
