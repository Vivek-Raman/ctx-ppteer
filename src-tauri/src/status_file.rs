use std::{io::Write, path::Path};

pub const MAX_STATUS_BYTES: usize = 1024 * 1024;

pub fn write_markdown(path: &Path, markdown: &str) -> Result<usize, String> {
    let bytes = markdown.as_bytes();
    if bytes.len() > MAX_STATUS_BYTES {
        return Err("The status document exceeds the 1 MiB limit".into());
    }
    let parent = path
        .parent()
        .filter(|parent| parent.is_dir())
        .ok_or_else(|| {
            format!(
                "The status file parent directory does not exist: {}",
                path.display()
            )
        })?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("Unable to prepare status update: {error}"))?;
    temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|error| format!("Unable to write status update: {error}"))?;
    temporary
        .persist(path)
        .map_err(|error| format!("Unable to replace status document: {}", error.error))?;
    Ok(bytes.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_status_content() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("status.md");
        std::fs::write(&path, "old").unwrap();
        assert_eq!(write_markdown(&path, "# Current").unwrap(), 9);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "# Current");
    }

    #[test]
    fn rejects_content_over_the_limit() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("status.md");
        assert!(write_markdown(&path, &"x".repeat(MAX_STATUS_BYTES + 1)).is_err());
        assert!(!path.exists());
    }
}
