use chrono::{DateTime, Local};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::{fs, path::{Path, PathBuf}, sync::{mpsc, Arc, Mutex}, thread, time::{Duration, Instant}};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

const MAX_BYTES: u64 = 1024 * 1024;
const DEFAULT_SOURCE_FILE: &str = "agent-status.md";

#[derive(Clone, Serialize)]
struct Snapshot { path: Option<String>, markdown: String, revision: u64, health: String, error: Option<String>, modified_at: Option<String> }
struct ViewerState { path: Mutex<Option<PathBuf>>, snapshot: Mutex<Snapshot> }

fn read_snapshot(path: &Path, revision: u64, previous: &Snapshot) -> Snapshot {
    let metadata = fs::metadata(path);
    let modified_at = metadata.as_ref().ok().and_then(|m| m.modified().ok()).map(|t| { let dt: DateTime<Local> = t.into(); dt.to_rfc3339() });
    let base = || Snapshot { path: Some(path.display().to_string()), markdown: previous.markdown.clone(), revision, health: "stale".into(), error: None, modified_at: modified_at.clone() };
    match metadata {
        Ok(meta) if !meta.is_file() => Snapshot { error: Some("The selected path is not a regular file".into()), health: "error".into(), ..base() },
        Ok(meta) if meta.len() > MAX_BYTES => Snapshot { error: Some("The file is larger than the 1 MiB limit".into()), health: "error".into(), ..base() },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Snapshot { health: "waiting".into(), error: Some("Waiting for the file to appear".into()), modified_at: None, ..base() },
        Err(e) => Snapshot { error: Some(format!("Unable to read file: {e}")), ..base() },
        Ok(_) => match fs::read(path).and_then(|bytes| String::from_utf8(bytes).map_err(std::io::Error::other)) {
            Ok(markdown) => Snapshot { markdown, health: "current".into(), error: None, ..base() },
            Err(e) => Snapshot { error: Some(format!("Unable to decode UTF-8: {e}")), ..base() },
        },
    }
}

fn update(app: &AppHandle, state: &ViewerState) {
    let Some(path) = state.path.lock().ok().and_then(|p| p.clone()) else { return };
    let snapshot = { let previous = state.snapshot.lock().unwrap(); read_snapshot(&path, previous.revision + 1, &previous) };
    *state.snapshot.lock().unwrap() = snapshot.clone();
    let _ = app.emit("source-update", snapshot);
}

fn default_source_path(app: &AppHandle) -> Result<PathBuf, String> {
    let directory = app.path().app_data_dir().map_err(|e| format!("Unable to find the application data directory: {e}"))?;
    fs::create_dir_all(&directory).map_err(|e| format!("Unable to create the application data directory: {e}"))?;
    Ok(directory.join(DEFAULT_SOURCE_FILE))
}

fn start_watcher(app: AppHandle, state: Arc<ViewerState>) {
    thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let Ok(mut watcher): Result<RecommendedWatcher, _> = notify::recommended_watcher(tx) else { return };
        let mut watched_parent: Option<PathBuf> = None;
        let mut last_reconcile = Instant::now();
        loop {
            let path = state.path.lock().ok().and_then(|p| p.clone());
            if let Some(path) = path {
                let parent = path.parent().unwrap_or(Path::new(".")).to_path_buf();
                if watched_parent.as_ref() != Some(&parent) { if let Some(old) = watched_parent.take() { let _ = watcher.unwatch(&old); } if watcher.watch(&parent, RecursiveMode::NonRecursive).is_ok() { watched_parent = Some(parent); } }
                if rx.recv_timeout(Duration::from_millis(250)).is_ok() { thread::sleep(Duration::from_millis(120)); update(&app, &state); }
                if last_reconcile.elapsed() >= Duration::from_secs(30) { update(&app, &state); last_reconcile = Instant::now(); }
            } else { thread::sleep(Duration::from_millis(500)); }
        }
    });
}

#[tauri::command] fn get_snapshot(state: State<'_, Arc<ViewerState>>) -> Snapshot { state.snapshot.lock().unwrap().clone() }
#[tauri::command] fn set_source(path: String, app: AppHandle, state: State<'_, Arc<ViewerState>>) -> Result<(), String> { if path.trim().is_empty() { return Err("A source path is required".into()); } state.path.lock().map_err(|_| "State unavailable")?.replace(PathBuf::from(path)); update(&app, &state); Ok(()) }
#[tauri::command] fn use_default_source(app: AppHandle, state: State<'_, Arc<ViewerState>>) -> Result<(), String> { let path = default_source_path(&app)?; state.path.lock().map_err(|_| "State unavailable")?.replace(path); update(&app, &state); Ok(()) }
#[tauri::command] fn get_default_source(app: AppHandle) -> Result<String, String> { Ok(default_source_path(&app)?.display().to_string()) }
#[tauri::command] fn refresh_source(app: AppHandle, state: State<'_, Arc<ViewerState>>) { update(&app, &state); }
#[tauri::command] fn set_pinned(pinned: bool, app: AppHandle) -> Result<(), String> { app.get_webview_window("main").ok_or("Main window unavailable")?.set_always_on_top(pinned).map_err(|e| e.to_string()) }
#[tauri::command] async fn pick_source(app: AppHandle) -> Option<String> { app.dialog().file().add_filter("Markdown", &["md", "markdown", "txt"]).blocking_pick_file().map(|p| p.to_string()) }

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = Arc::new(ViewerState { path: Mutex::new(None), snapshot: Mutex::new(Snapshot { path: None, markdown: String::new(), revision: 0, health: "waiting".into(), error: None, modified_at: None }) });
    tauri::Builder::default().plugin(tauri_plugin_dialog::init()).manage(state.clone()).invoke_handler(tauri::generate_handler![get_snapshot, set_source, use_default_source, get_default_source, refresh_source, set_pinned, pick_source]).setup(|app| { let state = app.state::<Arc<ViewerState>>().inner().clone(); let path = default_source_path(app.handle()).map_err(|e| std::io::Error::other(e))?; state.path.lock().map_err(|_| std::io::Error::other("State unavailable"))?.replace(path); update(app.handle(), &state); start_watcher(app.handle().clone(), state); Ok(()) }).run(tauri::generate_context!()).expect("error while running Tauri application");
}
