//! Data read from the original SCRANTIC.SCR (Win16 NE executable): sound effects and the
//! logic tables (scene catalog, walk routes, RNG seed table, whitecap animations).
//! Nothing from the original is embedded in this program; offsets below were found by
//! disassembling the 1992 build and are validated against known strings at load time.

use std::collections::HashMap;

use super::{AssetError, Result, bytes, u16_at};

/// One entry of the scene catalog (DS:0x1756, 17 bytes per record).
#[derive(Clone, Copy, Debug)]
pub struct SceneDef {
    pub weight: u8,
    pub spot_start: u8,
    pub hdg_start: u8,
    pub spot_end: u8,
    pub hdg_end: u8,
    pub duration: u8,
    pub min_phase: u8,
    pub max_phase: u8,
    pub day: u8,
    pub flags: u32,
    pub ads_id: u8,
    pub ads_tag: u8,
}

/// A walk frame: JOHNWALK sprite index (+ mirror flag) and island-relative position.
#[derive(Clone, Copy, Debug)]
pub struct WalkFrame {
    pub sprite: u16,
    pub mirror: bool,
    pub x: i32,
    pub y: i32,
}

pub struct ExeData {
    pub sounds: HashMap<u32, Vec<f32>>,
    pub ads_names: HashMap<u8, String>,
    pub scenes: Vec<SceneDef>,
    /// path id (1..=34) -> (start heading, end heading, end spot)
    pub path_info: Vec<[u8; 3]>,
    /// path id (1..=34) -> frames, terminated where the original has (0,0,0)
    pub paths: Vec<Vec<WalkFrame>>,
    /// spot (1..=6) -> path id of the standing-turn table / walking-turn table
    pub stand_turns: [u8; 7],
    pub walk_turns: [u8; 7],
    /// (from, to) -> route list: [-spot, (path, weight)..., ..., -7]
    pub routes: HashMap<(u8, u8), Vec<i16>>,
    pub rng_table: [u16; 56],
    pub rng_j: usize,
    pub rng_k: usize,
    /// Whitecap frame sequences (BACKGRND.BMP indices), kinds 3, 2 and 1.
    pub whitecap_seqs: [Vec<u8>; 3],
}

const DATA_SEGMENT: usize = 14;
const WALK_SEGMENT: usize = 4;
const PATH_JUMP_TABLE: usize = 0x04FB;

/// Largest total of WAVE bytes decoded; the genuine sounds are far below this.
const MAX_SOUND_BYTES: usize = 16 << 20;

impl ExeData {
    pub fn parse(exe: &[u8]) -> Result<Self> {
        let ne = NeFile::parse(exe)?;
        let ds = ne.segment(DATA_SEGMENT)?;
        if ds.get(0x1681..0x168D) != Some(b"ACTIVITY.ADS".as_slice())
            || ds.get(0x1D56..0x1D5F) != Some(b"WAVESFX%d".as_slice())
        {
            return Err(AssetError::Format(
                "unsupported SCRANTIC.SCR build (expected the 1992 release)".into(),
            ));
        }

        let mut ads_names = HashMap::new();
        let mut p = 0x166E;
        while u16_at(ds, p)? != 0 {
            let id = u16_at(ds, p)?;
            let raw = bytes(ds, p + 2, 15)?;
            let name = String::from_utf8_lossy(&raw[..raw.iter().position(|&b| b == 0).unwrap_or(15)]).into_owned();
            if id < 256 {
                ads_names.insert(id as u8, name);
            }
            p += 17;
        }

        let mut scenes = Vec::new();
        let mut p = 0x1756;
        while u16_at(ds, p)? != 0 {
            let r = bytes(ds, p, 17)?;
            scenes.push(SceneDef {
                weight: r[2],
                spot_start: r[3],
                hdg_start: r[4],
                spot_end: r[5],
                hdg_end: r[6],
                duration: r[7],
                min_phase: r[8],
                max_phase: r[9],
                day: r[10],
                flags: u32::from_le_bytes([r[11], r[12], r[13], r[14]]),
                ads_id: r[15],
                ads_tag: r[16],
            });
            p += 17;
        }

        let path_info = bytes(ds, 0x2DE, 35 * 3)?.as_chunks::<3>().0.to_vec();
        let mut stand_turns = [0u8; 7];
        let mut walk_turns = [0u8; 7];
        stand_turns.copy_from_slice(bytes(ds, 0x347, 7)?);
        walk_turns.copy_from_slice(bytes(ds, 0x34E, 7)?);

        // Path frame pointers are immediates in the code of seg4's path lookup switch.
        let walk_code = ne.segment(WALK_SEGMENT)?;
        let mut paths = vec![Vec::new()];
        for i in 0..34 {
            let handler = u16_at(walk_code, PATH_JUMP_TABLE + i * 2)? as usize;
            if walk_code.get(handler) != Some(&0xB8) {
                return Err(AssetError::Format("unexpected walk path table layout".into()));
            }
            let mut q = u16_at(walk_code, handler + 1)? as usize;
            let mut frames = Vec::new();
            loop {
                let (w0, x, y) = (u16_at(ds, q)?, u16_at(ds, q + 2)? as i16, u16_at(ds, q + 4)? as i16);
                if w0 == 0 && x == 0 && y == 0 {
                    break;
                }
                frames.push(WalkFrame {
                    sprite: w0 & 0x7FFF,
                    mirror: w0 & 0x8000 != 0,
                    x: x as i32,
                    y: y as i32,
                });
                q += 6;
            }
            paths.push(frames);
        }

        let mut routes = HashMap::new();
        for from in 1..=6u8 {
            for to in 1..=6u8 {
                let ptr = u16_at(ds, 0x0A96 - 2 + from as usize * 12 + to as usize * 2)? as usize;
                if ptr == 0 {
                    continue;
                }
                let mut list = Vec::new();
                let mut q = ptr;
                loop {
                    let w = u16_at(ds, q)? as i16;
                    list.push(w);
                    q += 2;
                    if w == -7 {
                        break;
                    }
                }
                routes.insert((from, to), list);
            }
        }

        let mut rng_table = [0u16; 56];
        for (i, v) in rng_table.iter_mut().enumerate() {
            *v = u16_at(ds, 0x1CE6 + i * 2)?;
        }
        let seq =
            |o: usize| -> Result<Vec<u8>> { Ok(bytes(ds, o, 8)?.iter().copied().take_while(|&b| b != 0xFF).collect()) };

        Ok(Self {
            sounds: ne.wave_resources()?,
            ads_names,
            scenes,
            path_info,
            paths,
            stand_turns,
            walk_turns,
            routes,
            rng_table,
            rng_j: u16_at(ds, 0x1CE2)? as usize,
            rng_k: u16_at(ds, 0x1CE4)? as usize,
            whitecap_seqs: [seq(0x268)?, seq(0x270)?, seq(0x278)?],
        })
    }
}

struct NeFile<'a> {
    data: &'a [u8],
    ne: usize,
}

impl<'a> NeFile<'a> {
    fn parse(data: &'a [u8]) -> Result<Self> {
        if data.len() < 0x40 || &data[..2] != b"MZ" {
            return Err(AssetError::Format("SCRANTIC is not an executable".into()));
        }
        let ne = u16_at(data, 0x3C)? as usize;
        if data.get(ne..ne + 2) != Some(b"NE".as_slice()) {
            return Err(AssetError::Format("SCRANTIC is not a Win16 NE executable".into()));
        }
        Ok(Self { data, ne })
    }

    fn segment(&self, index: usize) -> Result<&'a [u8]> {
        let d = self.data;
        let count = u16_at(d, self.ne + 0x1C)? as usize;
        let table = self.ne + u16_at(d, self.ne + 0x22)? as usize;
        let shift = u16_at(d, self.ne + 0x32)?;
        if index == 0 || index > count || shift > 15 {
            return Err(AssetError::Format("missing NE segment".into()));
        }
        let e = table + (index - 1) * 8;
        let offset = (u16_at(d, e)? as usize) << shift;
        let len = match u16_at(d, e + 2)? {
            0 => 0x10000,
            n => n as usize,
        };
        bytes(d, offset, len).map_err(|_| AssetError::Format("truncated NE segment".into()))
    }

    /// Extracts the WAVESFXn resources (type "WAVE") as normalized mono samples at 11025 Hz.
    fn wave_resources(&self) -> Result<HashMap<u32, Vec<f32>>> {
        let d = self.data;
        let table = self.ne + u16_at(d, self.ne + 0x24)? as usize;
        let shift = u16_at(d, table)?;
        if shift > 15 {
            return Err(AssetError::Format("bad NE resource table".into()));
        }
        let mut resources = HashMap::new(); // (type id, res id) -> bytes
        let mut p = table + 2;
        loop {
            let type_id = u16_at(d, p)?;
            if type_id == 0 {
                break;
            }
            let count = u16_at(d, p + 2)? as usize;
            for i in 0..count {
                let e = p + 8 + i * 12;
                let offset = (u16_at(d, e)? as usize) << shift;
                let len = (u16_at(d, e + 2)? as usize) << shift;
                if let Ok(res) = bytes(d, offset, len) {
                    resources.insert((type_id, u16_at(d, e + 6)?), res);
                }
            }
            p += 8 + count * 12;
        }
        // RT_NAMETABLE (0x800F) maps the names used by the code to integer resource ids.
        let mut sounds = HashMap::new();
        let mut budget = MAX_SOUND_BYTES;
        if let Some(names) = resources.get(&(0x800F, 0x8001)) {
            let mut q = 0;
            while q + 6 < names.len() && u16_at(names, q)? != 0 {
                let len = u16_at(names, q)? as usize;
                let (type_id, res_id) = (u16_at(names, q + 2)?, u16_at(names, q + 4)?);
                let body = names
                    .get(q + 6..q + len)
                    .ok_or_else(|| AssetError::Format("bad NE name table".into()))?;
                let strings: Vec<&[u8]> = body.split(|&b| b == 0).collect();
                let name = strings
                    .get(1)
                    .map(|s| String::from_utf8_lossy(s).into_owned())
                    .unwrap_or_default();
                if let Some(n) = name.strip_prefix("WAVESFX").and_then(|n| n.parse::<u32>().ok())
                    && let Some(wav) = resources.get(&(type_id, res_id))
                {
                    // Several names may alias one resource; bound what they decode in total.
                    budget = budget
                        .checked_sub(wav.len())
                        .ok_or_else(|| AssetError::Format("NE sound resources too large".into()))?;
                    sounds.insert(n, decode_wav(wav)?);
                }
                q += len;
            }
        }
        Ok(sounds)
    }
}

/// 8-bit unsigned PCM RIFF/WAVE to f32.
fn decode_wav(wav: &[u8]) -> Result<Vec<f32>> {
    if wav.len() < 12 || &wav[..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err(AssetError::Format("bad WAVE resource".into()));
    }
    let mut p = 12;
    while p + 8 <= wav.len() {
        let size = u32::from_le_bytes([wav[p + 4], wav[p + 5], wav[p + 6], wav[p + 7]]) as usize;
        if &wav[p..p + 4] == b"data" {
            let end = (p + 8 + size).min(wav.len());
            return Ok(wav[p + 8..end].iter().map(|&s| (s as f32 - 128.0) / 128.0).collect());
        }
        p += 8 + size + (size & 1);
    }
    Err(AssetError::Format("WAVE resource without data".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put16(v: &mut [u8], o: usize, x: u16) {
        v[o..o + 2].copy_from_slice(&x.to_le_bytes());
    }

    fn align(v: &mut Vec<u8>) -> usize {
        while !v.len().is_multiple_of(16) {
            v.push(0);
        }
        v.len()
    }

    /// Minimal synthetic NE with one WAVE resource of `wav_len` bytes that `n` name-table entries
    /// (`WAVESFX1..=n`) all point at.
    fn aliasing_exe(n: usize, wav_len: usize) -> Vec<u8> {
        let mut f = vec![0u8; 0x200];
        f[..2].copy_from_slice(b"MZ");
        put16(&mut f, 0x3C, 0x40);
        f[0x40..0x42].copy_from_slice(b"NE");
        put16(&mut f, 0x40 + 0x1C, 14); // segment count
        put16(&mut f, 0x40 + 0x22, 0x40); // segment table
        put16(&mut f, 0x40 + 0x24, 0x40 + 14 * 8); // resource table
        put16(&mut f, 0x40 + 0x32, 4); // segment shift
        let seg = |i: usize| 0x80 + (i - 1) * 8;
        let rt = 0x40 + 0x40 + 14 * 8;
        // data segment (14)
        let ds_off = align(&mut f);
        let mut ds = vec![0u8; 0x2000];
        ds[0x1681..0x168D].copy_from_slice(b"ACTIVITY.ADS");
        ds[0x1D56..0x1D5F].copy_from_slice(b"WAVESFX%d");
        f.extend_from_slice(&ds);
        put16(&mut f, seg(14), (ds_off >> 4) as u16);
        put16(&mut f, seg(14) + 2, 0x2000);
        // walk code (4): every path handler is `mov ax, 0x1E00`
        let wc_off = align(&mut f);
        let mut wc = vec![0u8; 0x600];
        for i in 0..34 {
            wc[0x4FB + i * 2..0x4FB + i * 2 + 2].copy_from_slice(&0x580u16.to_le_bytes());
        }
        wc[0x580] = 0xB8;
        wc[0x581..0x583].copy_from_slice(&0x1E00u16.to_le_bytes());
        f.extend_from_slice(&wc);
        put16(&mut f, seg(4), (wc_off >> 4) as u16);
        put16(&mut f, seg(4) + 2, 0x600);
        // the WAVE resource
        let wav_off = align(&mut f);
        let mut wav = Vec::from(*b"RIFF\0\0\0\0WAVEdata");
        wav.extend_from_slice(&((wav_len - 20) as u32).to_le_bytes());
        wav.resize(wav_len, 0x80);
        f.extend_from_slice(&wav);
        // RT_NAMETABLE: n names, all -> (0x8011, 0x8002)
        let nt_off = align(&mut f);
        let mut nt = Vec::new();
        for k in 1..=n {
            let name = format!("\0WAVESFX{k}");
            nt.extend_from_slice(&((6 + name.len()) as u16).to_le_bytes());
            nt.extend_from_slice(&0x8011u16.to_le_bytes());
            nt.extend_from_slice(&0x8002u16.to_le_bytes());
            nt.extend_from_slice(name.as_bytes());
        }
        nt.extend_from_slice(&[0u8; 8]);
        f.extend_from_slice(&nt);
        align(&mut f);
        // resource table
        let mut r = 4u16.to_le_bytes().to_vec();
        for (t, id, off, len) in [
            (0x800Fu16, 0x8001u16, nt_off, nt.len()),
            (0x8011, 0x8002, wav_off, wav_len),
        ] {
            r.extend_from_slice(&t.to_le_bytes());
            r.extend_from_slice(&1u16.to_le_bytes());
            r.extend_from_slice(&[0; 4]);
            r.extend_from_slice(&((off >> 4) as u16).to_le_bytes());
            r.extend_from_slice(&(len.div_ceil(16) as u16).to_le_bytes());
            r.extend_from_slice(&[0; 2]);
            r.extend_from_slice(&id.to_le_bytes());
            r.extend_from_slice(&[0; 4]);
        }
        r.extend_from_slice(&[0; 2]);
        f[rt..rt + r.len()].copy_from_slice(&r);
        f
    }

    #[test]
    fn aliased_sound_names_decode_within_budget() {
        let exe = aliasing_exe(2, 256 << 10);
        assert_eq!(ExeData::parse(&exe).expect("two aliases fit").sounds.len(), 2);
    }

    #[test]
    fn many_aliases_of_one_wave_are_rejected() {
        // 80 names x 256 KiB of WAVE exceed the 16 MiB budget while the file stays near 256 KiB.
        let exe = aliasing_exe(80, 256 << 10);
        assert!(ExeData::parse(&exe).is_err());
    }
}
