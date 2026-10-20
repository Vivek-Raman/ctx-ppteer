pub mod domain;
pub mod retention;
pub mod snapshot;
pub mod store;
pub mod watcher;

pub use domain::{Document, Entry, Point};
pub use store::{StatusError, StatusStore, MAX_STATUS_BYTES};

use std::{path::Path, time::Duration};

pub fn read_document(path: &Path) -> Result<Document, String> {
    StatusStore::new(path.to_path_buf())
        .read()
        .map_err(|error| error.to_string())
}
pub fn read_project_status(
    path: &Path,
    folder_name: &str,
    session_id: &str,
) -> Result<Vec<Entry>, String> {
    StatusStore::new(path.to_path_buf())
        .read_project(folder_name, session_id)
        .map_err(|error| error.to_string())
}
pub fn write_project_status(
    path: &Path,
    folder_name: String,
    session_id: String,
    branch: String,
    points: Vec<Point>,
) -> Result<usize, String> {
    StatusStore::new(path.to_path_buf())
        .upsert(folder_name, session_id, branch, points)
        .map_err(|error| error.to_string())
}
pub fn dismiss_point(
    path: &Path,
    folder_name: &str,
    session_id: Option<&str>,
    branch: &str,
    updated_at: &str,
    point_index: usize,
) -> Result<bool, String> {
    dismiss_point_by_id(
        path,
        folder_name,
        session_id,
        branch,
        updated_at,
        "",
        Some(point_index),
    )
}

pub fn dismiss_point_by_id(
    path: &Path,
    folder_name: &str,
    session_id: Option<&str>,
    branch: &str,
    updated_at: &str,
    point_id: &str,
    point_index: Option<usize>,
) -> Result<bool, String> {
    StatusStore::new(path.to_path_buf())
        .dismiss(
            folder_name,
            session_id,
            branch,
            updated_at,
            point_id,
            point_index,
        )
        .map_err(|error| error.to_string())
}
pub fn prune_expired_points(path: &Path, age: Duration) -> Result<bool, String> {
    StatusStore::new(path.to_path_buf())
        .prune(age)
        .map_err(|error| error.to_string())
}
