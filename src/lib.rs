//! open-scrantic: a reimplementation of Screen Antics: Johnny Castaway (Sierra/Dynamix, 1992)
//! that runs on the original, installed screensaver files.

// Only the Windows screensaver build needs `unsafe` (`windows_ui`); every other build forbids it outright.
#![cfg_attr(not(feature = "screensaver"), forbid(unsafe_code))]

pub mod assets;
pub mod audio;
pub mod engine;
pub mod present;
#[cfg(all(feature = "screensaver", not(windows)))]
compile_error!("the `screensaver` feature builds the Windows .scr and only works on Windows");
#[cfg(all(feature = "screensaver", windows))]
pub mod windows_ui;

/// Directory of the running executable: where the screensaver files and `SCRANTIC.INI` live.
pub fn app_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_else(|| ".".into())
}

/// The folder in the user's profile (`%APPDATA%\OpenScrantic`) where the screensaver build also looks for the
/// original files: Windows copies only the `.scr` to `System32`, which does not have them.
#[cfg(feature = "screensaver")]
fn user_dir_in(appdata: Option<std::ffi::OsString>) -> Option<std::path::PathBuf> {
    appdata.map(|a| std::path::PathBuf::from(a).join("OpenScrantic"))
}

/// `%APPDATA%\OpenScrantic`, if `APPDATA` is set (screensaver build only).
#[cfg(feature = "screensaver")]
pub fn user_dir() -> Option<std::path::PathBuf> {
    user_dir_in(std::env::var_os("APPDATA"))
}

/// Folders searched, in order, for `SCRANTIC.SCR`, `RESOURCE.MAP` and `RESOURCE.001`: that of the
/// executable and, in the screensaver build, the user folder after it. The first one that has
/// `SCRANTIC.SCR` is the install.
pub fn data_dirs() -> Vec<std::path::PathBuf> {
    #[allow(unused_mut)]
    let mut dirs = vec![app_dir()];
    #[cfg(feature = "screensaver")]
    dirs.extend(user_dir());
    dirs
}

/// Folder of `SCRANTIC.INI`: always the one the original files are read from (see [`assets::locate`]);
/// the folder of the executable when there is no install.
pub fn settings_dir() -> std::path::PathBuf {
    static DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dirs = data_dirs();
        assets::locate(&dirs).map_or_else(|_| app_dir(), std::path::Path::to_path_buf)
    })
    .clone()
}

#[cfg(all(test, feature = "screensaver"))]
mod tests {
    use super::*;

    #[test]
    fn user_dir_is_openscrantic_under_appdata() {
        let root = std::path::PathBuf::from("appdata-root");
        let dir = user_dir_in(Some(root.clone().into_os_string())).unwrap();
        assert_eq!(dir, root.join("OpenScrantic"));
        assert_eq!(user_dir_in(None), None);
    }
}
