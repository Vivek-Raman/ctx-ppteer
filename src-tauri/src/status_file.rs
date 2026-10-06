use std::{fs, io::Write, path::Path};

pub const MAX_STATUS_BYTES: usize = 1024 * 1024;

pub fn read_project_status(path: &Path, folder_name: &str) -> Result<Option<String>, String> {
    let markdown = read_markdown(path)?;
    let title = project_title(folder_name)?;
    Ok(project_section(&markdown, &title).map(str::to_owned))
}

pub fn write_project_status(path: &Path, folder_name: &str, status: &str) -> Result<usize, String> {
    let title = project_title(folder_name)?;
    if status.trim().is_empty() {
        return Err("The status text cannot be empty".into());
    }

    let section = format!("## {title}\n\n{}\n", status.trim());
    let markdown = read_markdown(path)?;
    let updated = if let Some((start, end)) = project_section_range(&markdown, &title) {
        let separator = if end < markdown.len() && !markdown[end..].starts_with('\n') {
            "\n"
        } else {
            ""
        };
        format!(
            "{}{}{}{}",
            &markdown[..start],
            section,
            separator,
            &markdown[end..]
        )
    } else if markdown.trim().is_empty() {
        section
    } else {
        format!("{}\n\n{section}", markdown.trim_end())
    };
    write_markdown(path, &updated)
}

fn read_markdown(path: &Path) -> Result<String, String> {
    match fs::read(path) {
        Ok(bytes) => {
            if bytes.len() > MAX_STATUS_BYTES {
                return Err("The status document exceeds the 1 MiB limit".into());
            }
            String::from_utf8(bytes)
                .map_err(|error| format!("Unable to decode status document as UTF-8: {error}"))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("Unable to read status document: {error}")),
    }
}

fn project_title(folder_name: &str) -> Result<String, String> {
    if folder_name.is_empty() || folder_name.contains(['\n', '\r']) {
        return Err("The project folder name must be a non-empty single line".into());
    }
    Ok(folder_name.to_owned())
}

fn project_section<'a>(markdown: &'a str, title: &str) -> Option<&'a str> {
    let (start, end) = project_section_range(markdown, title)?;
    let section = &markdown[start..end];
    let header = format!("## {title}\n\n");
    section.strip_prefix(&header).map(str::trim_end)
}

fn project_section_range(markdown: &str, title: &str) -> Option<(usize, usize)> {
    let header = format!("## {title}");
    let mut offset = 0;
    let mut start = None;
    for raw_line in markdown.split_inclusive('\n') {
        let line = raw_line.trim_end_matches(['\n', '\r']);
        if start.is_some() && line.starts_with("## ") {
            return Some((start.unwrap(), offset));
        }
        if line == header {
            start = Some(offset);
        }
        offset += raw_line.len();
    }
    start.map(|start| (start, markdown.len()))
}

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

    #[test]
    fn replaces_only_the_requested_project_section() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("status.md");
        write_project_status(&path, "ctx-ppteer", "main\n\n- First status.").unwrap();
        write_project_status(&path, "other-project", "develop\n\n- Other status.").unwrap();
        write_project_status(&path, "ctx-ppteer", "feature/status\n\n- Updated status.").unwrap();

        assert_eq!(
            read_project_status(&path, "ctx-ppteer").unwrap(),
            Some("feature/status\n\n- Updated status.".into())
        );
        assert_eq!(
            read_project_status(&path, "other-project").unwrap(),
            Some("develop\n\n- Other status.".into())
        );
    }

    #[test]
    fn rejects_invalid_folder_names() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("status.md");
        assert!(write_project_status(&path, "", "main").is_err());
        assert!(write_project_status(&path, "project\nname", "main").is_err());
    }

    #[test]
    fn preserves_the_folder_name_in_the_heading() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("status.md");

        write_project_status(&path, "myAPI-tool_v2", "main").unwrap();

        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "## myAPI-tool_v2\n\nmain\n"
        );
    }
}
