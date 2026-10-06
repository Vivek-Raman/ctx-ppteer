use chrono::{DateTime, Local};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

const MAX_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Serialize)]
pub struct Snapshot {
    pub path: Option<String>,
    pub markdown: String,
    pub revision: u64,
    pub health: String,
    pub error: Option<String>,
    pub modified_at: Option<String>,
}

pub struct ViewerState {
    pub path: Mutex<Option<PathBuf>>,
    pub snapshot: Mutex<Snapshot>,
}

pub fn new_viewer_state() -> Arc<ViewerState> {
    Arc::new(ViewerState {
        path: Mutex::new(None),
        snapshot: Mutex::new(Snapshot {
            path: None,
            markdown: String::new(),
            revision: 0,
            health: "waiting".into(),
            error: None,
            modified_at: None,
        }),
    })
}

fn read_snapshot(path: &Path, revision: u64, previous: &Snapshot) -> Snapshot {
    let metadata = fs::metadata(path);
    let modified_at = metadata
        .as_ref()
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .map(|time| {
            let date_time: DateTime<Local> = time.into();
            date_time.to_rfc3339()
        });
    let stale_snapshot = || Snapshot {
        path: Some(path.display().to_string()),
        markdown: previous.markdown.clone(),
        revision,
        health: "stale".into(),
        error: None,
        modified_at: modified_at.clone(),
    };

    match metadata {
        Ok(metadata) if !metadata.is_file() => Snapshot {
            error: Some("The selected path is not a regular file".into()),
            health: "error".into(),
            ..stale_snapshot()
        },
        Ok(metadata) if metadata.len() > MAX_BYTES => Snapshot {
            error: Some("The file is larger than the 1 MiB limit".into()),
            health: "error".into(),
            ..stale_snapshot()
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Snapshot {
            health: "waiting".into(),
            error: Some("Waiting for the file to appear".into()),
            modified_at: None,
            ..stale_snapshot()
        },
        Err(error) => Snapshot {
            error: Some(format!("Unable to read file: {error}")),
            ..stale_snapshot()
        },
        Ok(_) => match fs::read(path)
            .and_then(|bytes| String::from_utf8(bytes).map_err(std::io::Error::other))
        {
            Ok(markdown) => Snapshot {
                markdown,
                health: "current".into(),
                error: None,
                ..stale_snapshot()
            },
            Err(error) => Snapshot {
                error: Some(format!("Unable to decode UTF-8: {error}")),
                ..stale_snapshot()
            },
        },
    }
}

pub fn update(app: &AppHandle, state: &ViewerState) {
    let Some(path) = state.path.lock().ok().and_then(|path| path.clone()) else {
        return;
    };
    let snapshot = {
        let previous = state.snapshot.lock().unwrap();
        read_snapshot(&path, previous.revision + 1, &previous)
    };
    *state.snapshot.lock().unwrap() = snapshot.clone();
    let _ = app.emit("source-update", snapshot);
}

pub fn start_watcher(app: AppHandle, state: Arc<ViewerState>) {
    thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let Ok(mut watcher): Result<RecommendedWatcher, _> = notify::recommended_watcher(tx) else {
            return;
        };
        let mut watched_parent: Option<PathBuf> = None;
        let mut last_reconcile = Instant::now();
        loop {
            let path = state.path.lock().ok().and_then(|path| path.clone());
            if let Some(path) = path {
                let parent = path.parent().unwrap_or(Path::new(".")).to_path_buf();
                if watched_parent.as_ref() != Some(&parent) {
                    if let Some(old_parent) = watched_parent.take() {
                        let _ = watcher.unwatch(&old_parent);
                    }
                    if watcher.watch(&parent, RecursiveMode::NonRecursive).is_ok() {
                        watched_parent = Some(parent);
                    }
                }
                if rx.recv_timeout(Duration::from_millis(250)).is_ok() {
                    thread::sleep(Duration::from_millis(120));
                    update(&app, &state);
                }
                if last_reconcile.elapsed() >= Duration::from_secs(30) {
                    update(&app, &state);
                    last_reconcile = Instant::now();
                }
            } else {
                thread::sleep(Duration::from_millis(500));
            }
        }
    });
}
