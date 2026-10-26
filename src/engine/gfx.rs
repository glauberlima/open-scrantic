//! 640x480 palette-indexed drawing surface reproducing the GDI operations the original used.

use crate::assets::resource::{Picture, Sprite};

pub const WIDTH: i32 = 640;
pub const HEIGHT: i32 = 480;

/// Sprite blit flags, as passed by TTM opcodes (bits 4..7 of 0xA5x4).
pub const FLIP_V: u8 = 1;
pub const FLIP_H: u8 = 2;

/// Inclusive clip rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clip {
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
}

impl Clip {
    pub const FULL: Clip = Clip {
        x1: 0,
        y1: 0,
        x2: WIDTH - 1,
        y2: HEIGHT - 1,
    };

    pub fn rect(x: i32, y: i32, w: i32, h: i32) -> Clip {
        Clip {
            x1: x,
            y1: y,
            x2: x + w - 1,
            y2: y + h - 1,
        }
    }

    /// The clip bounded to the canvas, or `None` when nothing is left.
    fn visible(self) -> Option<Clip> {
        Some(self.bounded()).filter(|c| c.x1 <= c.x2 && c.y1 <= c.y2)
    }

    /// Pixel index range of row `y` within the clip.
    fn row(self, y: i32) -> (usize, usize) {
        ((y * WIDTH + self.x1) as usize, (y * WIDTH + self.x2 + 1) as usize)
    }

    fn bounded(self) -> Clip {
        Clip {
            x1: self.x1.max(0),
            y1: self.y1.max(0),
            x2: self.x2.min(WIDTH - 1),
            y2: self.y2.min(HEIGHT - 1),
        }
    }
}

#[derive(Clone)]
pub struct Canvas {
    pub px: Vec<u8>,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            px: vec![0; (WIDTH * HEIGHT) as usize],
        }
    }
}

impl Canvas {
    pub fn fill(&mut self, color: u8) {
        self.px.fill(color);
    }

    #[inline]
    fn put(&mut self, x: i32, y: i32, c: u8, clip: Clip) {
        if x >= clip.x1 && x <= clip.x2 && y >= clip.y1 && y <= clip.y2 {
            self.px[(y * WIDTH + x) as usize] = c;
        }
    }

    pub fn pixel(&mut self, x: i32, y: i32, c: u8, clip: Clip) {
        self.put(x, y, c, clip.bounded());
    }

    /// Loads a full-screen picture (SCR); pictures shorter than 480 lines leave the rest untouched.
    pub fn load_picture(&mut self, pic: &Picture) {
        for y in 0..pic.h.min(HEIGHT as usize) {
            let w = pic.w.min(WIDTH as usize);
            let dst = y * WIDTH as usize;
            self.px[dst..dst + w].copy_from_slice(&pic.px[y * pic.w..y * pic.w + w]);
        }
    }

    pub fn copy_rect_from(&mut self, src: &Canvas, x: i32, y: i32, w: i32, h: i32) {
        let Some(c) = Clip::rect(x, y, w, h).visible() else {
            return;
        };
        for yy in c.y1..=c.y2 {
            let (a, b) = c.row(yy);
            self.px[a..b].copy_from_slice(&src.px[a..b]);
        }
    }

    /// Copies the pixels of `r` (clipped to the canvas) out, row by row.
    pub fn save_rect(&self, r: Clip) -> Vec<u8> {
        let mut out = Vec::new();
        if let Some(c) = r.visible() {
            for y in c.y1..=c.y2 {
                let (a, b) = c.row(y);
                out.extend_from_slice(&self.px[a..b]);
            }
        }
        out
    }

    /// Puts back what `save_rect` took for the same `r`.
    pub fn restore_rect(&mut self, r: Clip, saved: &[u8]) {
        let Some(c) = r.visible() else {
            return;
        };
        let w = (c.x2 - c.x1 + 1) as usize;
        for (row, y) in saved.chunks_exact(w).zip(c.y1..=c.y2) {
            let (a, b) = c.row(y);
            self.px[a..b].copy_from_slice(row);
        }
    }

    pub fn sprite(&mut self, s: &Sprite, x: i32, y: i32, flags: u8, clip: Clip) {
        let c = clip.bounded();
        let (x0, y0) = (x.max(c.x1), y.max(c.y1));
        let (x1, y1) = ((x + s.w - 1).min(c.x2), (y + s.h - 1).min(c.y2));
        for dy in y0..=y1 {
            let sy = if flags & FLIP_V != 0 {
                s.h - 1 - (dy - y)
            } else {
                dy - y
            };
            let row = (sy * s.w) as usize;
            for dx in x0..=x1 {
                let sx = if flags & FLIP_H != 0 {
                    s.w - 1 - (dx - x)
                } else {
                    dx - x
                };
                let v = s.px[row + sx as usize];
                if v != 0 {
                    self.px[(dy * WIDTH + dx) as usize] = v;
                }
            }
        }
    }

    /// GDI MoveTo/LineTo: the end point itself is not drawn.
    pub fn line(&mut self, x1: i32, y1: i32, x2: i32, y2: i32, c: u8, clip: Clip) {
        let clip = clip.bounded();
        let (dx, dy) = ((x2 - x1).abs(), -(y2 - y1).abs());
        let (sx, sy) = ((x2 - x1).signum(), (y2 - y1).signum());
        let (mut x, mut y, mut err) = (x1, y1, dx + dy);
        while (x, y) != (x2, y2) {
            self.put(x, y, c, clip);
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// TTM rectangle: `filled` draws a 1px `fg` border around a `bg` interior.
    #[allow(clippy::too_many_arguments)] // mirrors the original GDI-backed primitive
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, fg: u8, bg: u8, filled: bool, clip: Clip) {
        let clip = clip.bounded();
        if w <= 0 || h <= 0 {
            return;
        }
        // Only the part inside the clip is visited: the arguments are 16-bit, so the whole rectangle can be
        // a billion pixels of which `put` would reject nearly all.
        for yy in y.max(clip.y1)..(y + h).min(clip.y2 + 1) {
            for xx in x.max(clip.x1)..(x + w).min(clip.x2 + 1) {
                let border = xx == x || yy == y || xx == x + w - 1 || yy == y + h - 1;
                if border {
                    self.put(xx, yy, if filled && w > 1 && h > 1 && fg == bg { bg } else { fg }, clip);
                } else if filled {
                    self.put(xx, yy, bg, clip);
                }
            }
        }
    }

    /// GDI Ellipse in the box [cx-r, cx+r): `fg` outline, `bg` interior.
    pub fn ellipse(&mut self, cx: i32, cy: i32, r: i32, fg: u8, bg: u8, clip: Clip) {
        let clip = clip.bounded();
        if r <= 0 {
            return;
        }
        let rf = r as f32;
        let inside = |px: i32, py: i32| {
            let (fx, fy) = (px as f32 + 0.5 - cx as f32, py as f32 + 0.5 - cy as f32);
            fx * fx + fy * fy <= rf * rf
        };
        for py in (cy - r).max(clip.y1)..(cy + r).min(clip.y2 + 1) {
            for px in (cx - r).max(clip.x1)..(cx + r).min(clip.x2 + 1) {
                if !inside(px, py) {
                    continue;
                }
                let edge = !inside(px - 1, py) || !inside(px + 1, py) || !inside(px, py - 1) || !inside(px, py + 1);
                self.put(px, py, if edge { fg } else { bg }, clip);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_rect_outside_the_canvas_is_a_no_op() {
        let src = Canvas {
            px: vec![7; (WIDTH * HEIGHT) as usize],
        };
        let mut dst = Canvas::default();
        for (x, y) in [(-10, 0), (-10, 5), (WIDTH + 3, 0), (0, HEIGHT + 3), (0, -10)] {
            dst.copy_rect_from(&src, x, y, 5, 5);
        }
        assert!(dst.px.iter().all(|&p| p == 0));
    }

    #[test]
    fn copy_rect_is_clipped_to_the_canvas() {
        let src = Canvas {
            px: vec![7; (WIDTH * HEIGHT) as usize],
        };
        let mut dst = Canvas::default();
        dst.copy_rect_from(&src, -2, -2, 5, 5);
        assert_eq!(dst.px.iter().filter(|&&p| p == 7).count(), 9);
        assert_eq!(dst.px[0], 7);
    }

    /// Runs `f` on a thread and reports whether it returned within a second.
    fn finishes_promptly(f: impl FnOnce() + Send + 'static) -> bool {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            f();
            let _ = tx.send(());
        });
        rx.recv_timeout(std::time::Duration::from_secs(1)).is_ok()
    }

    #[test]
    fn huge_rect_costs_only_the_visible_part() {
        assert!(finishes_promptly(|| {
            let mut c = Canvas::default();
            c.rect(-30000, -30000, 32767, 32767, 1, 2, true, Clip::FULL);
            c.rect(-100, -100, 32767, 32767, 1, 2, true, Clip::FULL);
        }));
    }

    #[test]
    fn huge_ellipse_costs_only_the_visible_part() {
        assert!(finishes_promptly(|| {
            let mut c = Canvas::default();
            c.ellipse(320, 240, 32767, 1, 2, Clip::FULL);
            c.ellipse(-20000, 5, 32767, 1, 2, Clip::FULL);
        }));
    }

    /// The unclipped loops this module used before the visible region was computed up front.
    #[allow(clippy::too_many_arguments)] // mirrors Canvas::rect
    fn reference_rect(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, fg: u8, bg: u8, filled: bool, clip: Clip) {
        let clip = clip.bounded();
        for yy in y..y + h {
            for xx in x..x + w {
                let border = xx == x || yy == y || xx == x + w - 1 || yy == y + h - 1;
                if border {
                    c.put(xx, yy, if filled && w > 1 && h > 1 && fg == bg { bg } else { fg }, clip);
                } else if filled {
                    c.put(xx, yy, bg, clip);
                }
            }
        }
    }

    #[test]
    fn rect_matches_the_unclipped_loops() {
        let clips = [Clip::FULL, Clip::rect(100, 50, 200, 120)];
        for clip in clips {
            for (x, y, w, h) in [
                (-5, -5, 20, 20),
                (90, 40, 30, 30),
                (600, 450, 100, 100),
                (-50, 10, 900, 5),
                (7, 7, 1, 1),
            ] {
                for filled in [false, true] {
                    let (mut a, mut b) = (Canvas::default(), Canvas::default());
                    a.rect(x, y, w, h, 3, 4, filled, clip);
                    reference_rect(&mut b, x, y, w, h, 3, 4, filled, clip);
                    assert!(a.px == b.px, "rect {x},{y} {w}x{h} filled={filled}");
                }
            }
        }
    }

    #[test]
    fn ellipse_matches_the_unclipped_loops() {
        for (cx, cy, r) in [
            (320, 240, 50),
            (5, 5, 30),
            (630, 470, 40),
            (-10, 240, 25),
            (100, 100, 1),
        ] {
            let (mut a, mut b) = (Canvas::default(), Canvas::default());
            a.ellipse(cx, cy, r, 3, 4, Clip::FULL);
            let rf = r as f32;
            let inside = |px: i32, py: i32| {
                let (fx, fy) = (px as f32 + 0.5 - cx as f32, py as f32 + 0.5 - cy as f32);
                fx * fx + fy * fy <= rf * rf
            };
            for py in cy - r..cy + r {
                for px in cx - r..cx + r {
                    if inside(px, py) {
                        let edge =
                            !inside(px - 1, py) || !inside(px + 1, py) || !inside(px, py - 1) || !inside(px, py + 1);
                        b.put(px, py, if edge { 3 } else { 4 }, Clip::FULL);
                    }
                }
            }
            assert!(a.px == b.px, "ellipse {cx},{cy} r={r}");
        }
    }
}
