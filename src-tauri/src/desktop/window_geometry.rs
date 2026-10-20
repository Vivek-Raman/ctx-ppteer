use serde::{Deserialize, Serialize};
use tauri::{PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::preferences::SettingsOwner;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct WindowState {
    pub(crate) monitor_name: Option<String>,
    pub(crate) relative_x: f64,
    pub(crate) relative_y: f64,
    #[serde(default)]
    pub(crate) width: Option<u32>,
    #[serde(default)]
    pub(crate) height: Option<u32>,
    #[serde(default)]
    pub(crate) maximized: bool,
}

pub fn restore(window: &WebviewWindow, owner: &SettingsOwner) {
    let Some(state) = owner.snapshot().window else {
        return;
    };
    let Ok(monitors) = window.available_monitors() else {
        return;
    };
    let primary = window.primary_monitor().ok().flatten();
    let Some(monitor) = monitors
        .iter()
        .find(|monitor| monitor.name() == state.monitor_name.as_ref())
        .or(primary.as_ref())
    else {
        return;
    };
    let area = monitor.work_area();
    if let (Some(width), Some(height)) = (state.width, state.height) {
        let _ = window.set_size(PhysicalSize::new(
            width.min(area.size.width).max(1),
            height.min(area.size.height).max(1),
        ));
    }
    let Ok(size) = window.outer_size() else {
        return;
    };
    let max_x = area.position.x + area.size.width.saturating_sub(size.width) as i32;
    let max_y = area.position.y + area.size.height.saturating_sub(size.height) as i32;
    let x = ((area.position.x as f64 + state.relative_x * area.size.width as f64).round() as i32)
        .clamp(area.position.x, max_x.max(area.position.x));
    let y = ((area.position.y as f64 + state.relative_y * area.size.height as f64).round() as i32)
        .clamp(area.position.y, max_y.max(area.position.y));
    let _ = window.set_position(PhysicalPosition::new(x, y));
    if state.maximized {
        let _ = window.maximize();
    }
}

pub fn capture(window: &WebviewWindow, owner: &SettingsOwner) {
    let Ok(maximized) = window.is_maximized() else {
        return;
    };
    if maximized {
        let _ = owner.set_window_maximized(true);
        return;
    }
    let (Ok(position), Ok(size), Ok(monitors)) = (
        window.outer_position(),
        window.outer_size(),
        window.available_monitors(),
    ) else {
        return;
    };
    let Some(monitor) = monitors.iter().find(|monitor| {
        let area = monitor.work_area();
        position.x >= area.position.x
            && position.x < area.position.x + area.size.width as i32
            && position.y >= area.position.y
            && position.y < area.position.y + area.size.height as i32
    }) else {
        return;
    };
    let area = monitor.work_area();
    let state = WindowState {
        monitor_name: monitor.name().cloned(),
        relative_x: (position.x - area.position.x) as f64 / area.size.width.max(1) as f64,
        relative_y: (position.y - area.position.y) as f64 / area.size.height.max(1) as f64,
        width: Some(size.width),
        height: Some(size.height),
        maximized: false,
    };
    let _ = owner.set_window(state);
}
