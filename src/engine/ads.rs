//! ADS sequencing scripts and the scene scheduler (seg10 of the original).
//!
//! An ADS file is split into tags (sections). Each active tag's script is re-evaluated on
//! every scheduler tick; it starts and stops TTM scenes, whose current frames are then
//! executed (drawn) in list order.

use super::Env;
use super::rng::Rng;
use super::ttm::{self, Scene, TtmCtx, state};
use crate::assets::resource::{Ads, Archive};
use crate::assets::{AssetError, Result};

const TAG_FIRST_RUN: u16 = 8;

/// Deepest nesting of IF / RANDOM blocks the interpreter follows. The installed scripts nest a few levels;
/// the limit keeps a crafted script from exhausting the stack (`panic = "abort"` makes that fatal) or from
/// making each tick quadratic in the script size.
const MAX_DEPTH: u32 = 32;

/// Most TTMs one ADS may list; the installed ones list at most 6.
const MAX_ADS_TTMS: usize = 16;

/// Most instructions across the TTMs of one ADS; the installed ones total at most 5,995. A list that names one
/// TTM many times would otherwise decode it that many times.
const MAX_ADS_OPS: usize = 250_000;

/// Tag states (low bits of `Tag::state`).
const TAG_RUNNING: u16 = 1;
const TAG_STOPPED: u16 = 2;
/// Start requested: reset the tag's scenes, then run.
const TAG_RESTART: u16 = 3;
/// Start requested: run without resetting the tag's scenes.
const TAG_CONTINUE: u16 = 4;

struct Tag {
    id: u16,
    code: usize,
    state: u16,
    counter: u16,
    resume: Option<usize>,
    scenes: Vec<usize>,
}

pub struct AdsRuntime {
    pub id: u8,
    script: Vec<u8>,
    tags: Vec<Tag>,
    ttms: Vec<TtmCtx>,
    scenes: Vec<Scene>,
    /// Scene drawing order (the original's linked list).
    order: Vec<usize>,
    cur: usize,
    brk: bool,
    cond: bool,
    now: u32,
    /// Current nesting of `exec` / `skip_block` calls.
    depth: u32,
}

fn is_condition(op: u16) -> bool {
    (0x1310..=0x13C1).contains(&op)
}

impl AdsRuntime {
    pub fn load(archive: &Archive, id: u8, name: &str) -> Result<Self> {
        let Ads { ttms: list, script } = archive.ads(name)?;
        if list.len() > MAX_ADS_TTMS {
            return Err(AssetError::Format(format!("{name}: too many TTMs")));
        }
        let mut ttms = Vec::new();
        let mut scenes = Vec::new();
        let mut ops = 0;
        for (slot, ttm_name) in list {
            let ttm = archive.ttm(&ttm_name)?;
            ops += ttm.frames.iter().map(Vec::len).sum::<usize>();
            if ops > MAX_ADS_OPS {
                return Err(AssetError::Format(format!("{name}: TTMs too large")));
            }
            for &(tag, frame) in &ttm.scenes {
                scenes.push(Scene::new(ttms.len(), slot, tag, frame));
            }
            ttms.push(TtmCtx::new(ttm));
        }
        if script.len() < 2 {
            return Err(AssetError::Format(format!("{name}: empty script")));
        }
        let mut ads = Self {
            id,
            order: (0..scenes.len()).collect(),
            script,
            tags: Vec::new(),
            ttms,
            scenes,
            cur: 0,
            brk: false,
            cond: false,
            now: 0,
            depth: 0,
        };
        // seg10:04fc: tags are 0xFFFF-separated sections, each starting with its id.
        let mut p = Some(2usize);
        let mut start = 2usize;
        while let Some(q) = p {
            if ads.w(q) == 0xFFFF {
                let tag = Tag {
                    id: ads.w(start - 2),
                    code: start,
                    state: TAG_FIRST_RUN,
                    counter: 0,
                    resume: None,
                    scenes: ads.tag_scenes(start),
                };
                ads.tags.push(tag);
                start = q + 4;
                p = (start < ads.script.len()).then_some(start);
            } else {
                p = ads.next(q);
            }
        }
        for s in &mut ads.scenes {
            s.reset();
        }
        Ok(ads)
    }

    fn w(&self, p: usize) -> u16 {
        match self.script.get(p..p + 2) {
            Some(b) => u16::from_le_bytes([b[0], b[1]]),
            None => 0xFFFF,
        }
    }

    fn arg(&self, p: usize, i: usize) -> i32 {
        self.w(p + 2 + i * 2) as i16 as i32
    }

    /// seg10:0303 – instruction length table.
    fn next(&self, p: usize) -> Option<usize> {
        if p + 1 >= self.script.len() {
            return None;
        }
        let len = match self.w(p) {
            0x2000 | 0x2005 => 10,
            0x2010 | 0x2015 | 0x2020 | 0x4000 | 0x4010 => 8,
            0x1010..=0x1070 | 0x1310..=0x1370 => 6,
            0xF010 | 0xF200 | 0xF210 | 0x1080 | 0x1380 | 0x1390 | 0x13A0 | 0x13A1 | 0x13B0 | 0x13B1 | 0x13C0
            | 0x13C1 | 0x3020 => 4,
            _ => 2,
        };
        Some(p + len)
    }

    /// seg10:040f – scenes started by a tag (0x2000/0x2005), reset when the tag restarts.
    fn tag_scenes(&self, start: usize) -> Vec<usize> {
        let mut list = Vec::new();
        let mut p = Some(start);
        while let Some(q) = p {
            let op = self.w(q);
            if op == 0xFFFF {
                break;
            }
            if (op == 0x2000 || op == 0x2005)
                && let Some(s) = self.find_scene(self.arg(q, 0), self.arg(q, 1))
                && !list.contains(&s)
            {
                list.push(s);
            }
            p = self.next(q);
        }
        list
    }

    fn find_scene(&self, slot: i32, tag: i32) -> Option<usize> {
        self.order
            .iter()
            .copied()
            .find(|&i| self.scenes[i].slot as i32 == slot && self.scenes[i].tag as i32 == tag)
    }

    fn tag_index(&self, id: i32) -> Option<usize> {
        self.tags.iter().position(|t| t.id as i32 == id)
    }

    /// seg10:0874 – start a tag (state 3: reset its scenes, then run).
    pub fn start_tag(&mut self, id: u8) {
        if let Some(i) = self.tag_index(id as i32) {
            let t = &mut self.tags[i];
            t.resume = None;
            t.state = (t.state & TAG_FIRST_RUN) | TAG_RESTART;
        }
    }

    /// seg10:0766 – is the tag still running (directly or through non-looping scenes)?
    pub fn tag_busy(&self, id: u8) -> bool {
        let Some(i) = self.tag_index(id as i32) else {
            return false;
        };
        let st = self.tags[i].state & !TAG_FIRST_RUN;
        if st == TAG_CONTINUE || st == TAG_RUNNING {
            return true;
        }
        self.tags[i].scenes.iter().any(|&s| {
            let sc = &self.scenes[s];
            sc.state != state::DONE && sc.state != state::IDLE && !sc.looping
        })
    }

    // ---- condition evaluation (seg10:1223) ----

    fn condition(&self, op: u16, p: usize) -> bool {
        let scene = self.find_scene(self.arg(p, 0), self.arg(p, 1)).map(|i| &self.scenes[i]);
        match op & 0xFF {
            0x10 => scene.is_some_and(|s| s.state == state::FROZEN),
            0x20 => scene.is_none_or(|s| s.state != state::FROZEN),
            0x30 => scene.is_none_or(|s| s.plays == 0),
            0x40 => scene.is_some_and(|s| s.plays != 0),
            0x50 => scene.is_some_and(|s| s.state == state::DONE),
            0x60 => scene.is_none_or(|s| s.state == state::DONE || s.state == state::IDLE),
            0x70 => scene.is_some_and(|s| (1..=3).contains(&s.state)),
            0x80 => self.tags[self.cur].counter as i32 <= self.arg(p, 0),
            _ => false, // ADS variables: unused by this screensaver
        }
    }

    /// Evaluates a condition chain with AND/OR (left to right, short-circuit as the original).
    fn eval_cond(&mut self, mut p: usize) -> Option<usize> {
        let mut result = false;
        let mut or_hit = false;
        loop {
            if self.condition(self.w(p), p) {
                result = true;
            }
            loop {
                p = self.next(p)?;
                let op = self.w(p);
                if op != 0x1430 && op != 0x1420 {
                    self.cond = result || or_hit;
                    return Some(p);
                }
                if result {
                    if op == 0x1430 {
                        or_hit = true;
                        p = self.next(p)?; // skip the OR, then the next condition below
                        continue;
                    }
                    result = false; // AND: evaluate the next condition
                    p = self.next(p)?;
                    break;
                }
                if op == 0x1430 {
                    p = self.next(p)?;
                    break;
                }
                p = self.next(p)?; // false AND x: skip x
            }
        }
    }

    // ---- block structure ----

    /// seg10:106b – skip to the matching ELSE/ENDIF.
    fn skip_block(&mut self, p: usize) -> Option<usize> {
        if self.depth >= MAX_DEPTH {
            return None;
        }
        self.depth += 1;
        let r = self.skip_block_inner(p);
        self.depth -= 1;
        r
    }

    fn skip_block_inner(&mut self, mut p: usize) -> Option<usize> {
        loop {
            let op = self.w(p);
            match op {
                0 | 0xFFFF => return None,
                0x1510 | 0x1500 => return Some(p),
                _ if is_condition(op) => {
                    let cond_end = self.eval_cond(p)?;
                    let mut q = self.skip_block(cond_end)?;
                    if self.w(q) == 0x1500 {
                        q = self.skip_block(q + 2)?;
                    }
                    p = self.next(q)?;
                }
                _ => p = self.next(p)?,
            }
        }
    }

    /// seg10:11db – skip the IF branch and run the ELSE branch if present.
    fn skip_to_else(&mut self, p: usize, rng: &mut Rng) -> Option<usize> {
        let q = self.skip_block(p)?;
        if self.w(q) == 0x1500 {
            self.run_block(q + 2, rng)
        } else {
            Some(q)
        }
    }

    /// seg10:118a – skip a WAIT block up to its 0x1520 terminator.
    fn skip_wait(&self, mut p: usize) -> Option<usize> {
        loop {
            match self.w(p) {
                0 | 0xFFFF => return None,
                0x1520 => return Some(p),
                _ => p = self.next(p)?,
            }
        }
    }

    /// seg10:1925 – run statements until a block terminator or the end of the tag.
    fn run_block(&mut self, mut p: usize, rng: &mut Rng) -> Option<usize> {
        while self.w(p) != 0xFFFF {
            let r = self.exec(p, rng);
            if self.brk {
                self.brk = false;
                return r;
            }
            p = r?;
        }
        None
    }

    fn weight(&self, p: usize) -> i32 {
        match self.w(p) {
            0x2000 | 0x2005 => self.arg(p, 3),
            0x3020 => self.arg(p, 0),
            _ => self.arg(p, 2),
        }
    }

    /// seg10:0cda – RANDOM … END_RANDOM: pick one statement by weight (0x3020 = do nothing).
    fn random(&mut self, start: usize, rng: &mut Rng) -> Option<usize> {
        let mut total = 0i32;
        let mut p = start;
        while self.w(p) != 0x30FF {
            total = total.saturating_add(self.weight(p));
            p = self.next(p)?;
        }
        let after = p + 2;
        if total <= 0 {
            return Some(after);
        }
        let mut r = (rng.below(total as u16) as i16 as i32).abs() + 1;
        let mut p = start;
        loop {
            r -= self.weight(p);
            if r <= 0 {
                break;
            }
            p = self.next(p)?;
        }
        if self.w(p) != 0x3020 {
            self.exec(p, rng);
        }
        Some(after)
    }

    /// seg10:0db6 – start a scene: n=0 once, n>0 n times, n<0 for -n ticks.
    fn add_scene(&mut self, slot: i32, tag: i32, n: i32) {
        if let Some(i) = self.find_scene(slot, tag) {
            let now = self.now;
            let s = &mut self.scenes[i];
            match n {
                0 => s.state = state::ONCE,
                n if n > 0 => {
                    s.state = state::REPEAT;
                    s.repeats = n - 1;
                }
                n => {
                    s.state = state::TIMED;
                    s.end_time = now.wrapping_add(n.unsigned_abs());
                }
            }
            s.plays += 1;
        }
    }

    /// seg10:1519 – execute one statement, returning the next one (None stops the tag).
    fn exec(&mut self, p: usize, rng: &mut Rng) -> Option<usize> {
        if self.depth >= MAX_DEPTH {
            return None;
        }
        self.depth += 1;
        let r = self.exec_inner(p, rng);
        self.depth -= 1;
        r
    }

    fn exec_inner(&mut self, p: usize, rng: &mut Rng) -> Option<usize> {
        let op = self.w(p);
        let (a0, a1, a2) = (self.arg(p, 0), self.arg(p, 1), self.arg(p, 2));
        match op {
            0x0001 | 0x0005 => Some(p + 2),
            0xFFFF => None,
            0x1500 => {
                let q = self.skip_block(p + 2);
                self.brk = true;
                q
            }
            0x1510 => {
                self.brk = true;
                Some(p)
            }
            0x1520 => {
                self.brk = true;
                None
            }
            0x2000 => {
                if let Some(i) = self.find_scene(a0, a1) {
                    self.scenes[i].cur = self.scenes[i].start;
                }
                self.add_scene(a0, a1, a2);
                Some(p + 10)
            }
            0x2005 => {
                self.add_scene(a0, a1, a2);
                Some(p + 10)
            }
            0x2010 | 0x2015 | 0x2020 => {
                if let Some(i) = self.find_scene(a0, a1) {
                    match op {
                        0x2010 => self.scenes[i].state = state::IDLE,
                        0x2015 => self.scenes[i].state = state::FROZEN,
                        _ => self.scenes[i].reset(),
                    }
                }
                Some(p + 8)
            }
            0x4000 | 0x4010 => {
                if let Some(i) = self.find_scene(a0, a1) {
                    self.order.retain(|&s| s != i);
                    if op == 0x4000 {
                        self.order.push(i)
                    } else {
                        self.order.insert(0, i)
                    }
                }
                Some(p + 8)
            }
            0x3010 => self.random(p + 2, rng),
            0xF000 => {
                self.tags[self.cur].state = TAG_STOPPED;
                None
            }
            0xF010 => {
                let i = if a0 == -1 { Some(self.cur) } else { self.tag_index(a0) };
                if let Some(i) = i {
                    self.tags[i].state = TAG_STOPPED;
                    if i == self.cur {
                        return None;
                    }
                }
                Some(p + 4)
            }
            0xF200 | 0xF210 => {
                if let Some(i) = self.tag_index(a0) {
                    let t = &mut self.tags[i];
                    t.resume = None;
                    t.state = (t.state & TAG_FIRST_RUN) | if op == 0xF200 { TAG_CONTINUE } else { TAG_RESTART };
                }
                Some(p + 4)
            }
            _ if is_condition(op) => {
                let q = self.eval_cond(p)?;
                let r = if self.cond {
                    self.run_block(q, rng)
                } else {
                    self.skip_to_else(q, rng)
                };
                r.map(|r| r + 2)
            }
            0x1010..=0x1080 if op & 0xF == 0 => {
                // WAIT-style condition: re-evaluated from here on every tick while true.
                let q = self.eval_cond(p)?;
                let cur = self.cur;
                if self.cond {
                    self.tags[cur].counter += 1;
                    self.tags[cur].resume = Some(p);
                    self.run_block(q, rng);
                    None
                } else {
                    self.tags[cur].counter = 0;
                    self.tags[cur].resume = None;
                    self.skip_wait(q).map(|r| r + 2)
                }
            }
            _ => None, // 0x3020/0x1420/0x1430 outside their context, unknown opcodes
        }
    }

    /// seg10:0c30 – a tag's re-entry point: just after a 0x0005 marker, if any.
    fn reentry(&self, start: usize) -> usize {
        let mut p = Some(start);
        while let Some(q) = p {
            match self.w(q) {
                0x0005 => return q + 2,
                0xFFFF => break,
                _ => p = self.next(q),
            }
        }
        start
    }

    /// seg10:1acb – one scheduler tick. Returns true if any scene ran a frame.
    pub fn update(&mut self, env: &mut Env<'_>) -> bool {
        self.now = env.now;

        // Propagate tag states to their scenes; restarted tags reset their scenes.
        for t in 0..self.tags.len() {
            let flags = self.tags[t].state & !TAG_FIRST_RUN;
            for k in 0..self.tags[t].scenes.len() {
                let s = self.tags[t].scenes[k];
                if flags == TAG_RESTART {
                    self.scenes[s].reset()
                } else {
                    self.scenes[s].tag_state = flags
                }
            }
        }

        // Run every active tag's script.
        for t in 0..self.tags.len() {
            let mut st = self.tags[t].state;
            let mut p = self.tags[t].code;
            if st & TAG_FIRST_RUN != 0 {
                st &= !TAG_FIRST_RUN;
                self.tags[t].state = st;
            } else {
                p = self.reentry(p);
            }
            if let Some(r) = self.tags[t].resume {
                p = r;
            }
            if st == TAG_RESTART || st == TAG_CONTINUE {
                self.tags[t].state = TAG_RUNNING;
                st = TAG_RUNNING;
            }
            self.cur = t;
            if st == TAG_RUNNING {
                self.run_block(p, env.rng);
            }
        }

        // Execute (draw) the current frame of every running scene, in list order.
        let mut ran = false;
        let now = self.now;
        for k in 0..self.order.len() {
            let i = self.order[k];
            let (ttm_index, tag_state, old_state) = {
                let s = &self.scenes[i];
                (s.ttm, s.tag_state, s.state)
            };
            let running = matches!(old_state, state::ONCE | state::REPEAT | state::TIMED | state::FROZEN);
            if tag_state != 6 && running {
                let scene = &mut self.scenes[i];
                match ttm::run_frame(&mut self.ttms[ttm_index], scene, env) {
                    None => {
                        if tag_state != 5 {
                            scene.goto = scene.start;
                            scene.state = state::DONE;
                        }
                    }
                    Some(_) if tag_state == 5 => {}
                    Some(res) => {
                        scene.redraw = true;
                        ran = true;
                        if let Some(d) = res.delay
                            && d != scene.delay
                        {
                            scene.next_time = now.wrapping_add(d as u32);
                            scene.delay = d;
                        }
                        if res.purge {
                            scene.goto = scene.start;
                            if scene.state == state::REPEAT && scene.repeats != 0 {
                                if scene.advance(now) {
                                    scene.repeats -= 1;
                                }
                            } else if scene.state == state::TIMED && scene.end_time != 0 {
                                scene.advance(now);
                            } else if scene.advance(now) {
                                scene.state = state::DONE;
                                scene.delay = 0;
                            }
                        } else {
                            if let Some(g) = res.goto
                                && g != -1
                            {
                                scene.goto = g;
                                if scene.cur == g {
                                    scene.looping = true;
                                }
                            }
                            if scene.state != state::FROZEN {
                                scene.advance(now);
                            }
                        }
                    }
                }
            } else if tag_state != 6 && tag_state != 5 && old_state == state::DONE {
                self.scenes[i].state = state::IDLE;
            }
            if old_state == state::TIMED && self.scenes[i].end_time <= now {
                self.scenes[i].state = state::DONE;
            }
        }
        ran
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::resource::fixtures::*;

    /// An archive with one ADS, `A.ADS`, that lists `ttms` as (slot, name) and has the given script, plus
    /// `extra` entries.
    fn archive_with(script: &[u8], ttms: &[(u16, &str)], extra: &[(&str, Vec<u8>)]) -> Archive {
        let mut res = (ttms.len() as u16).to_le_bytes().to_vec();
        for (slot, name) in ttms {
            res.extend_from_slice(&slot.to_le_bytes());
            res.extend_from_slice(name.as_bytes());
            res.push(0);
        }
        let mut body = chunk(b"RES:", &res);
        body.extend(chunk(b"SCR:", &rle(script)));
        let mut entries = vec![("A.ADS", body)];
        entries.extend(extra.iter().map(|(n, b)| (*n, b.clone())));
        archive_of(&entries)
    }

    fn archive_with_script(script: &[u8]) -> Archive {
        archive_with(script, &[], &[])
    }

    /// Tag 1 holding `ops`, ended by the 0xFFFF separator.
    fn script(ops: &[u16]) -> Vec<u8> {
        std::iter::once(1u16)
            .chain(ops.iter().copied())
            .chain([0xFFFF, 0])
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    fn runtime(ops: &[u16]) -> AdsRuntime {
        AdsRuntime::load(&archive_with_script(&script(ops)), 1, "A.ADS").unwrap()
    }

    fn rng() -> Rng {
        Rng::new([0; 56], 24, 55)
    }

    /// Runs `f` on a thread with the stack the engine's main thread has and reports whether it returned within
    /// five seconds. A stack overflow aborts the whole process, which also fails the test.
    fn finishes_promptly(f: impl FnOnce() + Send + 'static) -> bool {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .stack_size(1 << 20)
            .spawn(move || {
                f();
                let _ = tx.send(());
            })
            .unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(5)).is_ok()
    }

    #[test]
    fn deeply_nested_conditions_run_without_overflowing_the_stack() {
        // IF (scene never played) x 100k: each nested block runs, then recurses.
        let ops: Vec<u16> = (0..100_000).flat_map(|_| [0x1330, 0, 0]).collect();
        assert!(finishes_promptly(move || {
            let mut ads = runtime(&ops);
            ads.run_block(2, &mut rng());
        }));
    }

    #[test]
    fn deeply_nested_skipped_conditions_do_not_overflow_the_stack() {
        // IF (scene is running) x 100k: false, so the whole nest is skipped by recursion.
        let ops: Vec<u16> = (0..100_000).flat_map(|_| [0x1370, 0, 0]).collect();
        assert!(finishes_promptly(move || {
            let mut ads = runtime(&ops);
            ads.run_block(2, &mut rng());
        }));
    }

    #[test]
    fn nested_random_blocks_are_bounded() {
        let mut ops = vec![0x3010; 100_000];
        ops.push(0x30FF);
        assert!(finishes_promptly(move || {
            let mut ads = runtime(&ops);
            ads.run_block(2, &mut rng());
        }));
    }

    #[test]
    fn random_weights_that_overflow_do_not_panic() {
        // 70k statements of weight 32767 sum past i32::MAX.
        let mut ops: Vec<u16> = (0..70_000).flat_map(|_| [0x3020, 0x7FFF]).collect();
        ops.push(0x30FF);
        ops.insert(0, 0x3010);
        assert!(finishes_promptly(move || {
            let mut ads = runtime(&ops);
            ads.run_block(2, &mut rng());
        }));
    }

    #[test]
    fn installed_scripts_nest_well_within_the_depth_limit() {
        let Some(dir) = std::env::var_os("OPEN_SCRANTIC_DATA") else {
            return;
        };
        let data = crate::assets::ScreensaverData::load(std::path::Path::new(&dir)).expect("load");
        let mut deepest = 0;
        for (&id, name) in &data.exe.ads_names {
            let ads = AdsRuntime::load(&data.archive, id, name).unwrap_or_else(|e| panic!("{name}: {e}"));
            let (mut depth, mut p, mut prev) = (0i32, 2, 0u16);
            while let Some(q) = ads.next(p) {
                let op = ads.w(p);
                // A condition right after AND / OR continues the chain instead of opening a block.
                let chained = prev == 0x1420 || prev == 0x1430;
                match op {
                    0xFFFF => depth = 0, // next tag
                    op if (is_condition(op) && !chained) || (0x1010..=0x1080).contains(&op) && op & 0xF == 0 => {
                        depth += 1
                    }
                    0x3010 => depth += 1,
                    0x1510 | 0x1520 | 0x30FF => depth -= 1,
                    _ => {}
                }
                deepest = deepest.max(depth);
                prev = op;
                p = q;
            }
        }
        // Each level costs an `exec` and, when skipping, a `skip_block`.
        assert!(2 * deepest as u32 + 2 <= MAX_DEPTH, "deepest nesting {deepest}");
    }

    #[test]
    fn an_ads_listing_one_ttm_many_times_is_rejected() {
        let t = ("T.TTM", ttm_body(1, &[0x0FF0; 4]));
        let few: Vec<(u16, &str)> = (0..MAX_ADS_TTMS as u16).map(|i| (i, "T.TTM")).collect();
        assert!(AdsRuntime::load(&archive_with(&script(&[]), &few, std::slice::from_ref(&t)), 1, "A.ADS").is_ok());
        let many: Vec<(u16, &str)> = (0..=MAX_ADS_TTMS as u16).map(|i| (i, "T.TTM")).collect();
        assert!(AdsRuntime::load(&archive_with(&script(&[]), &many, &[t]), 1, "A.ADS").is_err());
    }

    #[test]
    fn ttms_that_are_large_together_are_rejected() {
        // Each TTM is within its own limit; four references to it are not within the ADS's.
        let ops = vec![0x0110u16; 90_000];
        let t = ("T.TTM", ttm_body(1, &ops));
        let one = [(0u16, "T.TTM")];
        assert!(AdsRuntime::load(&archive_with(&script(&[]), &one, std::slice::from_ref(&t)), 1, "A.ADS").is_ok());
        let four = [(0u16, "T.TTM"), (1, "T.TTM"), (2, "T.TTM"), (3, "T.TTM")];
        assert!(AdsRuntime::load(&archive_with(&script(&[]), &four, &[t]), 1, "A.ADS").is_err());
    }
}
