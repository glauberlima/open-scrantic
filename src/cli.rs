//! Command line of the regular (windowed or fullscreen) program. The Windows screensaver build
//! (`--features screensaver`) has none of this: see `saver.rs`.

use open_scrantic::engine::config::Config;
use open_scrantic::{assets, audio};

use crate::Surface;

const USAGE: &str = "\
Usage: open-scrantic [--fullscreen] [--dedither] [--soft-audio] [--background=0|1] [--clouds=0|1]
                     [--waves=0|1] [--sounds=0|1]

Options:
  --fullscreen    start fullscreen instead of in a window
  --dedither      blend the 1992 checkerboard dithering into solid colors
  --soft-audio    improve the sounds: resample to the device rate, hide the 8-bit hiss, fade cuts
  --background=N  override Background of SCRANTIC.INI for this run (0 = off, 1 = on)
  --clouds=N      override Clouds of SCRANTIC.INI for this run
  --waves=N       override Waves of SCRANTIC.INI for this run
  --sounds=N      override Sounds of SCRANTIC.INI for this run
  -V, --version   print the version and exit
  -h, --help      print this help and exit

The original screensaver files (SCRANTIC.SCR, RESOURCE.MAP, RESOURCE.001) and SCRANTIC.INI are read
from the directory of this executable. Esc exits; F12 saves a screenshot there.";

/// Overrides of the INI options for this run; they are never written back to the file.
#[derive(Default)]
struct Overrides {
    background: Option<bool>,
    clouds: Option<bool>,
    waves: Option<bool>,
    sounds: Option<bool>,
}

impl Overrides {
    fn apply(&self, cfg: &mut Config) {
        cfg.background = self.background.unwrap_or(cfg.background);
        cfg.clouds = self.clouds.unwrap_or(cfg.clouds);
        cfg.waves = self.waves.unwrap_or(cfg.waves);
        cfg.sounds = self.sounds.unwrap_or(cfg.sounds);
    }
}

pub struct Options {
    fullscreen: bool,
    dedither: bool,
    soft_audio: bool,
    overrides: Overrides,
}

impl Options {
    /// The settings of this run: the INI's, with the command line applied (the flags only turn the
    /// improvements on).
    pub fn config(&self, saved: &Config) -> Config {
        let mut config = saved.clone();
        self.overrides.apply(&mut config);
        config.dedither |= self.dedither;
        config.soft_audio |= self.soft_audio;
        config
    }

    /// Prints `error: {e}` and exits with status 1.
    pub fn fail(&self, e: impl std::fmt::Display) -> ! {
        eprintln!("error: {e}");
        std::process::exit(1);
    }

    /// Reports that the screensaver files could not be loaded and exits.
    pub fn fail_data(&self, e: assets::AssetError) -> ! {
        self.fail(e)
    }

    /// Where the picture is shown: a window, or the whole screen with `--fullscreen`.
    pub fn surface(&self) -> Surface {
        if self.fullscreen {
            Surface::Fullscreen
        } else {
            Surface::Window
        }
    }

    /// The sound output.
    pub fn audio(&self, run: &Config) -> Option<audio::Audio> {
        audio::Audio::new(run.soft_audio)
    }
}

/// `0` or `1`, as in the INI.
fn parse_flag(name: &str, value: &str) -> Result<bool, String> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(format!("{name} takes 0 or 1, not {value:?}\n\n{USAGE}")),
    }
}

/// Parses the command line; `Ok(None)` means a message was printed and the program should exit.
pub fn parse_args() -> Result<Option<Options>, String> {
    let mut o = Options {
        fullscreen: false,
        dedither: false,
        soft_audio: false,
        overrides: Overrides::default(),
    };
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--fullscreen" => o.fullscreen = true,
            "--dedither" => o.dedither = true,
            "--soft-audio" => o.soft_audio = true,
            "-V" | "--version" => {
                println!("open-scrantic {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                return Ok(None);
            }
            _ => match a.split_once('=') {
                Some(("--background", v)) => o.overrides.background = Some(parse_flag("--background", v)?),
                Some(("--clouds", v)) => o.overrides.clouds = Some(parse_flag("--clouds", v)?),
                Some(("--waves", v)) => o.overrides.waves = Some(parse_flag("--waves", v)?),
                Some(("--sounds", v)) => o.overrides.sounds = Some(parse_flag("--sounds", v)?),
                _ => return Err(format!("unknown argument {a}\n\n{USAGE}")),
            },
        }
    }
    Ok(Some(o))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_take_only_zero_or_one() {
        assert_eq!(parse_flag("--waves", "0"), Ok(false));
        assert_eq!(parse_flag("--waves", "1"), Ok(true));
        assert!(
            parse_flag("--waves", "2")
                .unwrap_err()
                .starts_with("--waves takes 0 or 1")
        );
        assert!(parse_flag("--waves", "").is_err());
    }

    #[test]
    fn overrides_change_only_the_options_they_name() {
        let mut cfg = Config::default();
        assert!(cfg.background && cfg.clouds && cfg.waves && cfg.sounds);
        Overrides {
            waves: Some(false),
            sounds: Some(false),
            ..Overrides::default()
        }
        .apply(&mut cfg);
        assert!(cfg.background && cfg.clouds && !cfg.waves && !cfg.sounds);
    }
}
