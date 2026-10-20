use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub text: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submitted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub folder_name: String,
    pub session_id: Option<String>,
    pub branch: String,
    pub points: Vec<Point>,
    pub updated_at: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
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

pub fn single_line(value: &str) -> bool {
    !value.trim().is_empty() && !value.contains(['\n', '\r'])
}

pub fn validate(document: &Document) -> Result<(), String> {
    if !matches!(document.version, 1 | 2) {
        return Err("Unsupported status schema version".into());
    }
    let mut keys = HashSet::new();
    for entry in &document.entries {
        if !single_line(&entry.folder_name)
            || !single_line(&entry.branch)
            || entry.session_id.as_ref().is_some_and(|id| !single_line(id))
            || entry.points.is_empty()
        {
            return Err("Folder, branch, and supplied session ID must be non-empty single lines; points are required".into());
        }
        if let Some(id) = &entry.session_id {
            if !keys.insert((&entry.folder_name, id)) {
                return Err("Duplicate project/session record".into());
            }
        }
        let mut ids = HashSet::new();
        for point in &entry.points {
            if point.text.trim().is_empty() {
                return Err("Points need text".into());
            }
            if point.link.as_ref().is_some_and(|link| {
                !(link
                    .strip_prefix("https://")
                    .or_else(|| link.strip_prefix("http://"))
                    .is_some_and(|rest| !rest.is_empty()))
                    || link.chars().any(char::is_whitespace)
            }) {
                return Err("Links must use HTTP or HTTPS without whitespace".into());
            }
            if let Some(at) = &point.submitted_at {
                DateTime::parse_from_rfc3339(at).map_err(|_| "Invalid point timestamp")?;
            }
            if document.version == 2 {
                let id = point
                    .id
                    .as_ref()
                    .ok_or("Schema version 2 points require IDs")?;
                if !single_line(id) || !ids.insert(id) {
                    return Err("Point IDs must be unique non-empty single lines".into());
                }
            } else if let Some(id) = &point.id {
                if !single_line(id) || !ids.insert(id) {
                    return Err("Point IDs must be unique non-empty single lines".into());
                }
            }
        }
        DateTime::parse_from_rfc3339(&entry.updated_at).map_err(|_| "Invalid status timestamp")?;
    }
    Ok(())
}

fn legacy_id(entry: &Entry, point: &Point, occurrence: usize, used: &HashSet<String>) -> String {
    let base = format!(
        "legacy:{}:{}:{}:{}:{}",
        entry.folder_name,
        entry.session_id.as_deref().unwrap_or(""),
        entry.branch,
        point.text,
        point.submitted_at.as_deref().unwrap_or("")
    );
    let mut id = format!("p-{}", Uuid::new_v5(&Uuid::NAMESPACE_OID, base.as_bytes()));
    let mut suffix = occurrence;
    while used.contains(&id) {
        suffix += 1;
        id = format!(
            "p-{}-{suffix}",
            Uuid::new_v5(&Uuid::NAMESPACE_OID, format!("{base}:{suffix}").as_bytes())
        );
    }
    id
}
/// Assign deterministic IDs only while reading a version 1 document. Callers must validate again.
pub fn assign_legacy_ids(document: &mut Document) {
    if document.version != 1 {
        return;
    }
    let mut used: HashSet<String> = document
        .entries
        .iter()
        .flat_map(|entry| entry.points.iter().filter_map(|point| point.id.clone()))
        .collect();
    for entry in &mut document.entries {
        let metadata = Entry {
            folder_name: entry.folder_name.clone(),
            session_id: entry.session_id.clone(),
            branch: entry.branch.clone(),
            points: vec![],
            updated_at: entry.updated_at.clone(),
        };
        for (index, point) in entry.points.iter_mut().enumerate() {
            if point.id.is_none() {
                let id = legacy_id(&metadata, point, index, &used);
                used.insert(id.clone());
                point.id = Some(id);
            }
        }
    }
}
fn new_id() -> String {
    format!("p-{}", Uuid::new_v4())
}

pub fn upsert(
    document: &mut Document,
    folder: String,
    session: String,
    branch: String,
    mut points: Vec<Point>,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let existing = document
        .entries
        .iter()
        .find(|entry| entry.folder_name == folder && entry.session_id.as_deref() == Some(&session))
        .cloned();
    let timestamp = now.to_rfc3339();
    let old_ids: HashSet<String> = existing
        .as_ref()
        .into_iter()
        .flat_map(|entry| entry.points.iter().filter_map(|point| point.id.clone()))
        .collect();
    let mut reserved = HashSet::new();
    for point in &points {
        if let Some(id) = &point.id {
            if !reserved.insert(id.clone()) || existing.is_none() || !old_ids.contains(id) {
                return Err("Supplied point ID is unknown or duplicated".into());
            }
        }
    }
    let mut matched = HashSet::new();
    for point in &mut points {
        let old = point
            .id
            .as_ref()
            .and_then(|id| {
                existing.as_ref().and_then(|entry| {
                    entry
                        .points
                        .iter()
                        .find(|old| old.id.as_deref() == Some(id))
                })
            })
            .or_else(|| {
                existing.as_ref().and_then(|entry| {
                    entry.points.iter().find(|old| {
                        !reserved.contains(old.id.as_deref().unwrap_or(""))
                            && !matched.contains(old.id.as_deref().unwrap_or(""))
                            && point.id.is_none()
                            && old.text == point.text
                            && old.done == point.done
                            && old.link == point.link
                    })
                })
            });
        if let Some(old) = old {
            if let Some(id) = &old.id {
                matched.insert(id.clone());
                point.id = Some(id.clone());
            }
            if point.submitted_at.is_none() {
                point.submitted_at = old.submitted_at.clone();
            }
        }
        if point.id.is_none() {
            point.id = Some(new_id());
        }
        if point.submitted_at.is_none() {
            point.submitted_at = Some(timestamp.clone());
        }
    }
    let entry = Entry {
        folder_name: folder,
        session_id: Some(session),
        branch,
        points,
        updated_at: timestamp,
    };
    let mut candidate = document.clone();
    candidate.version = 2;
    if let Some(index) = candidate
        .entries
        .iter()
        .position(|old| old.folder_name == entry.folder_name && old.session_id == entry.session_id)
    {
        candidate.entries[index] = entry;
    } else {
        candidate.entries.push(entry);
    }
    candidate
        .entries
        .sort_by_key(|entry| entry.folder_name.to_lowercase());
    validate(&candidate)?;
    *document = candidate;
    Ok(())
}

pub fn dismiss(
    document: &mut Document,
    folder: &str,
    session: Option<&str>,
    branch: &str,
    updated_at: &str,
    point_id: &str,
    point_index: Option<usize>,
) -> bool {
    let Some(entry_index) = document.entries.iter().position(|entry| {
        entry.folder_name == folder
            && entry.session_id.as_deref() == session
            && entry.branch == branch
            && entry.updated_at == updated_at
    }) else {
        return false;
    };
    let point_index = if point_id.is_empty() {
        point_index
    } else {
        document.entries[entry_index]
            .points
            .iter()
            .position(|point| point.id.as_deref() == Some(point_id))
    };
    let Some(point_index) = point_index else {
        return false;
    };
    if point_index >= document.entries[entry_index].points.len() {
        return false;
    }
    document.entries[entry_index].points.remove(point_index);
    if document.entries[entry_index].points.is_empty() {
        document.entries.remove(entry_index);
    }
    document.version = 2;
    true
}

pub fn prune(
    document: &mut Document,
    age: std::time::Duration,
    now: DateTime<Utc>,
) -> Result<bool, String> {
    let cutoff = now - chrono::Duration::from_std(age).map_err(|error| error.to_string())?;
    let before = document
        .entries
        .iter()
        .map(|entry| entry.points.len())
        .sum::<usize>();
    for entry in &mut document.entries {
        entry.points.retain(|point| {
            point.submitted_at.as_ref().is_none_or(|at| {
                DateTime::parse_from_rfc3339(at).is_ok_and(|at| at.with_timezone(&Utc) >= cutoff)
            })
        });
    }
    document.entries.retain(|entry| !entry.points.is_empty());
    document.version = 2;
    validate(document)?;
    Ok(before
        != document
            .entries
            .iter()
            .map(|entry| entry.points.len())
            .sum::<usize>())
}
