use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::error::ConfigError;

/// Writes `text` to `path` in one step: first to a temporary file in the
/// same directory, then a rename over `path`. A reader sees the old text or
/// the new text, never a part.
///
/// When `path` is a symbolic link, the file that it points to gets the new
/// text and the link stays. An existing file keeps its permissions. A new
/// file gets the permissions 0644 and a missing directory is made.
pub(crate) fn write_atomic(path: &Path, text: &str) -> Result<(), ConfigError> {
    let io_error = |error: std::io::Error| ConfigError::io(path, &error);
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
    let directory = target.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(directory).map_err(io_error)?;
    let permissions = match fs::metadata(&target) {
        Ok(metadata) => metadata.permissions(),
        Err(_) => fs::Permissions::from_mode(0o644),
    };
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut temporary = tempfile::Builder::new()
        .prefix(&format!(".{name}."))
        .suffix(".tmp")
        .tempfile_in(directory)
        .map_err(io_error)?;
    temporary.write_all(text.as_bytes()).map_err(io_error)?;
    temporary.as_file().sync_all().map_err(io_error)?;
    fs::set_permissions(temporary.path(), permissions).map_err(io_error)?;
    temporary
        .persist(&target)
        .map_err(|error| io_error(error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;

    use super::*;

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).expect("metadata").permissions().mode() & 0o777
    }

    #[test]
    fn writes_a_new_file_and_its_directory() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("petlauncher").join("settings.toml");

        write_atomic(&path, "window_width = 800\n").expect("write");

        assert_eq!(
            fs::read_to_string(&path).expect("read"),
            "window_width = 800\n"
        );
        assert_eq!(mode(&path), 0o644);
    }

    #[test]
    fn replaces_the_text_and_keeps_the_permissions() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("settings.toml");
        fs::write(&path, "old\n").expect("write old");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("chmod");

        write_atomic(&path, "new\n").expect("write");

        assert_eq!(fs::read_to_string(&path).expect("read"), "new\n");
        assert_eq!(mode(&path), 0o600);
    }

    #[test]
    fn leaves_no_temporary_file() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let path = dir.path().join("shortcuts.toml");

        write_atomic(&path, "a\n").expect("first write");
        write_atomic(&path, "b\n").expect("second write");

        let names: Vec<String> = fs::read_dir(dir.path())
            .expect("list")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, ["shortcuts.toml"]);
    }

    #[test]
    fn writes_the_target_of_a_symbolic_link_and_keeps_the_link() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let dotfiles = dir.path().join("dotfiles");
        let config = dir.path().join("config");
        fs::create_dir_all(&dotfiles).expect("dotfiles");
        fs::create_dir_all(&config).expect("config");
        let target = dotfiles.join("settings.toml");
        let link = config.join("settings.toml");
        fs::write(&target, "old\n").expect("write old");
        symlink(&target, &link).expect("symlink");

        write_atomic(&link, "new\n").expect("write");

        assert!(fs::symlink_metadata(&link)
            .expect("link metadata")
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&target).expect("read"), "new\n");
    }

    #[test]
    fn gives_an_io_error_when_the_directory_cannot_be_made() {
        let dir = tempfile::tempdir().expect("temporary directory");
        let file = dir.path().join("a-file");
        fs::write(&file, "").expect("write");

        let result = write_atomic(&file.join("settings.toml"), "x\n");

        assert!(matches!(result, Err(ConfigError::Io { .. })), "{result:?}");
    }
}
