//! Persistent settings and story progress, stored like the original SCRANTIC.INI
//! (section [ScreenSaver.ScreenAntics], same keys and defaults).

use std::fs;
use std::path::PathBuf;

const SECTION: &str = "[ScreenSaver.ScreenAntics]";

#[derive(Clone, Debug)]
pub struct Config {
    pub background: bool,
    pub clouds: bool,
    pub waves: bool,
    pub sounds: bool,
    /// Not in the original: blend the checkerboard dithering ("Dedither").
    pub dedither: bool,
    /// Not in the original: improved sound playback ("SoftAudio").
    pub soft_audio: bool,
    /// Furthest story chapter unlocked ("NumDays").
    pub num_days: i32,
    /// Next story chapter to show ("Introduction").
    pub intro: i32,
    pub intro_saved: i32,
    pub year: i32,
    pub month: i32,
    pub day: i32,
    /// Start of the day/night cycle, as HHMM ("StartTime").
    pub start_time: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            background: true,
            clouds: true,
            waves: true,
            sounds: true,
            dedither: false,
            soft_audio: false,
            num_days: 0,
            intro: 0,
            intro_saved: 0,
            year: 0,
            month: 0,
            day: 0,
            start_time: 900,
        }
    }
}

/// The INI is a dozen short lines; anything bigger is not one and is replaced like any other invalid file.
const MAX_INI_BYTES: u64 = 4096;

fn read_small(path: &std::path::Path) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_INI_BYTES {
        return None;
    }
    fs::read_to_string(path).ok()
}

impl Config {
    /// `SCRANTIC.INI` lives in the folder the original files are read from ([`crate::settings_dir`]); the
    /// location cannot be customized.
    pub fn path() -> PathBuf {
        crate::settings_dir().join("SCRANTIC.INI")
    }

    /// Loads the INI. A missing file, or one with any invalid value, is replaced by a new file
    /// holding the defaults (story progress included), and the run continues with those.
    pub fn load() -> Self {
        let path = Self::path();
        let parsed = read_small(&path).and_then(|text| Self::parse(&text));
        parsed.unwrap_or_else(|| {
            let cfg = Self::default();
            if let Err(e) = cfg.save() {
                eprintln!("warning: cannot write {}: {e}", path.display());
            }
            cfg
        })
    }

    /// Strict parser: `None` if any known key in the section has a value that is not an integer
    /// in its valid range, or if a line in the section is not `key=value`. Unknown keys and
    /// keys of other sections are ignored; missing keys keep their defaults.
    fn parse(text: &str) -> Option<Self> {
        let mut cfg = Self::default();
        let mut in_section = false;
        // Editors and `Set-Content -Encoding UTF8` may prepend a BOM, which would hide the section header.
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            if line.starts_with('[') {
                in_section = line.eq_ignore_ascii_case(SECTION);
                continue;
            }
            if !in_section {
                continue;
            }
            let (key, value) = line.split_once('=')?;
            let v = value.trim().parse::<i32>().ok();
            let flag = |v: Option<i32>| match v? {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            };
            let range = |v: Option<i32>, hi: i32| v.filter(|n| (0..=hi).contains(n));
            match key.trim().to_ascii_lowercase().as_str() {
                "background" => cfg.background = flag(v)?,
                "clouds" => cfg.clouds = flag(v)?,
                "waves" => cfg.waves = flag(v)?,
                "sounds" => cfg.sounds = flag(v)?,
                "dedither" => cfg.dedither = flag(v)?,
                "softaudio" => cfg.soft_audio = flag(v)?,
                "numdays" => cfg.num_days = range(v, 11)?,
                "introduction" => cfg.intro = range(v, 12)?,
                "currentyear" => cfg.year = range(v, 9999)?,
                "currentmonth" => cfg.month = range(v, 12)?,
                "currentday" => cfg.day = range(v, 31)?,
                "starttime" => cfg.start_time = range(v, 2359).filter(|n| n % 100 < 60)?,
                _ => {}
            }
        }
        Some(cfg)
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        let intro = if self.intro_saved != 0 {
            self.intro_saved
        } else {
            self.intro
        };
        let b = |v: bool| i32::from(v);
        let text = format!(
            "{SECTION}\nBackground={}\nClouds={}\nWaves={}\nSounds={}\nDedither={}\nSoftAudio={}\nNumDays={}\n\
             Introduction={}\nCurrentYear={}\nCurrentMonth={}\nCurrentDay={}\nStartTime={}\n",
            b(self.background),
            b(self.clouds),
            b(self.waves),
            b(self.sounds),
            b(self.dedither),
            b(self.soft_audio),
            self.num_days,
            intro,
            self.year,
            self.month,
            self.day,
            self.start_time
        );
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        // Write next to the INI and rename over it, so a crash never leaves a truncated file.
        let tmp = path.with_extension("INI.tmp");
        fs::write(&tmp, text)?;
        fs::rename(&tmp, path).inspect_err(|_| {
            let _ = fs::remove_file(&tmp);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip() {
        let text = format!("{SECTION}\nBackground=1\nClouds=0\nStartTime=2359\nNumDays=11\nIntroduction=12\n");
        let cfg = Config::parse(&text).unwrap();
        assert!(cfg.background && !cfg.clouds);
        assert_eq!((cfg.start_time, cfg.num_days, cfg.intro), (2359, 11, 12));
    }

    #[test]
    fn improvements_are_off_by_default_and_round_trip() {
        let cfg = Config::default();
        assert!(!cfg.dedither && !cfg.soft_audio);
        let cfg = Config::parse(&format!("{SECTION}\nDedither=1\nSoftAudio=1\n")).unwrap();
        assert!(cfg.dedither && cfg.soft_audio);
    }

    #[test]
    fn invalid_values_are_rejected() {
        for bad in [
            "Sounds=2",
            "Sounds=abc",
            "Sounds=",
            "Dedither=2",
            "SoftAudio=x",
            "NumDays=12",
            "Introduction=-1",
            "StartTime=960",
            "StartTime=2400",
            "CurrentMonth=13",
            "garbage",
        ] {
            assert!(Config::parse(&format!("{SECTION}\n{bad}\n")).is_none(), "{bad}");
        }
    }

    #[test]
    fn a_utf8_bom_does_not_hide_the_section() {
        let cfg = Config::parse(&format!("\u{feff}{SECTION}\nSounds=0\nNumDays=5\n")).unwrap();
        assert!(!cfg.sounds);
        assert_eq!(cfg.num_days, 5);
    }

    #[test]
    fn unknown_keys_and_other_sections_are_ignored() {
        let cfg = Config::parse(&format!("[Other]\nSounds=9\n{SECTION}\nFoo=bar\nSounds=0\n")).unwrap();
        assert!(!cfg.sounds);
    }

    #[test]
    fn an_oversized_or_non_regular_ini_is_not_read() {
        let dir = std::env::temp_dir().join(format!("open-scrantic-ini-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let small = dir.join("small.ini");
        fs::write(
            &small, "[x]
",
        )
        .unwrap();
        assert_eq!(
            read_small(&small).as_deref(),
            Some(
                "[x]
"
            )
        );
        let big = dir.join("big.ini");
        fs::write(&big, vec![b'a'; MAX_INI_BYTES as usize + 1]).unwrap();
        assert!(read_small(&big).is_none());
        assert!(read_small(&dir).is_none()); // a folder
        let _ = fs::remove_dir_all(dir);
    }
}
