use std::{env, ffi::OsString, path::Path};

/// Prevent linuxdeploy's AppRun Python and library paths from leaking into
/// external engines. The AppImage needs those paths for its own process, but
/// host-installed Python applications such as yt-dlp and gallery-dl must use
/// their host Python runtime and standard library.
pub fn sanitize_tokio_command(command: &mut tokio::process::Command) {
    let Some(app_dir) = env::var_os("APPDIR") else {
        return;
    };
    let app_dir = Path::new(&app_dir);

    sanitize_python_home(app_dir, |key| {
        command.env_remove(key);
    });

    for key in ["PYTHONPATH", "LD_LIBRARY_PATH"] {
        match filtered_path_variable(key, app_dir) {
            Some(value) => {
                command.env(key, value);
            }
            None => {
                command.env_remove(key);
            }
        }
    }
}

pub fn sanitize_std_command(command: &mut std::process::Command) {
    let Some(app_dir) = env::var_os("APPDIR") else {
        return;
    };
    let app_dir = Path::new(&app_dir);

    sanitize_python_home(app_dir, |key| {
        command.env_remove(key);
    });

    for key in ["PYTHONPATH", "LD_LIBRARY_PATH"] {
        match filtered_path_variable(key, app_dir) {
            Some(value) => {
                command.env(key, value);
            }
            None => {
                command.env_remove(key);
            }
        }
    }
}

fn sanitize_python_home(app_dir: &Path, mut remove: impl FnMut(&str)) {
    if env::var_os("PYTHONHOME")
        .as_deref()
        .is_some_and(|value| Path::new(value).starts_with(app_dir))
    {
        remove("PYTHONHOME");
    }
}

fn filtered_path_variable(key: &str, app_dir: &Path) -> Option<OsString> {
    let value = env::var_os(key)?;
    filtered_path_value(&value, app_dir)
}

fn filtered_path_value(value: &OsString, app_dir: &Path) -> Option<OsString> {
    let retained = env::split_paths(value)
        .filter(|path| !path.starts_with(app_dir))
        .collect::<Vec<_>>();

    if retained.is_empty() {
        None
    } else {
        env::join_paths(retained).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::filtered_path_value;
    use std::{ffi::OsString, path::Path};

    #[test]
    fn removes_appimage_paths_and_preserves_host_paths() {
        let value = OsString::from("/tmp/example.AppDir/usr/lib:/opt/custom/lib");

        let filtered = filtered_path_value(&value, Path::new("/tmp/example.AppDir"));

        assert_eq!(filtered, Some(OsString::from("/opt/custom/lib")));
    }
}
