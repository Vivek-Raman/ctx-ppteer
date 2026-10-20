use crate::{
    preferences::SettingsOwner,
    status::retention::clear_expired_points,
    status::snapshot::{
        publish_retention_error, publish_watcher_error, retention_recovered, update,
        watcher_recovered, ViewerState,
    },
};
use notify::{RecursiveMode, Watcher};
use std::{
    path::PathBuf,
    sync::{mpsc, Arc},
    thread,
    time::{Duration, Instant},
};
use tauri::AppHandle;

pub fn start(app: AppHandle, state: Arc<ViewerState>, owner: Arc<SettingsOwner>) {
    thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let mut watcher = match notify::recommended_watcher(tx.clone()) {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                publish_watcher_error(
                    &app,
                    &state,
                    format!("Unable to start status file watcher: {error}"),
                );
                None
            }
        };
        let mut watched_parent: Option<PathBuf> = None;
        let mut last_reconcile = Instant::now();
        let mut next_watch_attempt = Instant::now();
        let mut retry_delay = Duration::from_millis(250);
        let mut last_watcher_error: Option<String> = None;
        loop {
            if watcher.is_none() && Instant::now() >= next_watch_attempt {
                if let Ok(new_watcher) = notify::recommended_watcher(tx.clone()) {
                    watcher = Some(new_watcher);
                } else {
                    next_watch_attempt = Instant::now() + retry_delay;
                    retry_delay = (retry_delay * 2).min(Duration::from_secs(30));
                }
            }
            let parent = state
                .path
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf();
            if watched_parent.as_ref() != Some(&parent) {
                if let Some(old) = watched_parent.take() {
                    if let Some(w) = watcher.as_mut() {
                        let _ = w.unwatch(&old);
                    }
                }
                if let Some(w) = watcher.as_mut() {
                    match w.watch(&parent, RecursiveMode::NonRecursive) {
                        Ok(()) => {
                            watched_parent = Some(parent);
                            last_watcher_error = None;
                            watcher_recovered(&app, &state);
                        }
                        Err(error) => {
                            let message = format!("Unable to watch the status directory: {error}");
                            if last_watcher_error.as_deref() != Some(&message) {
                                publish_watcher_error(&app, &state, message.clone());
                                last_watcher_error = Some(message);
                            }
                            watcher = None;
                            next_watch_attempt = Instant::now() + retry_delay;
                            retry_delay = (retry_delay * 2).min(Duration::from_secs(30));
                        }
                    }
                }
            }
            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(Ok(_event)) => {
                    thread::sleep(Duration::from_millis(120));
                    update(&app, &state);
                }
                Ok(Err(error)) => {
                    let message = format!("Status watcher error: {error}");
                    if last_watcher_error.as_deref() != Some(&message) {
                        publish_watcher_error(&app, &state, message.clone());
                        last_watcher_error = Some(message);
                    }
                    watched_parent = None;
                    watcher = None;
                    next_watch_attempt = Instant::now() + retry_delay;
                    retry_delay = (retry_delay * 2).min(Duration::from_secs(30));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    publish_watcher_error(&app, &state, "Status watcher stopped");
                    watched_parent = None;
                    watcher = None;
                    next_watch_attempt = Instant::now() + retry_delay;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if last_reconcile.elapsed() >= Duration::from_secs(30) {
                if let Err(error) = clear_expired_points(&state, &owner) {
                    publish_retention_error(
                        &app,
                        &state,
                        format!("Unable to prune status points: {error}"),
                    );
                } else {
                    retention_recovered(&app, &state);
                }
                update(&app, &state);
                last_reconcile = Instant::now();
            }
        }
    });
}
