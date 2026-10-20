use crate::status::store::{StatusStore, MAX_STATUS_BYTES};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    Waiting,
    Current,
    Stale,
    Error,
}
#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub path: Option<String>,
    pub document: crate::status::domain::Document,
    pub revision: u64,
    pub health: Health,
    pub error: Option<String>,
    pub modified_at: Option<String>,
}

pub struct ViewerState {
    pub path: PathBuf,
    pub store: StatusStore,
    pub snapshot: Mutex<Snapshot>,
    coordinator: RefreshCoordinator,
    watcher_error: Mutex<Option<String>>,
    retention_error: Mutex<Option<String>>,
}
impl ViewerState {
    pub fn new(path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            store: StatusStore::new(path.clone()),
            path,
            snapshot: Mutex::new(Snapshot {
                path: None,
                document: Default::default(),
                revision: 0,
                health: Health::Waiting,
                error: None,
                modified_at: None,
            }),
            coordinator: RefreshCoordinator::new(),
            watcher_error: Mutex::new(None),
            retention_error: Mutex::new(None),
        })
    }
}

struct RefreshCoordinator {
    serial: Mutex<()>,
}
impl RefreshCoordinator {
    fn new() -> Self {
        Self {
            serial: Mutex::new(()),
        }
    }
    fn refresh<R, P>(&self, read: R, publish: P)
    where
        R: FnOnce() -> Snapshot,
        P: FnOnce(Snapshot),
    {
        let _serial = self.serial.lock().unwrap();
        let next = read();
        publish(next);
    }
}

fn modified_at(path: &std::path::Path) -> Option<String> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|time| chrono::DateTime::<chrono::Local>::from(time).to_rfc3339())
}
fn snapshot_for(state: &ViewerState, previous: &Snapshot) -> Snapshot {
    let modified_at = modified_at(&state.path);
    let stale = |health, error| Snapshot {
        path: Some(state.path.display().to_string()),
        document: previous.document.clone(),
        revision: previous.revision.saturating_add(1),
        health,
        error,
        modified_at: modified_at.clone(),
    };
    match std::fs::metadata(&state.path) {
        Ok(meta) if !meta.is_file() => stale(
            Health::Error,
            Some("The selected path is not a regular file".into()),
        ),
        Ok(meta) if meta.len() > MAX_STATUS_BYTES as u64 => stale(
            Health::Error,
            Some("The file is larger than the 1 MiB limit".into()),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => stale(Health::Waiting, None),
        Err(error) => stale(Health::Error, Some(format!("Unable to read file: {error}"))),
        Ok(_) => match state.store.read() {
            Ok(document) => {
                let warning = state
                    .watcher_error
                    .lock()
                    .unwrap()
                    .clone()
                    .or_else(|| state.retention_error.lock().unwrap().clone());
                Snapshot {
                    path: Some(state.path.display().to_string()),
                    document,
                    revision: previous.revision.saturating_add(1),
                    health: if warning.is_some() {
                        Health::Error
                    } else {
                        Health::Current
                    },
                    error: warning,
                    modified_at,
                }
            }
            Err(error) => stale(Health::Stale, Some(error.to_string())),
        },
    }
}

pub fn update(app: &AppHandle, state: &ViewerState) {
    state.coordinator.refresh(
        || {
            let previous = state.snapshot.lock().unwrap().clone();
            snapshot_for(state, &previous)
        },
        |snapshot| {
            *state.snapshot.lock().unwrap() = snapshot.clone();
            let _ = app.emit("source-update", snapshot);
        },
    );
}
pub fn publish_watcher_error(app: &AppHandle, state: &ViewerState, message: impl Into<String>) {
    *state.watcher_error.lock().unwrap() = Some(message.into());
    update(app, state);
}
pub fn watcher_recovered(app: &AppHandle, state: &ViewerState) {
    *state.watcher_error.lock().unwrap() = None;
    update(app, state);
}
pub fn publish_retention_error(app: &AppHandle, state: &ViewerState, message: impl Into<String>) {
    *state.retention_error.lock().unwrap() = Some(message.into());
    update(app, state);
}
pub fn retention_recovered(app: &AppHandle, state: &ViewerState) {
    *state.retention_error.lock().unwrap() = None;
    update(app, state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::domain::Point;
    use std::sync::{Arc, Barrier};

    #[test]
    fn concurrent_refreshes_publish_in_revision_order() {
        let coordinator = Arc::new(RefreshCoordinator::new());
        let snapshot = Arc::new(Mutex::new(Snapshot {
            path: None,
            document: Default::default(),
            revision: 0,
            health: Health::Waiting,
            error: None,
            modified_at: None,
        }));
        let barrier = Arc::new(Barrier::new(3));
        let published = Arc::new(Mutex::new(Vec::new()));
        std::thread::scope(|scope| {
            for _ in 0..2 {
                let coordinator = coordinator.clone();
                let barrier = barrier.clone();
                let published = published.clone();
                let snapshot = snapshot.clone();
                scope.spawn(move || {
                    barrier.wait();
                    coordinator.refresh(
                        || {
                            let mut next = snapshot.lock().unwrap().clone();
                            next.revision += 1;
                            next
                        },
                        |next| {
                            *snapshot.lock().unwrap() = next.clone();
                            published.lock().unwrap().push(next.revision);
                        },
                    );
                });
            }
            barrier.wait();
        });
        assert_eq!(*published.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn successful_reads_replace_the_document_during_a_watcher_outage() {
        let directory = tempfile::tempdir().unwrap();
        let state = ViewerState::new(directory.path().join("status.json"));
        state
            .store
            .upsert(
                "project".into(),
                "session".into(),
                "main".into(),
                vec![Point {
                    text: "first".into(),
                    done: false,
                    link: None,
                    submitted_at: None,
                    id: None,
                }],
            )
            .unwrap();
        let first = snapshot_for(&state, &state.snapshot.lock().unwrap());
        *state.snapshot.lock().unwrap() = first.clone();
        *state.watcher_error.lock().unwrap() = Some("watch unavailable".into());
        state
            .store
            .upsert(
                "project".into(),
                "session".into(),
                "main".into(),
                vec![Point {
                    text: "second".into(),
                    done: false,
                    link: None,
                    submitted_at: None,
                    id: first.document.entries[0].points[0].id.clone(),
                }],
            )
            .unwrap();
        let second = snapshot_for(&state, &first);
        assert_eq!(second.document.entries[0].points[0].text, "second");
        assert_eq!(second.error.as_deref(), Some("watch unavailable"));
    }
}
