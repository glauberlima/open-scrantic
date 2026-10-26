//! Loading of the original, installed screensaver files: SCRANTIC.SCR, RESOURCE.MAP and RESOURCE.001.

pub mod exe;
pub mod resource;

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub use exe::ExeData;
pub use resource::Archive;

#[derive(Debug)]
pub enum AssetError {
    Io(std::io::Error),
    Missing(String),
    Format(String),
}

/// Longest piece of data-file text shown in an error message, in characters.
const MAX_SHOWN: usize = 256;

/// Text that came from the data files, safe for a terminal: control characters (ESC, BEL...) are
/// escaped and the length is capped.
fn sanitize(s: &str) -> String {
    let mut out: String = s
        .chars()
        .take(MAX_SHOWN)
        .flat_map(|c| {
            if c.is_control() {
                c.escape_default().collect()
            } else {
                vec![c]
            }
        })
        .collect();
    if s.chars().nth(MAX_SHOWN).is_some() {
        out.push_str("...");
    }
    out
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Missing(name) => write!(f, "missing screensaver file: {}", sanitize(name)),
            Self::Format(msg) => write!(f, "invalid screensaver data: {}", sanitize(msg)),
        }
    }
}

impl std::error::Error for AssetError {}

impl From<std::io::Error> for AssetError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, AssetError>;

/// Bounds-checked slice of `len` bytes at `start`.
fn bytes(d: &[u8], start: usize, len: usize) -> Result<&[u8]> {
    start
        .checked_add(len)
        .and_then(|end| d.get(start..end))
        .ok_or_else(|| AssetError::Format("truncated data".into()))
}

fn u16_at(d: &[u8], o: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(bytes(d, o, 2)?.try_into().unwrap()))
}

fn u32_at(d: &[u8], o: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes(d, o, 4)?.try_into().unwrap()))
}

pub struct ScreensaverData {
    pub archive: Archive,
    pub exe: ExeData,
}

/// Largest data file read; the installed ones are a little over 1 MiB.
const MAX_FILE_BYTES: u64 = 64 << 20;

/// Reads a file from `dir`, matching the name case-insensitively (installs are often upper case).
fn read_ci(dir: &Path, name: &str) -> Result<Vec<u8>> {
    let entries =
        fs::read_dir(dir).map_err(|e| AssetError::Missing(format!("screensaver directory {} ({e})", dir.display())))?;
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().eq_ignore_ascii_case(name) {
            let meta = fs::metadata(entry.path())?;
            if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
                return Err(AssetError::Format(format!(
                    "{name} is not a regular file of a plausible size"
                )));
            }
            return Ok(fs::read(entry.path())?);
        }
    }
    Err(AssetError::Missing(format!("{} in {}", name, dir.display())))
}

/// Whether `dir` holds a file called `name`, case-insensitively.
fn has_ci(dir: &Path, name: &str) -> bool {
    fs::read_dir(dir).is_ok_and(|mut entries| {
        entries.any(|e| e.is_ok_and(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name)))
    })
}

/// The install: the first of `dirs` that has `SCRANTIC.SCR`. `SCRANTIC.INI` lives in the same folder.
pub fn locate(dirs: &[PathBuf]) -> Result<&Path> {
    dirs.iter()
        .find(|dir| has_ci(dir, "SCRANTIC.SCR"))
        .map(PathBuf::as_path)
        .ok_or_else(|| {
            let places: Vec<_> = dirs.iter().map(|d| d.display().to_string()).collect();
            AssetError::Missing(format!("SCRANTIC.SCR in {}", places.join(" or ")))
        })
}

impl ScreensaverData {
    /// Loads from the install found by [`locate`]: if its files are incomplete or invalid that is an error,
    /// rather than a silent fall back to another folder.
    pub fn load_first(dirs: &[PathBuf]) -> Result<Self> {
        Self::load(locate(dirs)?)
    }

    pub fn load(dir: &Path) -> Result<Self> {
        let exe = read_ci(dir, "SCRANTIC.SCR")?;
        let map = read_ci(dir, "RESOURCE.MAP")?;
        let volume = read_ci(dir, "RESOURCE.001")?;
        if volume.len() < 1024 {
            return Err(AssetError::Format(
                "RESOURCE.001 is the installer placeholder; install the screensaver first".into(),
            ));
        }
        Ok(Self {
            archive: Archive::new(&map, &volume)?,
            exe: ExeData::parse(&exe)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set OPEN_SCRANTIC_DATA to an install directory to run data-dependent tests.
    fn data_dir() -> Option<std::path::PathBuf> {
        std::env::var_os("OPEN_SCRANTIC_DATA").map(Into::into)
    }

    #[test]
    fn loads_installed_game() {
        let Some(dir) = data_dir() else { return };
        let data = ScreensaverData::load(&dir).expect("load");
        assert_eq!(data.exe.scenes.len(), 80);
        assert_eq!(data.exe.sounds.len(), 23);
        assert_eq!(data.exe.paths.len(), 35);
        assert_eq!(data.archive.bmp("JOHNWALK.BMP").unwrap().sprites.len(), 36);
        assert_eq!(data.archive.ttm("FISHWALK.TTM").unwrap().scenes, vec![(19, 1)]);
        let ocean = data.archive.scr("OCEAN00.SCR").unwrap();
        assert_eq!((ocean.w, ocean.h), (640, 480));
    }

    #[test]
    fn load_first_reports_every_folder_it_searched() {
        let base = std::env::temp_dir().join(format!("open-scrantic-test-{}", std::process::id()));
        let (a, b) = (base.join("a"), base.join("b"));
        fs::create_dir_all(&a).unwrap();
        let err = ScreensaverData::load_first(&[a.clone(), b.clone()])
            .err()
            .expect("no files anywhere");
        let msg = err.to_string();
        assert!(
            msg.contains("SCRANTIC.SCR") && msg.contains(&a.display().to_string()),
            "{msg}"
        );
        assert!(msg.contains(&b.display().to_string()), "{msg}");
        // A folder with SCRANTIC.SCR is the install, even when it is incomplete: no fall back to the next one.
        fs::write(a.join("scrantic.scr"), b"x").unwrap();
        let err = ScreensaverData::load_first(&[a, b]).err().expect("incomplete install");
        assert!(err.to_string().contains("RESOURCE.MAP"), "{err}");
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn the_install_is_the_first_folder_with_the_executable() {
        let base = std::env::temp_dir().join(format!("open-scrantic-locate-{}", std::process::id()));
        let (a, b) = (base.join("a"), base.join("b"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let dirs = [a.clone(), b.clone()];
        assert!(locate(&dirs).is_err());
        fs::write(b.join("SCRANTIC.SCR"), b"x").unwrap();
        assert_eq!(locate(&dirs).unwrap(), b);
        fs::write(a.join("Scrantic.scr"), b"x").unwrap();
        assert_eq!(locate(&dirs).unwrap(), a);
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn corrupt_input_is_an_error_not_a_panic() {
        assert!(exe::ExeData::parse(&[]).is_err());
        assert!(exe::ExeData::parse(&[0; 100]).is_err());
        let mut mz = vec![0u8; 0x80];
        mz[..2].copy_from_slice(b"MZ");
        mz[0x3C] = 0xF0;
        assert!(exe::ExeData::parse(&mz).is_err());

        // A map announcing 65535 entries over a tiny volume.
        let mut map = vec![0u8; 21];
        map[19] = 0xFF;
        map[20] = 0xFF;
        assert!(resource::Archive::new(&map, &[0; 32]).is_err());

        assert!(resource::decompress(&[2, 0xFF, 0xFF, 0xFF, 0x7F]).is_err()); // absurd size
        assert!(resource::decompress(&[2, 8, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]).is_err()); // invalid LZW code
        assert!(resource::decompress(&[9, 0, 0, 0, 0]).is_err()); // unknown method
    }

    #[test]
    fn truncated_installs_are_errors_not_panics() {
        let Some(dir) = data_dir() else { return };
        let exe = fs::read(dir.join("SCRANTIC.SCR")).expect("SCRANTIC.SCR");
        let map = fs::read(dir.join("RESOURCE.MAP")).expect("RESOURCE.MAP");
        let volume = fs::read(dir.join("RESOURCE.001")).expect("RESOURCE.001");
        for cut in (0..exe.len()).step_by(997) {
            let _ = exe::ExeData::parse(&exe[..cut]);
        }
        for cut in (0..volume.len()).step_by(4999) {
            let _ = resource::Archive::new(&map, &volume[..cut]);
        }
        for cut in 0..map.len() {
            let _ = resource::Archive::new(&map[..cut], &volume);
        }
    }

    #[test]
    fn error_text_from_data_files_is_escaped_and_bounded() {
        for err in [
            AssetError::Missing("\x1b]0;title\x07\x1b[2J".into()),
            AssetError::Format("\x1b]52;c;AAAA\x07".into()),
        ] {
            let msg = err.to_string();
            assert!(!msg.contains(['\x1b', '\x07']), "raw control byte in {msg:?}");
        }
        let long = AssetError::Format("x".repeat(1 << 20)).to_string();
        assert!(long.len() < 512, "{} bytes", long.len());
    }

    #[test]
    fn a_data_file_that_is_not_a_regular_file_is_refused() {
        let dir = std::env::temp_dir().join(format!("open-scrantic-readci-{}", std::process::id()));
        fs::create_dir_all(dir.join("RESOURCE.MAP")).unwrap(); // a folder where the file should be
        fs::write(dir.join("SCRANTIC.SCR"), b"ok").unwrap();
        assert!(read_ci(&dir, "RESOURCE.MAP").is_err());
        assert_eq!(read_ci(&dir, "scrantic.scr").unwrap(), b"ok");
        let _ = fs::remove_dir_all(dir);
    }
}
