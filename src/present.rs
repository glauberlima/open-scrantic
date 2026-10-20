//! Presentation of the 640x480 frame: optional dedithering and sharp-bilinear scaling to the window.
//! Nothing here touches the engine; it only changes what reaches the screen (documented deviation).

use crate::engine::gfx::{HEIGHT, WIDTH};

const W: usize = WIDTH as usize;
const H: usize = HEIGHT as usize;

fn pack(c: [u8; 3]) -> u32 {
    (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32
}

/// RGB frame from the palette indices, unchanged.
pub fn plain(px: &[u8], pal: &[[u8; 3]; 256]) -> Vec<u32> {
    px.iter().map(|&i| pack(pal[i as usize])).collect()
}

/// Per-channel midpoint in the palette's own (sRGB) values. Averaging in linear light would be
/// physically closer to what the eye sees at a distance, but it brightens saturated mixes and washes
/// out the colors the palette was designed for.
fn average(a: [u8; 3], b: [u8; 3]) -> [u8; 3] {
    let mix = |x: u8, y: u8| (x as u16 + y as u16).div_ceil(2) as u8;
    [mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2])]
}

/// Shortest alternating run (`a b a b`) taken as dither where a checkerboard thins out to one pixel.
const MIN_RUN: usize = 4;

/// RGB frame where every 2x2 checkerboard block (`a b / b a`, a != b) is replaced by the average of
/// its two colors. The dither of the original is this pattern in two palette indices, so detection
/// is exact index equality. Where a dithered area ends in a one-pixel line there is no block left,
/// so pixels in a horizontal or vertical alternating run of `MIN_RUN` or more are blended too.
/// A pixel in several blocks or runs keeps the partner of the first one found.
pub fn dedither(px: &[u8], pal: &[[u8; 3]; 256]) -> Vec<u32> {
    let mut partner: Vec<Option<u8>> = vec![None; W * H];
    for y in 0..H - 1 {
        for x in 0..W - 1 {
            let i = y * W + x;
            let (a, b, c, d) = (px[i], px[i + 1], px[i + W], px[i + W + 1]);
            if a == d && b == c && a != b {
                for (j, q) in [(i, b), (i + 1, a), (i + W, a), (i + W + 1, b)] {
                    partner[j].get_or_insert(q);
                }
            }
        }
    }
    for (lines, len, along, across) in [(H, W, 1, W), (W, H, W, 1)] {
        for line in 0..lines {
            let base = line * across;
            let mut start = 0;
            for x in 1..=len {
                let continues = x < len
                    && px[base + x * along] != px[base + (x - 1) * along]
                    && (x == 1 || px[base + x * along] == px[base + (x - 2) * along]);
                if continues {
                    continue;
                }
                // An end pixel equal to the pixel outside the run is the edge of a solid area, not dither.
                let lo = start + usize::from(start > 0 && px[base + start * along] == px[base + (start - 1) * along]);
                let hi = x - usize::from(x < len && px[base + x * along] == px[base + (x - 1) * along]);
                if hi.saturating_sub(lo) >= MIN_RUN {
                    for k in lo..hi {
                        let neighbor = if k > lo { k - 1 } else { k + 1 };
                        partner[base + k * along].get_or_insert(px[base + neighbor * along]);
                    }
                }
                start = x;
            }
        }
    }
    let mut pairs = vec![u32::MAX; 256 * 256];
    px.iter()
        .zip(&partner)
        .map(|(&p, &q)| match q {
            None => pack(pal[p as usize]),
            Some(q) => {
                let slot = &mut pairs[p as usize * 256 + q as usize];
                if *slot == u32::MAX {
                    *slot = pack(average(pal[p as usize], pal[q as usize]));
                }
                *slot
            }
        })
        .collect()
}

/// Per output coordinate: source pixel, neighbor across the nearest pixel edge, neighbor weight (0..=128).
fn sharp_axis(n_dst: usize, n_src: usize, k: f64) -> Vec<(usize, usize, u32)> {
    (0..n_dst)
        .map(|o| {
            let s = (o as f64 + 0.5) / k;
            let i = (s as usize).min(n_src - 1);
            let f = s - i as f64;
            let (e, j) = if f < 0.5 {
                (f, i.saturating_sub(1))
            } else {
                (1.0 - f, (i + 1).min(n_src - 1))
            };
            (i, j, ((0.5 - e * k).clamp(0.0, 0.5) * 256.0) as u32)
        })
        .collect()
}

fn mix(a: u32, b: u32, w: u32) -> u32 {
    let ch = |sh: u32| (((a >> sh & 255) * (256 - w) + (b >> sh & 255) * w) >> 8) << sh;
    ch(16) | ch(8) | ch(0)
}

/// Draws `src` (640x480 RGB) into `dst` (`w` x `h`) as large as fits, centered, black elsewhere.
/// Sharp-bilinear: pixels stay solid and are blended with the neighbor only within one screen pixel
/// of an edge, so an integer factor is exact replication and a fractional one has uniform pixels.
pub fn scale(src: &[u32], dst: &mut [u32], w: usize, h: usize) {
    dst.fill(0);
    let k = (w as f64 / W as f64).min(h as f64 / H as f64);
    let (dw, dh) = ((W as f64 * k) as usize, (H as f64 * k) as usize);
    if dw == 0 || dh == 0 {
        return;
    }
    let (ox, oy) = ((w - dw) / 2, (h - dh) / 2);
    let cols = sharp_axis(dw, W, k);
    for (y, &(r0, r1, wy)) in sharp_axis(dh, H, k).iter().enumerate() {
        let (row0, row1) = (&src[r0 * W..][..W], &src[r1 * W..][..W]);
        let out = &mut dst[(y + oy) * w + ox..][..dw];
        for (d, &(c0, c1, wx)) in out.iter_mut().zip(&cols) {
            *d = if wx == 0 && wy == 0 {
                row0[c0]
            } else {
                mix(mix(row0[c0], row0[c1], wx), mix(row1[c0], row1[c1], wx), wy)
            };
        }
    }
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// PNG file (uncompressed deflate) of a `w` x `h` frame of packed RGB.
pub fn png(w: usize, h: usize, frame: &[u32]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w * 3 + 1) * h);
    for row in frame.chunks(w) {
        raw.push(0);
        raw.extend(row.iter().flat_map(|&c| [(c >> 16) as u8, (c >> 8) as u8, c as u8]));
    }
    let mut z = vec![0x78, 0x01];
    for (i, block) in raw.chunks(65535).enumerate() {
        let last = (i + 1) * 65535 >= raw.len();
        z.push(u8::from(last));
        z.extend_from_slice(&(block.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        z.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut chunk = |tag: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut c = tag.to_vec();
        c.extend_from_slice(data);
        out.extend_from_slice(&c);
        out.extend_from_slice(&crc32(&c).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&(w as u32).to_be_bytes());
    ihdr.extend_from_slice(&(h as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pal() -> [[u8; 3]; 256] {
        let mut p = [[0u8; 3]; 256];
        p[1] = [255, 255, 255];
        p[2] = [0, 0, 255];
        p
    }

    fn checker(a: u8, b: u8) -> Vec<u8> {
        (0..W * H)
            .map(|i| if (i % W + i / W).is_multiple_of(2) { a } else { b })
            .collect()
    }

    #[test]
    fn checkerboard_becomes_one_color() {
        let out = dedither(&checker(0, 2), &pal());
        assert!(out.iter().all(|&c| c == out[0]));
        assert_ne!(out[0], pack(pal()[0]));
        assert_ne!(out[0], pack(pal()[2]));
    }

    #[test]
    fn checkerboard_keeps_the_midpoint_of_its_colors() {
        let out = dedither(&checker(0, 2), &pal());
        assert_eq!(out[0], pack([0, 0, 128]));
    }

    #[test]
    fn solid_lines_and_lone_pixels_are_kept() {
        let mut px = vec![1u8; W * H];
        px[10 * W + 10] = 2; // lone pixel
        for x in 0..W {
            px[20 * W + x] = 2; // one-pixel line
        }
        assert_eq!(dedither(&px, &pal()), plain(&px, &pal()));
    }

    #[test]
    fn one_pixel_alternating_lines_are_blended_but_short_ones_are_kept() {
        let mut px = vec![1u8; W * H];
        for x in 0..6 {
            px[10 * W + x] = if x % 2 == 0 { 2 } else { 1 };
        }
        for y in 0..6 {
            px[(20 + y) * W + 30] = if y % 2 == 0 { 2 } else { 1 };
        }
        for x in 0..3 {
            px[40 * W + x] = if x % 2 == 0 { 2 } else { 3 };
        }
        let out = dedither(&px, &pal());
        assert_eq!(out[10 * W], out[10 * W + 1]);
        assert_ne!(out[10 * W], pack(pal()[2]));
        assert_eq!(out[20 * W + 30], out[21 * W + 30]);
        assert_eq!(out[40 * W..40 * W + 3], plain(&px, &pal())[40 * W..40 * W + 3]);
    }

    #[test]
    fn png_has_signature_and_dimensions() {
        let out = png(2, 1, &[0xFF0000, 0x00FF00]);
        assert_eq!(&out[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(&out[16..24], [0, 0, 0, 2, 0, 0, 0, 1]);
    }

    #[test]
    fn letterbox_is_centered_and_extreme_sizes_work() {
        let rgb = vec![pack([9, 9, 9]); W * H];
        let (w, h) = (1000, 480);
        let mut dst = vec![1; w * h];
        scale(&rgb, &mut dst, w, h);
        assert_eq!(dst[0], 0);
        assert_eq!(dst[w / 2], pack([9, 9, 9]));
        for (w, h) in [(1, 1), (7680, 4320)] {
            scale(&rgb, &mut vec![0; w * h], w, h);
        }
    }

    fn halves() -> Vec<u32> {
        (0..W * H)
            .map(|i| if i % W < W / 2 { 0x000000 } else { 0xFFFFFF })
            .collect()
    }

    #[test]
    fn integer_factor_is_exact_replication() {
        let src = halves();
        let (w, h) = (W * 2, H * 2);
        let mut dst = vec![1; w * h];
        scale(&src, &mut dst, w, h);
        assert!((0..h).all(|y| (0..w).all(|x| dst[y * w + x] == src[y / 2 * W + x / 2])));
    }

    #[test]
    fn fractional_factor_blends_only_at_edges() {
        let (w, h) = (1440, 1080); // 2.25x
        let mut dst = vec![1; w * h];
        scale(&halves(), &mut dst, w, h);
        let row = &dst[540 * w..][..w];
        assert_eq!(row[100], 0x000000);
        assert_eq!(row[w - 100], 0xFFFFFF);
        assert!(row.iter().filter(|&&c| c != 0 && c != 0xFFFFFF).count() <= 2);
    }
}
