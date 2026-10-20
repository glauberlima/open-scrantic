//! Headless run: steps the engine with a fixed wall clock and saves PNG snapshots.
//! Usage: cargo run --example snapshot -- INSTALL_DIR OUT_DIR [STEPS] [EVERY] [HHMM] [SEED] [STORY_DAY] [SCENE|list]
//! Environment: DEDITHER=1 blends checkerboard dithering; SIZE=WxH scales like the window;
//! DATE=MMDD sets the calendar day (default 0601), which selects the holiday decoration.

use std::fs;
use std::path::Path;

use open_scrantic::assets::ScreensaverData;
use open_scrantic::engine::config::Config;
use open_scrantic::engine::gfx::{HEIGHT, WIDTH};
use open_scrantic::engine::{Clock, Engine, STEP_MS, TICK_MS};
use open_scrantic::present;

fn fail(msg: &str) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(2);
}

/// Optional positional argument `i`: `default` when absent, an error when present but invalid.
fn arg<T: std::str::FromStr>(a: &[String], i: usize, name: &str, default: T) -> T {
    match a.get(i) {
        None => default,
        Some(v) => v.parse().unwrap_or_else(|_| fail(&format!("invalid {name}: {v}"))),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = Path::new(a.get(1).unwrap_or_else(|| fail("missing INSTALL_DIR")));
    let out = Path::new(a.get(2).unwrap_or_else(|| fail("missing OUT_DIR")));
    let steps: u64 = arg(&a, 3, "STEPS", 400);
    let every: u64 = arg(&a, 4, "EVERY", 50);
    let hhmm: i32 = arg(&a, 5, "HHMM", 1000);
    let seed: u64 = arg(&a, 6, "SEED", 12345);
    let story_day: i32 = arg(&a, 7, "STORY_DAY", 0);
    if every == 0 {
        fail("EVERY must be at least 1");
    }
    if !(0..2400).contains(&hhmm) || hhmm % 100 >= 60 {
        fail("HHMM must be a time between 0000 and 2359");
    }
    if !(0..=11).contains(&story_day) {
        fail("STORY_DAY must be between 0 and 11");
    }
    fs::create_dir_all(out).unwrap();
    let (month, day) = match std::env::var("DATE") {
        Ok(v) => match v.parse::<i32>() {
            Ok(n) if v.len() == 4 && (1..=12).contains(&(n / 100)) && (1..=31).contains(&(n % 100)) => {
                (n / 100, n % 100)
            }
            _ => fail("DATE must be MMDD, for example 1224"),
        },
        Err(_) => (6, 1),
    };
    let clock = Clock {
        year: 1993,
        month,
        day,
        hour: hhmm / 100,
        minute: hhmm % 100,
    };
    let mut config = Config::default();
    if story_day > 0 {
        (config.num_days, config.intro) = (story_day, story_day);
        (config.year, config.month, config.day) = (clock.year, clock.month, clock.day);
    }
    let mut engine = Engine::new(ScreensaverData::load(dir).unwrap(), config, seed).unwrap();
    match a.get(8).map(String::as_str) {
        Some("list") => {
            for i in 0..engine.scene_count() {
                println!("{i}: {}", engine.scene_label(i));
            }
            return;
        }
        Some(v) => {
            let i: usize = v.parse().unwrap_or_else(|_| fail(&format!("invalid SCENE: {v}")));
            if i >= engine.scene_count() {
                fail(&format!("SCENE must be below {}", engine.scene_count()));
            }
            engine.force_scene(i);
        }
        None => {}
    }
    let mut ticks = 0u64;
    for step in 1..=steps {
        while (ticks + 1) * TICK_MS <= step * STEP_MS {
            ticks += 1;
            engine.tick();
        }
        engine.step(&clock);
        let sounds = engine.take_sounds();
        if !sounds.is_empty() {
            println!("step {step}: sounds {sounds:?}");
        }
        if step % every == 0 {
            let px = &engine.frame().px;
            let mut frame = if std::env::var_os("DEDITHER").is_some() {
                present::dedither(px, &engine.palette)
            } else {
                present::plain(px, &engine.palette)
            };
            let (mut w, mut h) = (WIDTH as usize, HEIGHT as usize);
            if let Ok(size) = std::env::var("SIZE") {
                let (sw, sh) = size.split_once('x').unwrap_or_else(|| fail("SIZE must be WxH"));
                (w, h) = (
                    sw.parse().unwrap_or_else(|_| fail("invalid SIZE")),
                    sh.parse().unwrap_or_else(|_| fail("invalid SIZE")),
                );
                let mut scaled = vec![0; w * h];
                present::scale(&frame, &mut scaled, w, h);
                frame = scaled;
            }
            fs::write(out.join(format!("step{step:05}.png")), present::png(w, h, &frame)).unwrap();
        }
    }
}
