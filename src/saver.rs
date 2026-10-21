//! The Windows screensaver build (`--features screensaver`, shipped as a `.scr`): it understands only
//! the switches Windows passes (`/s`, `/c`, `/p`, `/a`) and has no other command line.

use std::time::{Duration, Instant};

use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, WindowEvent};

use open_scrantic::engine::config::Config;
use open_scrantic::windows_ui;
use open_scrantic::{assets, audio};

/// What the program does for the arguments Windows passed.
#[derive(Debug, PartialEq)]
enum Mode {
    /// `/s`: the screensaver proper.
    Run,
    /// `/c`, or no argument at all: the settings dialog, owned by the window handle after the colon, if any.
    Configure(Option<isize>),
    /// `/p HWND`: the preview in the monitor drawing of the Screen Saver Settings window.
    Preview(isize),
    /// `/a` and anything else: nothing to show.
    Quit,
}

pub struct Options {
    /// Fills its window: no centering, no cursor. The preview takes this path too, as it fills its pane.
    pub fullscreen: bool,
    pub dedither: bool,
    pub soft_audio: bool,
    /// The preview pane this run draws into (`/p`); it is silent and neither ends on input nor saves progress.
    pub preview: Option<isize>,
}

impl Options {
    /// The settings of this run: those of the INI.
    pub fn config(&self, saved: &Config) -> Config {
        saved.clone()
    }

    /// Reports a fatal error and exits. Windows gives a screensaver no console, so it goes in a message box;
    /// a preview stays silent, as the box would pop up in the settings window every time it opens.
    pub fn fail(&self, e: impl std::fmt::Display) -> ! {
        if self.preview.is_none() {
            windows_ui::error_box(&e.to_string());
        }
        std::process::exit(1);
    }

    /// Reports that the screensaver files could not be loaded and exits.
    pub fn fail_data(&self, e: assets::AssetError) -> ! {
        self.fail(data_error_message(&e))
    }

    /// The sound output; a preview has none.
    pub fn audio(&self) -> Option<audio::Audio> {
        if self.preview.is_some() {
            None
        } else {
            audio::Audio::new(self.soft_audio)
        }
    }
}

/// The screensaver always runs fullscreen with the image and sound improvements on.
const OPTIONS: Options = Options {
    fullscreen: true,
    dedither: true,
    soft_audio: true,
    preview: None,
};

/// `/c:1234`, `/p 1234` and `/p:1234` carry a window handle; the part after the colon is `inline`,
/// otherwise the handle is the next argument if that is a number.
fn window_handle(inline: Option<&str>, args: &mut std::iter::Peekable<impl Iterator<Item = String>>) -> Option<isize> {
    match inline {
        Some(v) => v.parse().ok(),
        None => args
            .next_if(|a| a.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|a| a.parse().ok()),
    }
}

fn parse(args: impl Iterator<Item = String>) -> Mode {
    let mut args = args.peekable();
    let Some(first) = args.next() else {
        // Windows runs a screensaver without arguments to configure it.
        return Mode::Configure(None);
    };
    let Some(switch) = first.strip_prefix('/') else {
        return Mode::Quit;
    };
    let (name, inline) = match switch.split_once(':') {
        Some((n, v)) => (n, Some(v)),
        None => (switch, None),
    };
    match name.to_ascii_lowercase().as_str() {
        "s" => Mode::Run,
        "c" => Mode::Configure(window_handle(inline, &mut args)),
        "p" => window_handle(inline, &mut args).map_or(Mode::Quit, Mode::Preview),
        _ => Mode::Quit,
    }
}

/// Handles the arguments Windows passed. Returns the options to run the screensaver with, or `None` when
/// there is nothing (more) to do: a dialog was shown, a preview or password request was made, or the
/// screensaver is already running.
pub fn start() -> Option<Options> {
    match parse(std::env::args().skip(1)) {
        Mode::Run if windows_ui::already_running() => None,
        Mode::Run => Some(OPTIONS),
        Mode::Preview(parent) => Some(Options {
            preview: Some(parent),
            soft_audio: false,
            ..OPTIONS
        }),
        Mode::Configure(owner) => {
            configure(owner);
            None
        }
        Mode::Quit => None,
    }
}

/// Text for a failure to load the screensaver files. When they are missing it says where to put them, and
/// creates that folder so that it only takes pasting them in.
fn data_error_message(e: &assets::AssetError) -> String {
    match (e, open_scrantic::user_dir()) {
        (assets::AssetError::Missing(_), Some(dir)) => {
            let _ = std::fs::create_dir_all(&dir);
            format!(
                "{e}

Copy SCRANTIC.SCR, RESOURCE.MAP and RESOURCE.001 of the original screensaver to:
{}",
                dir.display()
            )
        }
        _ => e.to_string(),
    }
}

/// `/c`: lets the user toggle the four INI options. The INI lives with the original files, so without them
/// there is nothing to configure yet.
fn configure(owner: Option<isize>) {
    if let Err(e) = assets::locate(&open_scrantic::data_dirs()) {
        windows_ui::error_box(&data_error_message(&e));
        return;
    }
    let mut cfg = Config::load();
    let current = [cfg.background, cfg.clouds, cfg.waves, cfg.sounds];
    if let Some(new) = windows_ui::settings_dialog(current, owner) {
        [cfg.background, cfg.clouds, cfg.waves, cfg.sounds] = new;
        if let Err(e) = cfg.save() {
            windows_ui::error_box(&format!("Cannot save {}: {e}", Config::path().display()));
        }
    }
}

/// Cursor travel, in pixels, tolerated before the screensaver counts it as input.
const MOUSE_SLACK: f64 = 4.0;

/// Time after the first window event during which input is ignored: Windows sends key, mouse and focus
/// events of its own when it creates the window (and the release of the click that launched the preview).
const SETTLE: Duration = Duration::from_millis(1000);

/// Tells when the user has touched the computer: any key or button pressed, the wheel, losing the focus, or
/// the cursor moving away from where it was first seen.
#[derive(Default)]
pub struct InputGuard {
    first_event: Option<Instant>,
    focused: bool,
    cursor_origin: Option<PhysicalPosition<f64>>,
}

impl InputGuard {
    pub fn ends_run(&mut self, event: &WindowEvent) -> bool {
        let settled = self.first_event.get_or_insert_with(Instant::now).elapsed() >= SETTLE;
        match event {
            // The origin is learnt while settling, so only travel after that counts.
            WindowEvent::CursorMoved { position, .. } => self.moved_away(*position) && settled,
            WindowEvent::Focused(true) => {
                self.focused = true;
                false
            }
            // Losing a focus the window never had (it opened behind another one) is not the user's doing.
            WindowEvent::Focused(false) => settled && std::mem::take(&mut self.focused),
            WindowEvent::KeyboardInput { event, .. } => settled && event.state == ElementState::Pressed,
            WindowEvent::MouseInput { state, .. } => settled && *state == ElementState::Pressed,
            WindowEvent::MouseWheel { .. } => settled,
            _ => false,
        }
    }

    fn moved_away(&mut self, pos: PhysicalPosition<f64>) -> bool {
        let o = *self.cursor_origin.get_or_insert(pos);
        (pos.x - o.x).abs() > MOUSE_SLACK || (pos.y - o.y).abs() > MOUSE_SLACK
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(list: &[&str]) -> Mode {
        parse(list.iter().map(|s| (*s).to_string()))
    }

    #[test]
    fn windows_switches_select_the_mode_in_any_case() {
        assert_eq!(mode(&["/s"]), Mode::Run);
        assert_eq!(mode(&["/S"]), Mode::Run);
        assert_eq!(mode(&["/c"]), Mode::Configure(None));
        assert_eq!(mode(&["/c:1234"]), Mode::Configure(Some(1234)));
        assert_eq!(mode(&["/C", "5678"]), Mode::Configure(Some(5678)));
        assert_eq!(mode(&["/p", "5678"]), Mode::Preview(5678));
        assert_eq!(mode(&["/P:5678"]), Mode::Preview(5678));
        assert_eq!(mode(&["/p"]), Mode::Quit);
        assert_eq!(mode(&["/a"]), Mode::Quit);
        assert_eq!(mode(&[]), Mode::Configure(None));
    }

    #[test]
    fn anything_else_is_ignored() {
        for other in ["--fullscreen", "--help", "-s", "s", "/x", ""] {
            assert_eq!(mode(&[other]), Mode::Quit, "{other}");
        }
    }

    #[test]
    fn small_cursor_jitter_is_not_input() {
        let mut guard = InputGuard::default();
        assert!(!guard.moved_away(PhysicalPosition::new(100.0, 100.0)));
        assert!(!guard.moved_away(PhysicalPosition::new(103.0, 98.0)));
        assert!(guard.moved_away(PhysicalPosition::new(110.0, 100.0)));
    }
}
