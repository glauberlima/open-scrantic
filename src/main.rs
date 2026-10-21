//! open-scrantic: a modern reimplementation of Screen Antics: Johnny Castaway (Sierra/Dynamix, 1992).
//! Runs on the original, installed screensaver files (SCRANTIC.SCR, RESOURCE.MAP, RESOURCE.001).

// The screensaver build is a GUI program: Windows must not open a console window behind it.
#![cfg_attr(all(windows, feature = "screensaver"), windows_subsystem = "windows")]
#![cfg_attr(not(feature = "screensaver"), forbid(unsafe_code))]

#[cfg(not(feature = "screensaver"))]
mod cli;
#[cfg(feature = "screensaver")]
mod saver;

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Fullscreen, Window, WindowAttributes, WindowId};

use open_scrantic::engine::config::Config;
use open_scrantic::engine::gfx::{HEIGHT, WIDTH};
use open_scrantic::engine::{Clock, Engine, STEP_MS, TICK_MS};
use open_scrantic::present;
#[cfg(feature = "screensaver")]
use open_scrantic::windows_ui;
use open_scrantic::{assets, audio};

/// The title screen stays up while the 1992 machine loaded the first session.
const INTRO_HOLD: Duration = Duration::from_secs(2);

struct App {
    engine: Engine,
    audio: Option<audio::Audio>,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    start: Instant,
    steps: u64,
    ticks: u64,
    fullscreen: bool,
    /// Ends the run at the first sign of the user (screensaver build only; a preview has none).
    #[cfg(feature = "screensaver")]
    input: Option<saver::InputGuard>,
    /// The preview pane of the Screen Saver Settings window this run draws into, if it is the preview.
    #[cfg(feature = "screensaver")]
    preview: Option<isize>,
    dedither: bool,
    /// Settings as they are in the INI, which a run with overrides must write back unchanged.
    saved: Config,
    /// F12 was pressed; the next redraw saves the frame it draws.
    screenshot: bool,
    /// The title screen has been requested once; until the first step it needs no further redraws.
    intro_drawn: bool,
}

impl App {
    /// Advances the virtual Windows 3.1 clocks: a story step every 55 ms and the 16 ms tick.
    fn advance(&mut self) -> bool {
        let elapsed = self.start.elapsed().as_millis() as u64;
        let due = elapsed / STEP_MS;
        let mut stepped = false;
        // After long stalls (sleep, debugger) resume instead of fast-forwarding.
        if due > self.steps + 20 {
            self.steps = due - 1;
            self.ticks = self.steps * STEP_MS / TICK_MS;
        }
        while self.steps < due {
            self.steps += 1;
            let virtual_ms = self.steps * STEP_MS;
            while (self.ticks + 1) * TICK_MS <= virtual_ms {
                self.ticks += 1;
                self.engine.tick();
            }
            self.engine.step(&Clock::now());
            for id in self.engine.take_sounds() {
                if let Some(a) = self.audio.as_mut().filter(|_| self.engine.config.sounds) {
                    a.play(self.engine.sound_bank(), id);
                }
            }
            stepped = true;
        }
        stepped
    }

    /// Writes the current frame next to the executable: `-raw` is the 640x480 palette picture as the
    /// engine drew it, `-view` is what the window shows (dedithering included) before scaling.
    fn save_screenshot(&self) {
        let c = Clock::now();
        let name = format!(
            "screenshot-{:04}{:02}{:02}-{:02}{:02}-{}",
            c.year, c.month, c.day, c.hour, c.minute, self.steps
        );
        let px = &self.engine.frame().px;
        let pal = &self.engine.palette;
        let view = if self.dedither {
            present::dedither(px, pal)
        } else {
            present::plain(px, pal)
        };
        for (suffix, frame) in [("raw", present::plain(px, pal)), ("view", view)] {
            let path = open_scrantic::app_dir().join(format!("{name}-{suffix}.png"));
            let png = present::png(WIDTH as usize, HEIGHT as usize, &frame);
            match std::fs::write(&path, png) {
                Ok(()) => eprintln!("saved {}", path.display()),
                Err(e) => eprintln!("warning: cannot save {}: {e}", path.display()),
            }
        }
    }

    fn redraw(&mut self) {
        if std::mem::take(&mut self.screenshot) {
            self.save_screenshot();
        }
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            return;
        };
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else {
            return;
        };
        if surface.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buf) = surface.buffer_mut() else { return };
        let (w, h) = (size.width as usize, size.height as usize);
        let px = &self.engine.frame().px;
        let rgb = if self.dedither {
            present::dedither(px, &self.engine.palette)
        } else {
            present::plain(px, &self.engine.palette)
        };
        present::scale(&rgb, &mut buf, w, h);
        let _ = buf.present();
    }
}

/// Moves the window so its whole frame (title bar included) is centred on the primary monitor.
fn center_on_primary_monitor(window: &Window, event_loop: &ActiveEventLoop) {
    let Some(monitor) = event_loop
        .primary_monitor()
        .or_else(|| event_loop.available_monitors().next())
    else {
        return;
    };
    let (m, origin) = (monitor.size(), monitor.position());
    let w = window.outer_size();
    let x = origin.x + (m.width as i32 - w.width as i32) / 2;
    let y = origin.y + (m.height as i32 - w.height as i32) / 2;
    window.set_outer_position(PhysicalPosition::new(x, y));
}

impl App {
    fn window_attributes(&self, event_loop: &ActiveEventLoop) -> WindowAttributes {
        let mut attrs = Window::default_attributes()
            .with_title("Open Scrantic")
            .with_min_inner_size(PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
        if self.fullscreen {
            attrs = attrs.with_fullscreen(Some(Fullscreen::Borderless(None)));
        } else {
            // Largest whole multiple of the original 640x480 that fills at most 80% of the screen.
            let k = event_loop
                .primary_monitor()
                .or_else(|| event_loop.available_monitors().next())
                .map_or(1, |m| {
                    let s = m.size();
                    (s.width * 4 / 5 / WIDTH as u32)
                        .min(s.height * 4 / 5 / HEIGHT as u32)
                        .max(1)
                });
            attrs = attrs.with_inner_size(PhysicalSize::new(WIDTH as u32 * k, HEIGHT as u32 * k));
        }
        attrs
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        #[cfg(feature = "screensaver")]
        let attrs = match self.preview {
            Some(parent) => windows_ui::preview_attributes(parent),
            None => self.window_attributes(event_loop),
        };
        #[cfg(not(feature = "screensaver"))]
        let attrs = self.window_attributes(event_loop);
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                eprintln!("error: cannot create window: {e}");
                event_loop.exit();
                return;
            }
        };
        if !self.fullscreen {
            center_on_primary_monitor(&window, event_loop);
        }
        window.set_cursor_visible(!self.fullscreen);
        let surface = softbuffer::Context::new(Rc::clone(&window))
            .and_then(|ctx| softbuffer::Surface::new(&ctx, Rc::clone(&window)));
        match surface {
            Ok(s) => self.surface = Some(s),
            Err(e) => {
                eprintln!("error: cannot create drawing surface: {e}");
                event_loop.exit();
                return;
            }
        }
        self.window = Some(window);
        self.start = Instant::now() + INTRO_HOLD;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        #[cfg(feature = "screensaver")]
        if self.input.as_mut().is_some_and(|input| input.ends_run(&event)) {
            event_loop.exit();
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && event.logical_key == Key::Named(NamedKey::Escape) =>
            {
                event_loop.exit();
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && event.logical_key == Key::Named(NamedKey::F12) =>
            {
                self.screenshot = true;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            return;
        }
        // The preview ends with the settings window it draws into.
        #[cfg(feature = "screensaver")]
        if self.preview.is_some_and(|parent| !windows_ui::window_exists(parent)) {
            event_loop.exit();
            return;
        }
        let stepped = self.advance();
        if (stepped || !self.intro_drawn)
            && let Some(w) = &self.window
        {
            self.intro_drawn = true;
            w.request_redraw();
        }
        let next = self.start + Duration::from_millis((self.steps + 1) * STEP_MS);
        event_loop.set_control_flow(ControlFlow::WaitUntil(next));
    }

    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        // A preview must not move the story on: only real runs save their progress.
        #[cfg(feature = "screensaver")]
        if self.preview.is_some() {
            return;
        }
        let mut cfg = self.engine.config.clone();
        (cfg.background, cfg.clouds, cfg.waves, cfg.sounds) = (
            self.saved.background,
            self.saved.clouds,
            self.saved.waves,
            self.saved.sounds,
        );
        if let Err(e) = cfg.save() {
            eprintln!("warning: cannot save settings: {e}");
        }
    }
}

fn main() {
    #[cfg(not(feature = "screensaver"))]
    let opts = match cli::parse_args() {
        Ok(Some(o)) => o,
        Ok(None) => return,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };
    #[cfg(feature = "screensaver")]
    let Some(opts) = saver::start() else { return };
    let data = assets::ScreensaverData::load_first(&open_scrantic::data_dirs()).unwrap_or_else(|e| opts.fail_data(e));
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let saved = Config::load();
    let engine = Engine::new(data, opts.config(&saved), seed).unwrap_or_else(|e| opts.fail(e));
    let audio = opts.audio();
    let event_loop = EventLoop::new().unwrap_or_else(|e| opts.fail(e));
    let mut app = App {
        engine,
        audio,
        window: None,
        surface: None,
        start: Instant::now(),
        steps: 0,
        ticks: 0,
        fullscreen: opts.fullscreen,
        #[cfg(feature = "screensaver")]
        input: opts.preview.is_none().then(saver::InputGuard::default),
        #[cfg(feature = "screensaver")]
        preview: opts.preview,
        dedither: opts.dedither,
        saved,
        screenshot: false,
        intro_drawn: false,
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("error: {e}");
    }
}
