use std::ffi::OsStr;
use std::path::PathBuf;

pub fn find_in_path(name: &str, search_path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(search_path).find_map(|directory| {
        if directory.as_os_str().is_empty() {
            return None;
        }
        let path = directory.join(name);
        let metadata = path.metadata().ok()?;
        if !metadata.is_file() {
            return None;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o111 == 0 {
                return None;
            }
        }
        Some(path)
    })
}

pub fn can_utils_warning() -> String {
    if !cfg!(target_os = "linux") {
        return String::new();
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    let missing: Vec<_> = ["candump", "cansend"]
        .into_iter()
        .filter(|name| find_in_path(name, &path).is_none())
        .collect();
    if missing.is_empty() {
        String::new()
    } else {
        format!(
            "Missing Linux can-utils tools: {}. Install can-utils (Debian/Ubuntu: sudo apt install can-utils), ensure its tools are on PATH, then restart CyderVis.",
            missing.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_lookup_handles_missing_tools_and_spaces() {
        let directory = std::env::temp_dir().join(format!("cyder tools {}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let tool = directory.join("candump");
        std::fs::write(&tool, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let path = std::env::join_paths([&directory]).unwrap();
        assert_eq!(find_in_path("candump", &path), Some(tool.clone()));
        assert!(find_in_path("cansend", &path).is_none());
        assert!(find_in_path("candump", OsStr::new("")).is_none());
        std::fs::remove_file(tool).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
