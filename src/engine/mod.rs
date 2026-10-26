//! The Screen Antics engine: story planning, scheduling and composition of each frame.
//!
//! Timing reproduces the original: a 16 ms tick counter drives script delays and timers,
//! while the story/scheduler step runs on the Windows 3.1 timer (~55 ms).

pub mod ads;
pub mod config;
pub mod gfx;
pub mod island;
pub mod rng;
pub mod ttm;
pub mod walk;

use std::rc::Rc;

use chrono::{Datelike, Local, Timelike};

use crate::assets::resource::{Archive, Bmp};
use crate::assets::{AssetError, Result, ScreensaverData};
use ads::AdsRuntime;
use config::Config;
use gfx::Canvas;
use island::{Island, Options};
use rng::Rng;
use walk::Walk;

pub const TICK_MS: u64 = 16;
pub const STEP_MS: u64 = 55;
const STORY_CHIME: u32 = 25;
const IDLE_ADS: u8 = 0xFF;
const PLAN_BUDGET: i32 = 300;
/// Percentages of repeating a repeatable scene 1..=6 times (the six weights pushed at seg4:0aac..0ac1).
const REPEAT_WEIGHTS: [u16; 6] = [10, 20, 30, 20, 10, 10];
/// Scene roles in `SceneDef::flags` (also the `pick` masks) and the layout flag read by `step`.
const SCENE_OPENING: u32 = 0x01;
const SCENE_MIDDLE: u32 = 0x02;
const SCENE_FINAL: u32 = 0x04;
const SCENE_REPEATABLE: u32 = 0x08;
const SCENE_STORY: u32 = 0x80;
const SCENE_SHIFTED: u32 = 0x400;
/// Longest plan the original builds (seg4:08b9).
const MAX_PLAN_LEN: usize = 298;
/// Bound on the draws for a final scene; the original has none and can hang (see `plan_session`).
const MAX_FINAL_PICKS: u32 = 1000;

/// Everything a TTM frame may touch.
pub struct Env<'a> {
    pub work: &'a mut Canvas,
    pub bg: &'a mut Canvas,
    pub front: &'a mut Canvas,
    pub archive: &'a Archive,
    pub johnwalk: &'a Rc<Bmp>,
    pub rng: &'a mut Rng,
    pub sounds: &'a mut Vec<u32>,
    pub offset: (i32, i32),
    pub now: u32,
}

/// Local wall-clock time, injectable for tests.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub hour: i32,
    pub minute: i32,
}

impl Clock {
    pub fn now() -> Self {
        let t = Local::now();
        Self {
            year: t.year(),
            month: t.month() as i32,
            day: t.day() as i32,
            hour: t.hour() as i32,
            minute: t.minute() as i32,
        }
    }
}

pub struct Engine {
    data: ScreensaverData,
    pub config: Config,
    rng: Rng,
    island: Island,
    walk: Walk,
    ads: Option<AdsRuntime>,
    johnwalk: Rc<Bmp>,
    pub palette: [[u8; 3]; 256],
    bg: Canvas,
    work: Canvas,
    front: Canvas,
    pub now: u32,
    idle_timer: u32,
    offset: (i32, i32),
    plan: Vec<usize>,
    plan_idx: usize,
    pending: Vec<bool>,
    last_final: Option<usize>,
    last_first: Option<usize>,
    story_scene: Option<usize>,
    forced_scene: Option<usize>,
    story_state: i32,
    new_session: bool,
    debug: bool,
    sounds: Vec<u32>,
    /// Number of plan entries completed so far (diagnostics).
    pub scenes_done: u64,
}

/// seg4:0c48 – half-hour slot of an HHMM time, shifted as the original does.
fn half_hour_slot(t: i32) -> i32 {
    (t / 50 + (t % 100) / 30 + 14) % 16
}

impl Engine {
    pub fn new(data: ScreensaverData, config: Config, seed_time: u64) -> Result<Self> {
        // `plan_session` ends every session with a final scene; without one it has nothing to plan.
        if !data.exe.scenes.iter().any(|s| s.flags & SCENE_FINAL != 0) {
            return Err(AssetError::Format("scene catalog has no final scene".into()));
        }
        let palette = data.archive.palette("JOHNCAST.PAL")?;
        let johnwalk = Rc::new(data.archive.bmp("JOHNWALK.BMP")?);
        let island = Island::new(&data.archive, data.exe.whitecap_seqs.clone())?;
        let mut rng = Rng::new(data.exe.rng_table, data.exe.rng_j, data.exe.rng_k);
        rng.warm_up(seed_time);
        let mut front = Canvas::default();
        front.load_picture(&data.archive.scr("INTRO.SCR")?);
        let pending = vec![true; data.exe.scenes.len()];
        Ok(Self {
            data,
            config,
            rng,
            island,
            walk: Walk::default(),
            ads: None,
            johnwalk,
            palette,
            bg: front.clone(),
            work: front.clone(),
            front,
            now: 0,
            idle_timer: 0,
            offset: (0, 0),
            plan: Vec::new(),
            plan_idx: 0,
            pending,
            last_final: None,
            last_first: None,
            story_scene: None,
            story_state: 0,
            new_session: false,
            debug: std::env::var_os("OPEN_SCRANTIC_DEBUG").is_some(),
            forced_scene: None,
            sounds: Vec::new(),
            scenes_done: 0,
        })
    }

    pub fn frame(&self) -> &Canvas {
        &self.front
    }

    /// Sound effects requested since the last call (WAVESFX numbers).
    pub fn take_sounds(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.sounds)
    }

    pub fn sound_bank(&self) -> &std::collections::HashMap<u32, Vec<f32>> {
        &self.data.exe.sounds
    }

    /// The 16 ms timer callback (seg2:16ce).
    pub fn tick(&mut self) {
        self.now = self.now.wrapping_add(1);
        for t in [
            &mut self.walk.timer,
            &mut self.idle_timer,
            &mut self.island.wave_timer,
            &mut self.island.cap_timer,
        ] {
            *t = t.saturating_sub(1);
        }
    }

    // ---- story (seg4) ----

    /// seg4:0ba5 – advance story chapters with the calendar.
    fn update_day(&mut self, clock: &Clock) {
        let c = &mut self.config;
        if (clock.year, clock.month, clock.day) != (c.year, c.month, c.day) {
            if c.num_days < c.intro {
                c.num_days += 1;
            }
            if c.num_days > 11 || c.num_days < 2 {
                c.intro = 1;
                c.num_days = 1;
            }
            (c.year, c.month, c.day) = (clock.year, clock.month, clock.day);
            c.intro_saved = 0;
        }
        if self.new_session && self.story_state != 0 && c.intro <= c.num_days {
            c.intro += 1;
            c.intro_saved = c.intro;
        }
        self.story_state = 0;
        self.new_session = false;
    }

    /// seg4:0d76 – weighted random scene matching `mask`, the day phase and the time budget.
    fn pick(&mut self, mask: u32, phase: i32, mut budget: i32) -> Option<usize> {
        if mask == SCENE_FINAL {
            budget = 999;
        } else if mask == SCENE_STORY {
            if self.rng.below(10) != 0 {
                return None;
            }
            self.config.intro = self.config.num_days;
            budget = 999;
        }
        let intro = self.config.intro;
        let scenes = &self.data.exe.scenes;
        let marked: Vec<bool> = scenes
            .iter()
            .map(|s| {
                if mask == SCENE_STORY && s.day as i32 == intro {
                    return true;
                }
                s.flags & mask != 0
                    && s.ads_tag != 0
                    && s.min_phase as i32 <= phase
                    && s.day == 0
                    && (s.max_phase as i32) > phase
                    && (s.duration as i32 / 2) < budget
                    && self.island.compatible(s.flags)
            })
            .collect();
        let total: i32 = scenes
            .iter()
            .zip(&marked)
            .filter(|(_, m)| **m)
            .map(|(s, _)| s.weight as i32)
            .sum();
        if total == 0 {
            return None;
        }
        let mut r = self.rng.below(total as u16) as i32 + 1;
        for (i, s) in scenes.iter().enumerate() {
            if marked[i] {
                r -= s.weight as i32;
                if r <= 0 {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Debug aid (not in the original): number of entries in the scene catalog.
    pub fn scene_count(&self) -> usize {
        self.data.exe.scenes.len()
    }

    /// `ADS:tag` of catalog scene `i` (`idle` for the walking scenes).
    fn scene_name(&self, i: usize) -> String {
        let s = &self.data.exe.scenes[i];
        let ads = self.data.exe.ads_names.get(&s.ads_id).map_or("idle", String::as_str);
        format!("{ads}:{}", s.ads_tag)
    }

    /// Debug aid: one-line description of catalog scene `i` (`ADS:tag`, story day, flags, duration).
    pub fn scene_label(&self, i: usize) -> String {
        let s = &self.data.exe.scenes[i];
        format!(
            "{} day {} flags {:#x} phase {}..{} duration {}",
            self.scene_name(i),
            s.day,
            s.flags,
            s.min_phase,
            s.max_phase,
            s.duration
        )
    }

    /// Debug aid: the next planned session is just catalog scene `i`.
    pub fn force_scene(&mut self, i: usize) {
        self.forced_scene = Some(i);
    }

    /// seg4:08b9 – plan a session: optional story chapter, a final scene and filler scenes.
    fn plan_session(&mut self, phase: i32, mut budget: i32) {
        if let Some(s) = self.forced_scene.take() {
            self.island.apply_flags(self.data.exe.scenes[s].flags);
            self.pending[s] = true;
            self.plan = vec![s];
            return;
        }
        let flags = |e: &Self, i: usize| e.data.exe.scenes[i].flags;
        let duration = |e: &Self, i: usize| e.data.exe.scenes[i].duration as i32;
        let mut first = None;
        let mut fin = None;
        if let Some(s) = self.pick(SCENE_STORY, phase, 0)
            && Some(s) != self.last_final
            && Some(s) != self.last_first
        {
            if flags(self, s) & SCENE_FINAL != 0 {
                fin = Some(s)
            } else {
                first = Some(s)
            }
            self.story_scene = Some(s);
        }
        let mut attempts = 0;
        let fin = loop {
            if let Some(f) = fin
                && Some(f) != self.last_final
                && !(first.is_some() && flags(self, f) & SCENE_OPENING != 0)
            {
                break f;
            }
            attempts += 1;
            fin = if attempts < MAX_FINAL_PICKS {
                self.pick(SCENE_FINAL, phase, budget)
            } else {
                None
            };
            if fin.is_none() {
                // Nothing fits the constraints (or only the previous final does): the original would
                // spin forever; accept any final scene.
                fin = (0..self.data.exe.scenes.len()).find(|&i| flags(self, i) & SCENE_FINAL != 0);
                break fin.unwrap_or(0);
            }
        };
        budget -= duration(self, fin);
        self.island.apply_flags(flags(self, fin));
        self.last_final = Some(fin);
        if let Some(f) = first {
            budget -= duration(self, f);
            self.island.apply_flags(flags(self, f));
        }
        self.last_first = first;
        let intro = self.config.intro;
        if self.data.exe.scenes[fin].day as i32 == intro
            || first.is_some_and(|f| self.data.exe.scenes[f].day as i32 == intro)
        {
            self.story_state = 1;
        }

        let mut plan = Vec::new();
        let mut pending_first = first;
        if flags(self, fin) & SCENE_OPENING != 0 {
            budget = 0;
        } else if let Some(f) = pending_first.filter(|&f| flags(self, f) & SCENE_OPENING != 0) {
            if self.rng.chance(50) {
                plan.push(f);
                pending_first = None;
            } else {
                self.plan_opening(&mut plan, phase, &mut budget);
            }
        } else {
            self.plan_opening(&mut plan, phase, &mut budget);
        }
        while budget != 0 && plan.len() < MAX_PLAN_LEN {
            if let Some(f) = pending_first
                && self.rng.chance(5)
            {
                plan.push(f);
                pending_first = None;
                continue;
            }
            let Some(s) = self.pick(SCENE_MIDDLE, phase, budget) else {
                break;
            };
            let mut reps = 1;
            if flags(self, s) & SCENE_REPEATABLE != 0 {
                loop {
                    reps = self.rng.pick(&REPEAT_WEIGHTS) as i32;
                    if reps != 0 && !(reps > 1 && duration(self, s) * reps > budget) {
                        break;
                    }
                }
            }
            while reps > 0 && plan.len() < MAX_PLAN_LEN {
                plan.push(s);
                budget -= duration(self, s);
                self.island.apply_flags(flags(self, s));
                reps -= 1;
            }
        }
        plan.push(fin);
        for &i in &plan {
            self.pending[i] = true;
        }
        self.plan = plan;
    }

    fn plan_opening(&mut self, plan: &mut Vec<usize>, phase: i32, budget: &mut i32) {
        if let Some(s) = self.pick(SCENE_OPENING, phase, *budget) {
            plan.push(s);
            *budget -= self.data.exe.scenes[s].duration as i32;
            self.island.apply_flags(self.data.exe.scenes[s].flags);
        }
    }

    /// seg4:0540 – start a new session: plan scenes and build the island.
    fn new_session(&mut self, clock: &Clock) {
        self.ads = None;
        let t = clock.hour * 100 + if clock.minute >= 30 { 30 } else { 0 };
        let phase = (half_hour_slot(t) - half_hour_slot(self.config.start_time) + 16) % 16;
        self.island.phase = phase;
        self.island.holiday = match (clock.month, clock.day) {
            (10, 29..) => 1,
            (12, 23..=25) => 2,
            (12, 29..) | (1, 1) => 3,
            (3, 15..=17) => 4,
            _ => 0,
        };
        self.island.reset_config();
        self.update_day(clock);
        self.plan_session(phase, PLAN_BUDGET);
        let first = &self.data.exe.scenes[self.plan[0]];
        self.walk.spot = first.spot_start as i32;
        self.walk.hdg = first.hdg_start as i32;
        self.plan_idx = 0;
        let opts = Options {
            background: self.config.background,
            clouds: self.config.clouds,
            waves: self.config.waves,
        };
        self.island.init(
            &self.data.archive,
            &mut self.bg,
            &mut self.rng,
            &opts,
            self.config.num_days,
        );
        self.offset = self.island.ttm_offset(false);
        if self.debug {
            let names: Vec<String> = self.plan.iter().map(|&i| self.scene_name(i)).collect();
            eprintln!(
                "session: phase {}, day {}/{}, island cfg {:#x} tide {} -> {}",
                phase,
                self.config.intro,
                self.config.num_days,
                self.island.cfg,
                self.island.tide,
                names.join(" ")
            );
        }
    }

    fn load_ads(&mut self, id: u8) {
        if self.ads.as_ref().is_some_and(|a| a.id == id) {
            return;
        }
        self.ads = None;
        let Some(name) = self.data.exe.ads_names.get(&id) else {
            return;
        };
        match AdsRuntime::load(&self.data.archive, id, name) {
            Ok(a) => self.ads = Some(a),
            Err(e) => eprintln!("warning: {e}"),
        }
    }

    fn walk_step(&mut self) {
        self.walk.step(
            &self.data.exe,
            &mut self.rng,
            &self.johnwalk,
            &self.island,
            &mut self.work,
        );
    }

    /// Ends plan entry `si`: Johnny is left at the scene's end spot and the plan moves on.
    fn complete_scene(&mut self, si: usize) {
        let def = &self.data.exe.scenes[si];
        self.walk.spot = def.spot_end as i32;
        self.walk.hdg = def.hdg_end as i32;
        self.pending[si] = true;
        self.plan_idx += 1;
        self.scenes_done += 1;
    }

    /// One story/scheduler step (seg4:06bf), composing a new frame.
    pub fn step(&mut self, clock: &Clock) {
        if self.plan_idx >= self.plan.len() {
            self.new_session = true;
            self.new_session(clock);
        }
        self.work.px.copy_from_slice(&self.bg.px);
        let si = self.plan[self.plan_idx];
        self.island.step(&mut self.work, &mut self.rng);
        let def = self.data.exe.scenes[si];
        let arrived = !self.pending[si] || self.walk.go_to(def.spot_start as i32, def.hdg_start as i32);
        if !arrived {
            self.walk_step();
        } else if def.ads_id == IDLE_ADS {
            if self.pending[si] {
                self.pending[si] = false;
                self.idle_timer = self.rng.below(240) as u32 + 180;
                self.walk.reset();
            }
            self.walk_step();
            if self.idle_timer == 0 {
                self.complete_scene(si);
            }
        } else {
            if self.pending[si] {
                self.load_ads(def.ads_id);
                if let Some(a) = self.ads.as_mut() {
                    a.start_tag(def.ads_tag);
                }
                self.pending[si] = false;
                self.offset = self.island.ttm_offset(def.flags & SCENE_SHIFTED != 0);
            }
            let busy = match self.ads.as_mut() {
                Some(ads) => {
                    let mut env = Env {
                        work: &mut self.work,
                        bg: &mut self.bg,
                        front: &mut self.front,
                        archive: &self.data.archive,
                        johnwalk: &self.johnwalk,
                        rng: &mut self.rng,
                        sounds: &mut self.sounds,
                        offset: self.offset,
                        now: self.now,
                    };
                    ads.update(&mut env);
                    ads.tag_busy(def.ads_tag)
                }
                None => false,
            };
            if !busy {
                if self.debug {
                    eprintln!(
                        "tick {}: finished plan entry {} ({}:{})",
                        self.now, self.plan_idx, def.ads_id, def.ads_tag
                    );
                }
                self.complete_scene(si);
                if self.walk.spot == 0 {
                    self.walk.spot = self.rng.below(6) as i32 + 1;
                }
                if self.walk.hdg == 0 {
                    self.walk.hdg = (self.rng.next_u16() & 7) as i32;
                }
                if self.story_state == 2 {
                    self.config.intro += 1;
                    self.config.intro_saved = self.config.intro;
                    self.story_state = 3;
                }
            }
        }
        self.island.draw_holiday(&mut self.work);
        if self.story_scene == Some(si) {
            self.sounds.push(STORY_CHIME);
            self.story_scene = None;
            self.story_state = 2;
        }
        std::mem::swap(&mut self.front, &mut self.work);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screensaver_data() -> Option<ScreensaverData> {
        let dir = std::env::var_os("OPEN_SCRANTIC_DATA")?;
        Some(ScreensaverData::load(std::path::Path::new(&dir)).expect("screensaver data"))
    }

    #[test]
    fn every_ads_script_loads() {
        let Some(data) = screensaver_data() else { return };
        for (&id, name) in &data.exe.ads_names {
            AdsRuntime::load(&data.archive, id, name).unwrap_or_else(|e| panic!("{name}: {e}"));
        }
    }

    #[test]
    fn repeat_weights_cover_the_whole_draw() {
        assert_eq!(REPEAT_WEIGHTS.iter().sum::<u16>(), 100);
    }

    #[test]
    fn headless_run_completes_scenes() {
        let Some(data) = screensaver_data() else { return };
        let mut engine = Engine::new(data, Config::default(), 4242).unwrap();
        let clock = Clock {
            year: 1993,
            month: 6,
            day: 1,
            hour: 10,
            minute: 0,
        };
        let mut ticks = 0u64;
        for step in 1..=6000u64 {
            while (ticks + 1) * TICK_MS <= step * STEP_MS {
                ticks += 1;
                engine.tick();
            }
            engine.step(&clock);
        }
        assert!(engine.scenes_done >= 3, "only {} scenes completed", engine.scenes_done);
    }

    #[test]
    fn day_phase_matches_original_formula() {
        assert_eq!(half_hour_slot(900), (18 + 14) % 16);
        assert_eq!(half_hour_slot(1430), 11);
        let phase = |t| (half_hour_slot(t) - half_hour_slot(900) + 16) % 16;
        assert_eq!(phase(900), 0);
        assert_eq!(phase(1430), 11); // last daytime slot
        assert_eq!(phase(1500), 12); // night until 17:00
        assert_eq!(phase(1700), 0);
    }

    fn run_steps(engine: &mut Engine, steps: u64) {
        let clock = Clock {
            year: 1993,
            month: 6,
            day: 1,
            hour: 10,
            minute: 0,
        };
        let mut ticks = 0u64;
        for step in 1..=steps {
            while (ticks + 1) * TICK_MS <= step * STEP_MS {
                ticks += 1;
                engine.tick();
            }
            engine.step(&clock);
        }
    }

    #[test]
    fn an_empty_scene_catalog_is_rejected() {
        let Some(mut data) = screensaver_data() else { return };
        data.exe.scenes.clear();
        assert!(Engine::new(data, Config::default(), 1).is_err());
    }

    #[test]
    fn a_catalog_without_a_final_scene_is_rejected() {
        let Some(mut data) = screensaver_data() else { return };
        data.exe.scenes.retain(|s| s.flags & SCENE_FINAL == 0);
        assert!(Engine::new(data, Config::default(), 1).is_err());
    }

    #[test]
    fn zero_scene_weights_still_plan_sessions() {
        let Some(mut data) = screensaver_data() else { return };
        for s in &mut data.exe.scenes {
            s.weight = 0;
        }
        let mut engine = Engine::new(data, Config::default(), 7).unwrap();
        run_steps(&mut engine, 1500);
    }

    #[test]
    fn weights_summing_past_65535_still_plan_sessions() {
        let Some(mut data) = screensaver_data() else { return };
        let catalog = data.exe.scenes.clone();
        for _ in 0..40 {
            data.exe.scenes.extend(catalog.iter().cloned());
        }
        for s in &mut data.exe.scenes {
            s.weight = 255;
        }
        let mut engine = Engine::new(data, Config::default(), 7).unwrap();
        run_steps(&mut engine, 1500);
    }

    #[test]
    fn routes_that_never_reach_the_target_do_not_panic() {
        let Some(mut data) = screensaver_data() else { return };
        data.exe.routes.clear();
        let mut engine = Engine::new(data, Config::default(), 7).unwrap();
        run_steps(&mut engine, 1500);
    }
}
