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
