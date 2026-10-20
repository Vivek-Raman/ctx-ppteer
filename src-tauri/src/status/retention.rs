use crate::{preferences::SettingsOwner, status::snapshot::ViewerState};
use std::time::Duration;

pub fn clear_expired_points(state: &ViewerState, owner: &SettingsOwner) -> Result<bool, String> {
    let Some(minutes) = owner.snapshot().auto_clear_after_minutes else {
        return Ok(false);
    };
    state
        .store
        .prune(Duration::from_secs(minutes.saturating_mul(60)))
        .map_err(|error| error.to_string())
}
