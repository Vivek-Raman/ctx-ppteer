use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{fs, fs::OpenOptions, io::Write, path::Path};

pub const MAX_STATUS_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub text: String,
    #[serde(default)]
    pub done: bool,
    /// Optional HTTPS or HTTP URL; the viewer renders the point text as a link.
    #[serde(default)]
    pub link: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub folder_name: String,
    pub session_id: Option<String>,
    pub branch: String,
    pub points: Vec<Point>,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub version: u32,
    pub entries: Vec<Entry>,
}

impl Default for Document {
    fn default() -> Self {
        Self {
            version: 1,
            entries: vec![],
        }
    }
}

fn single_line(value: &str) -> bool {
    !value.trim().is_empty() && !value.contains(['\n', '\r'])
}

fn validate(document: &Document) -> Result<(), String> {
    if document.version != 1 {
        return Err("Unsupported status schema version".into());
    }
    for entry in &document.entries {
        if !single_line(&entry.folder_name)
            || !single_line(&entry.branch)
            || entry.session_id.as_ref().is_some_and(|id| !single_line(id))
            || entry.points.is_empty()
        {
            return Err("Folder, branch, and supplied session ID must be non-empty single lines; points are required".into());
        }
        for point in &entry.points {
            if point.text.trim().is_empty()
                || point.link.as_ref().is_some_and(|link| {
                    !(link
                        .strip_prefix("https://")
                        .or_else(|| link.strip_prefix("http://"))
                        .is_some_and(|rest| !rest.is_empty()))
                        || link.chars().any(char::is_whitespace)
                })
            {
                return Err(
                    "Points need text and links must use HTTP or HTTPS without whitespace".into(),
                );
            }
        }
        chrono::DateTime::parse_from_rfc3339(&entry.updated_at)
            .map_err(|_| "Invalid status timestamp")?;
    }
    let mut keys = std::collections::HashSet::new();
    for entry in &document.entries {
        if let Some(id) = &entry.session_id {
            if !keys.insert((&entry.folder_name, id)) {
                return Err("Duplicate project/session record".into());
            }
        }
    }
    Ok(())
}

pub fn read_document(path: &Path) -> Result<Document, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Document::default())
        }
        Err(error) => return Err(format!("Unable to read status JSON: {error}")),
    };
    if bytes.len() > MAX_STATUS_BYTES {
        return Err("Status JSON exceeds the 1 MiB limit".into());
    }
    let document: Document =
        serde_json::from_slice(&bytes).map_err(|error| format!("Invalid status JSON: {error}"))?;
    validate(&document)?;
    Ok(document)
}

pub fn read_project_status(
    path: &Path,
    folder_name: &str,
    session_id: &str,
) -> Result<Vec<Entry>, String> {
    if !single_line(folder_name) || !single_line(session_id) {
        return Err("Project folder and session ID must be non-empty single lines".into());
    }
    Ok(read_document(path)?
        .entries
        .into_iter()
        .filter(|entry| {
            entry.folder_name == folder_name && entry.session_id.as_deref() == Some(session_id)
        })
        .collect())
}

/// Lock a stable sidecar, not the JSON inode replaced by atomic writes.
pub fn write_project_status(
    path: &Path,
    folder_name: String,
    session_id: String,
    branch: String,
    points: Vec<Point>,
) -> Result<usize, String> {
    let entry = Entry {
        folder_name,
        session_id: Some(session_id),
        branch,
        points,
        updated_at: chrono::Utc::now().to_rfc3339(),
    };
    validate(&Document {
        version: 1,
        entries: vec![entry.clone()],
    })?;
    let parent = path.parent().ok_or("Status directory unavailable")?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension("lock"))
        .map_err(|error| error.to_string())?;
    lock.lock()
        .map_err(|error| format!("Unable to lock status JSON: {error}"))?;
    let mut document = read_document(path)?;
    if let Some(index) = document
        .entries
        .iter()
        .position(|old| old.folder_name == entry.folder_name && old.session_id == entry.session_id)
    {
        document.entries[index] = entry;
    } else {
        document.entries.push(entry);
    }
    document
        .entries
        .sort_by_key(|entry| entry.folder_name.to_lowercase());
    let bytes = serde_json::to_vec_pretty(&document).map_err(|error| error.to_string())?;
    if bytes.len() > MAX_STATUS_BYTES {
        return Err("Status JSON exceeds the 1 MiB limit".into());
    }
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| error.to_string())?;
    temporary
        .write_all(&bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|error| error.to_string())?;
    temporary.persist(path).map_err(|error| error.to_string())?;
    Ok(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(path: &Path, session: String, text: &str) {
        write_project_status(
            path,
            "project".into(),
            session,
            "main".into(),
            vec![Point {
                text: text.into(),
                done: false,
                link: None,
            }],
        )
        .unwrap();
    }
    #[test]
    fn sessions_replace_only_their_own_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        write(&path, "a".into(), "first");
        write(&path, "b".into(), "other");
        write(&path, "a".into(), "updated");
        let document = read_document(&path).unwrap();
        assert_eq!(document.entries.len(), 2);
        assert_eq!(document.entries[0].points[0].text, "updated");
        assert_eq!(document.entries[1].points[0].text, "other");
    }
    #[test]
    fn reads_only_the_requested_project_session_and_rejects_empty_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        write(&path, "a".into(), "own");
        write(&path, "b".into(), "other");
        let entries = read_project_status(&path, "project", "a").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].points[0].text, "own");
        assert!(read_project_status(&path, "another-project", "a")
            .unwrap()
            .is_empty());
        assert!(read_project_status(&path, "project", "missing")
            .unwrap()
            .is_empty());
        assert!(read_project_status(&path, "project", "").is_err());
        assert!(read_project_status(&path, "project", "a\nb").is_err());
    }

    #[test]
    fn legacy_anonymous_records_remain_viewable_but_are_not_returned_by_scoped_reads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let legacy = Entry {
            folder_name: "project".into(),
            session_id: None,
            branch: "main".into(),
            points: vec![Point {
                text: "legacy".into(),
                done: false,
                link: None,
            }],
            updated_at: chrono::Utc::now().to_rfc3339(),
        };
        fs::write(
            &path,
            serde_json::to_vec(&Document {
                version: 1,
                entries: vec![legacy],
            })
            .unwrap(),
        )
        .unwrap();
        write(&path, "a".into(), "own");
        assert_eq!(read_document(&path).unwrap().entries.len(), 2);
        assert_eq!(read_project_status(&path, "project", "a").unwrap().len(), 1);
    }

    #[test]
    fn parallel_writers_preserve_all_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        std::thread::scope(|scope| {
            for i in 0..24 {
                let path = &path;
                scope.spawn(move || write(path, i.to_string(), "activity"));
            }
        });
        assert_eq!(read_document(&path).unwrap().entries.len(), 24);
    }
    #[test]
    fn preserves_other_projects_and_updates_branch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        write(&path, "session".into(), "original");
        let points = || {
            vec![Point {
                text: "activity".into(),
                done: true,
                link: Some("https://example.com/pr/1".into()),
            }]
        };
        write_project_status(
            &path,
            "another".into(),
            "session".into(),
            "develop".into(),
            points(),
        )
        .unwrap();
        write_project_status(
            &path,
            "project".into(),
            "session".into(),
            "feature/json".into(),
            points(),
        )
        .unwrap();
        let document = read_document(&path).unwrap();
        assert_eq!(document.entries.len(), 2);
        assert_eq!(document.entries[0].folder_name, "another");
        assert_eq!(document.entries[1].branch, "feature/json");
        assert!(document.entries[1].points[0].done);
    }

    #[test]
    fn rejects_invalid_updates_without_changing_saved_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        write(&path, "session".into(), "original");
        let before = fs::read(&path).unwrap();
        for (session, text, link) in [
            ("", "text", None),
            ("session", " ", None),
            ("session", "text", Some("javascript:alert(1)")),
            ("session", "text", Some("https://")),
        ] {
            assert!(write_project_status(
                &path,
                "project".into(),
                session.to_owned(),
                "main".into(),
                vec![Point {
                    text: text.into(),
                    done: false,
                    link: link.map(str::to_owned)
                }]
            )
            .is_err());
        }
        assert!(write_project_status(
            &path,
            "project".into(),
            "session".into(),
            "main".into(),
            vec![]
        )
        .is_err());
        assert!(write_project_status(
            &path,
            "project".into(),
            "session".into(),
            "main".into(),
            vec![Point {
                text: "x".repeat(MAX_STATUS_BYTES),
                done: false,
                link: None
            }]
        )
        .is_err());
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn rejects_future_schema_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        fs::write(&path, r#"{"version":2,"entries":[]}"#).unwrap();
        assert!(read_document(&path).is_err());
    }

    #[test]
    fn malformed_json_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        fs::write(&path, "broken").unwrap();
        assert!(write_project_status(
            &path,
            "project".into(),
            "session".into(),
            "main".into(),
            vec![Point {
                text: "text".into(),
                done: false,
                link: None
            }]
        )
        .is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "broken");
    }
}
