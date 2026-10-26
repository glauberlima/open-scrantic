//! Dynamix resource archive (RESOURCE.MAP + RESOURCE.001) and the resource formats it holds.

use std::collections::HashMap;

use super::{AssetError, Result, bytes, u16_at, u32_at};

/// Most instructions one TTM may hold; the installed ones have at most 2368.
const MAX_TTM_OPS: usize = 100_000;

/// Most pixels one BMP may hold across its sprites; the installed ones have at most 277,048.
const MAX_BMP_PIXELS: usize = 2 << 20;

/// Largest decompressed payload accepted; the real ones are a few hundred KB.
const MAX_UNPACKED: usize = 16 << 20;

pub struct Archive {
    /// The whole RESOURCE.001, kept once: entries may overlap, so they are ranges into it, never copies.
    volume: Vec<u8>,
    /// Upper-case name -> (start, len) of the entry's data in `volume`.
    files: HashMap<String, (usize, usize)>,
}

#[cfg(test)]
impl Archive {
    /// Bytes of resource data this archive owns.
    fn retained_bytes(&self) -> usize {
        self.volume.len()
    }
}

impl Archive {
    pub fn new(map: &[u8], volume: &[u8]) -> Result<Self> {
        if map.len() < 21 {
            return Err(AssetError::Format("RESOURCE.MAP too small".into()));
        }
        let count = u16_at(map, 19)? as usize;
        let mut files = HashMap::with_capacity(count);
        for i in 0..count {
            let entry = 21 + i * 8;
            let offset = u32_at(map, entry + 4)? as usize;
            let header =
                bytes(volume, offset, 17).map_err(|_| AssetError::Format("RESOURCE.MAP entry out of range".into()))?;
            let name_end = header[..13].iter().position(|&b| b == 0).unwrap_or(13);
            let name = String::from_utf8_lossy(&header[..name_end]).to_ascii_uppercase();
            let size = u32_at(header, 13)? as usize;
            bytes(volume, offset + 17, size).map_err(|_| AssetError::Format(format!("{name}: data out of range")))?;
            files.insert(name, (offset + 17, size));
        }
        Ok(Self {
            volume: volume.to_vec(),
            files,
        })
    }

    /// The data of the entry called `name` (case-insensitive).
    fn data(&self, name: &str) -> Option<&[u8]> {
        let &(start, len) = self.files.get(&name.to_ascii_uppercase())?;
        self.volume.get(start..start + len)
    }

    fn chunks(&self, name: &str) -> Result<HashMap<String, &[u8]>> {
        let data = self.data(name).ok_or_else(|| AssetError::Missing(name.to_string()))?;
        let mut out = HashMap::new();
        let mut p = 0;
        while p + 8 <= data.len() {
            let tag = String::from_utf8_lossy(&data[p..p + 4]).into_owned();
            let size = u32_at(data, p + 4)?;
            p += 8;
            if size & 0x8000_0000 != 0 {
                continue; // container chunk: its children follow inline
            }
            let size = size as usize;
            let body = bytes(data, p, size).map_err(|_| AssetError::Format(format!("{name}: bad chunk {tag}")))?;
            out.insert(tag, body);
            p += size;
        }
        Ok(out)
    }

    fn chunk<'a>(chunks: &HashMap<String, &'a [u8]>, name: &str, tag: &str) -> Result<&'a [u8]> {
        chunks
            .get(tag)
            .copied()
            .ok_or_else(|| AssetError::Format(format!("{name}: missing {tag} chunk")))
    }

    pub fn bmp(&self, name: &str) -> Result<Bmp> {
        let c = self.chunks(name)?;
        let inf = Self::chunk(&c, name, "INF:")?;
        let count = u16_at(inf, 0)? as usize;
        let mut total = 0usize;
        for i in 0..count {
            let w = u16_at(inf, 2 + i * 2)? as usize;
            let h = u16_at(inf, 2 + count * 2 + i * 2)? as usize;
            total += w * h;
            if total > MAX_BMP_PIXELS {
                return Err(AssetError::Format(format!("{name}: image too large")));
            }
        }
        let pixels = decompress(Self::chunk(&c, name, "BIN:")?)?;
        let mut sprites = Vec::with_capacity(count);
        let mut nibble = 0usize;
        for i in 0..count {
            let w = u16_at(inf, 2 + i * 2)? as usize;
            let h = u16_at(inf, 2 + count * 2 + i * 2)? as usize;
            let px = unpack_4bpp(&pixels, nibble, w * h, name)?;
            nibble += w * h;
            sprites.push(Sprite {
                w: w as i32,
                h: h as i32,
                px,
            });
        }
        Ok(Bmp { sprites })
    }

    pub fn scr(&self, name: &str) -> Result<Picture> {
        let c = self.chunks(name)?;
        let dim = Self::chunk(&c, name, "DIM:")?;
        let (w, h) = (u16_at(dim, 0)? as usize, u16_at(dim, 2)? as usize);
        let pixels = decompress(Self::chunk(&c, name, "BIN:")?)?;
        Ok(Picture {
            w,
            h,
            px: unpack_4bpp(&pixels, 0, w * h, name)?,
        })
    }

    pub fn palette(&self, name: &str) -> Result<[[u8; 3]; 256]> {
        let c = self.chunks(name)?;
        let vga = Self::chunk(&c, name, "VGA:")?;
        let mut pal = [[0u8; 3]; 256];
        for (i, rgb) in pal.iter_mut().enumerate() {
            for (k, v) in rgb.iter_mut().enumerate() {
                let six = vga.get(i * 3 + k).copied().unwrap_or(0) & 63;
                *v = (six << 2) | (six >> 4);
            }
        }
        Ok(pal)
    }

    pub fn ttm(&self, name: &str) -> Result<Ttm> {
        let c = self.chunks(name)?;
        let pages = u16_at(Self::chunk(&c, name, "PAG:")?, 0)? as usize;
        let code = decompress(Self::chunk(&c, name, "TT3:")?)?;
        Ttm::parse(&code, pages)
    }

    pub fn ads(&self, name: &str) -> Result<Ads> {
        let c = self.chunks(name)?;
        let res = Self::chunk(&c, name, "RES:")?;
        let mut ttms = Vec::new();
        let mut p = 2;
        for _ in 0..u16_at(res, 0)? {
            let id = u16_at(res, p)?;
            let text = res
                .get(p + 2..)
                .ok_or_else(|| AssetError::Format(format!("{name}: truncated RES chunk")))?;
            let len = text.iter().position(|&b| b == 0).unwrap_or(text.len());
            ttms.push((id, String::from_utf8_lossy(&text[..len]).into_owned()));
            p += 2 + len + 1;
        }
        let script = decompress(Self::chunk(&c, name, "SCR:")?)?;
        Ok(Ads { ttms, script })
    }
}

/// Unpacks `count` nibbles; reads past the end of `packed` give 0, but a size far beyond the data is
/// rejected so that a corrupt header cannot trigger a huge allocation.
fn unpack_4bpp(packed: &[u8], first_nibble: usize, count: usize, name: &str) -> Result<Vec<u8>> {
    if first_nibble.saturating_add(count) > MAX_UNPACKED * 2 {
        return Err(AssetError::Format(format!("{name}: image too large")));
    }
    Ok((first_nibble..first_nibble + count)
        .map(|n| {
            let b = packed.get(n / 2).copied().unwrap_or(0);
            if n % 2 == 0 { b >> 4 } else { b & 15 }
        })
        .collect())
}

/// A sprite: 8-bit palette indices, index 0 is transparent.
pub struct Sprite {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
}

pub struct Bmp {
    pub sprites: Vec<Sprite>,
}

pub struct Picture {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

/// One TTM instruction: opcode, word arguments and optional string argument.
#[derive(Clone, Debug)]
pub struct Op {
    pub code: u16,
    pub args: [i16; 6],
    pub text: Option<String>,
}

impl Op {
    pub fn arg(&self, i: usize) -> i32 {
        self.args[i] as i32
    }
}

pub struct Ttm {
    /// Frames ("pages"), each terminated by an UPDATE (0x0FF0) instruction.
    pub frames: Vec<Vec<Op>>,
    /// Scene tags (0x1111) with the frame they start in, in file order.
    pub scenes: Vec<(u16, usize)>,
}

impl Ttm {
    fn parse(code: &[u8], pages: usize) -> Result<Self> {
        let mut frames = vec![Vec::new()];
        let mut scenes = Vec::new();
        let mut op_count = 0;
        let mut p = 0;
        while p + 2 <= code.len() {
            let opcode = u16_at(code, p)?;
            p += 2;
            let nargs = (opcode & 15) as usize;
            let mut op = Op {
                code: opcode,
                args: [0; 6],
                text: None,
            };
            if nargs == 15 {
                let end = code[p..]
                    .iter()
                    .position(|&b| b == 0)
                    .map(|e| p + e)
                    .unwrap_or(code.len());
                op.text = Some(String::from_utf8_lossy(&code[p..end]).into_owned());
                let len = end - p;
                p = end + 1 + usize::from(len % 2 == 0); // NUL-terminated, padded to an even size
            } else {
                if nargs > 6 || p + nargs * 2 > code.len() {
                    return Err(AssetError::Format(format!("bad TTM opcode {opcode:04x}")));
                }
                for i in 0..nargs {
                    op.args[i] = u16_at(code, p + i * 2)? as i16;
                }
                p += nargs * 2;
            }
            op_count += 1;
            if op_count > MAX_TTM_OPS {
                return Err(AssetError::Format("TTM has too many instructions".into()));
            }
            if opcode == 0x1111 {
                scenes.push((op.args[0] as u16, frames.len() - 1));
            }
            frames.last_mut().unwrap().push(op);
            if opcode == 0x0FF0 {
                frames.push(Vec::new());
            }
        }
        if frames.last().is_some_and(Vec::is_empty) {
            frames.pop();
        }
        frames.truncate(pages);
        Ok(Self { frames, scenes })
    }

    /// Frame whose first instruction is the given (local) tag, as the engine's GOTO resolves it.
    pub fn frame_of_tag(&self, tag: i32) -> Option<usize> {
        self.frames.iter().position(|f| {
            f.first()
                .is_some_and(|op| matches!(op.code, 0x1101 | 0x1111) && op.arg(0) == tag)
        })
    }
}

pub struct Ads {
    pub ttms: Vec<(u16, String)>,
    pub script: Vec<u8>,
}

/// Decompresses a BIN/SCR/TT3 payload: method byte, u32 size, data.
pub fn decompress(chunk: &[u8]) -> Result<Vec<u8>> {
    if chunk.len() < 5 {
        return Err(AssetError::Format("compressed chunk too small".into()));
    }
    let size = u32_at(chunk, 1)? as usize;
    if size > MAX_UNPACKED {
        return Err(AssetError::Format("decompressed size too large".into()));
    }
    let out = match chunk[0] {
        1 => rle(&chunk[5..], size),
        2 => lzw(&chunk[5..], size),
        m => return Err(AssetError::Format(format!("unknown compression method {m}"))),
    };
    if out.len() != size {
        return Err(AssetError::Format("decompressed size mismatch".into()));
    }
    Ok(out)
}

fn rle(src: &[u8], size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(size);
    let mut p = 0;
    while out.len() < size && p < src.len() {
        let c = src[p] as usize;
        p += 1;
        if c & 0x80 != 0 {
            let b = src.get(p).copied().unwrap_or(0);
            p += 1;
            out.extend(std::iter::repeat_n(b, c & 0x7F));
        } else {
            let end = (p + c).min(src.len());
            out.extend_from_slice(&src[p..end]);
            p = end;
        }
    }
    out.truncate(size);
    out
}

/// Dynamix LZW: 9..12-bit LSB-first codes, 0x100 resets the dictionary and realigns
/// the bit stream to a multiple of 8 codes.
fn lzw(src: &[u8], size: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(size);
    let (mut pos, mut buf, mut cnt) = (0usize, 0u32, 0u32);
    let mut get = |n: u32| -> Option<u32> {
        while cnt < n {
            buf |= (*src.get(pos)? as u32) << cnt;
            pos += 1;
            cnt += 8;
        }
        let v = buf & ((1 << n) - 1);
        buf >>= n;
        cnt -= n;
        Some(v)
    };
    let mut prefix = vec![0u16; 4096];
    let mut suffix = vec![0u8; 4096];
    let (mut nbits, mut free, mut bitpos) = (9u32, 257usize, 0u32);
    let Some(first) = get(nbits) else { return out };
    if first > 255 {
        // The first code must be a literal. A dictionary code here leaves slot `first` pointing at itself
        // once the dictionary grows to it, and decoding that code would never end; every installed stream
        // starts with a literal.
        return out;
    }
    let (mut old, mut last) = (first as usize, first as u8);
    out.push(last);
    let mut stack = Vec::with_capacity(4096);
    while out.len() < size {
        let Some(new) = get(nbits) else { break };
        let new = new as usize;
        if new > free {
            break; // invalid code: the decompressed size check then reports the corruption
        }
        bitpos += nbits;
        if new == 256 {
            let span = nbits << 3;
            let mut skip = (span - ((bitpos - 1) % span)) - 1;
            while skip > 0 {
                let n = skip.min(16);
                if get(n).is_none() {
                    break;
                }
                skip -= n;
            }
            nbits = 9;
            free = 256;
            bitpos = 0;
            continue;
        }
        let mut code = new;
        stack.clear();
        if code >= free {
            stack.push(last);
            code = old;
        }
        while code > 255 {
            if stack.len() > 4096 {
                return out; // a chain longer than the dictionary is a cycle: corrupt stream
            }
            stack.push(suffix[code]);
            code = prefix[code] as usize;
        }
        stack.push(code as u8);
        last = code as u8;
        out.extend(stack.iter().rev());
        if free < 4096 {
            prefix[free] = old as u16;
            suffix[free] = last;
            free += 1;
            if free >= (1 << nbits) && nbits < 12 {
                nbits += 1;
                bitpos = 0;
            }
        }
        old = new;
    }
    out.truncate(size);
    out
}

/// Builders for synthetic archives, shared by the tests of this crate.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::Archive;

    pub fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = tag.to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    /// Method 1 (RLE) payload holding `data` as literal runs.
    pub fn rle(data: &[u8]) -> Vec<u8> {
        let mut v = vec![1u8];
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        for c in data.chunks(0x7F) {
            v.push(c.len() as u8);
            v.extend_from_slice(c);
        }
        v
    }

    /// An archive of `(name, body)` entries laid out one after the other.
    pub fn archive_of(entries: &[(&str, Vec<u8>)]) -> Archive {
        let (mut volume, mut map) = (Vec::new(), vec![0u8; 21]);
        map[19..21].copy_from_slice(&(entries.len() as u16).to_le_bytes());
        for (name, body) in entries {
            map.extend_from_slice(&[0; 4]);
            map.extend_from_slice(&(volume.len() as u32).to_le_bytes());
            let mut header = [0u8; 13];
            header[..name.len()].copy_from_slice(name.as_bytes());
            volume.extend_from_slice(&header);
            volume.extend_from_slice(&(body.len() as u32).to_le_bytes());
            volume.extend_from_slice(body);
        }
        Archive::new(&map, &volume).unwrap()
    }

    /// TTM entry body: `pages` pages and the given 2-byte (no argument) opcodes.
    pub fn ttm_body(pages: u16, ops: &[u16]) -> Vec<u8> {
        let code: Vec<u8> = ops.iter().flat_map(|op| op.to_le_bytes()).collect();
        let mut body = chunk(b"PAG:", &pages.to_le_bytes());
        body.extend(chunk(b"TT3:", &rle(&code)));
        body
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{self, *};
    use super::*;

    /// A volume of `vol` bytes with a header every 17 bytes whose size runs to the end of the volume,
    /// so every entry overlaps the data of the ones before it, and the map that lists them all.
    fn overlapping(vol: usize) -> (Vec<u8>, Vec<u8>) {
        let n = vol / 17 - 1;
        let mut volume = vec![0x41u8; vol];
        let mut map = vec![0u8; 21 + n * 8];
        map[19..21].copy_from_slice(&(n as u16).to_le_bytes());
        for i in 0..n {
            let off = i * 17;
            let name = format!("F{i:05}.TTM");
            volume[off..off + 13].fill(0);
            volume[off..off + name.len()].copy_from_slice(name.as_bytes());
            let size = (vol - off - 17) as u32;
            volume[off + 13..off + 17].copy_from_slice(&size.to_le_bytes());
            map[21 + i * 8 + 4..21 + i * 8 + 8].copy_from_slice(&(off as u32).to_le_bytes());
        }
        (map, volume)
    }

    #[test]
    fn overlapping_entries_do_not_multiply_the_volume() {
        let vol = 64 << 10;
        let (map, volume) = overlapping(vol);
        let archive = Archive::new(&map, &volume).expect("overlapping entries are structurally valid");
        assert!(
            archive.retained_bytes() <= vol,
            "retained {} bytes",
            archive.retained_bytes()
        );
    }

    #[test]
    fn entries_are_found_by_name() {
        let mut volume = Vec::new();
        for (name, body) in [("A.TTM", b"abc".as_slice()), ("B.TTM", b"")] {
            let mut h = [0u8; 13];
            h[..name.len()].copy_from_slice(name.as_bytes());
            volume.extend_from_slice(&h);
            volume.extend_from_slice(&(body.len() as u32).to_le_bytes());
            volume.extend_from_slice(body);
        }
        let mut map = vec![0u8; 21];
        map[19] = 2;
        for off in [0u32, 17 + 3] {
            map.extend_from_slice(&[0; 4]);
            map.extend_from_slice(&off.to_le_bytes());
        }
        let archive = Archive::new(&map, &volume).unwrap();
        assert_eq!(archive.data("a.ttm").unwrap(), b"abc");
        assert_eq!(archive.data("B.TTM").unwrap(), b"");
    }

    /// LSB-first packer for the 9-bit codes of a fresh LZW stream (method byte 2 + size header).
    fn lzw_chunk(codes: &[u16], size: u32) -> Vec<u8> {
        let (mut out, mut buf, mut cnt) = (vec![2u8], 0u32, 0u32);
        out.extend_from_slice(&size.to_le_bytes());
        for &c in codes {
            buf |= (c as u32) << cnt;
            cnt += 9;
            while cnt >= 8 {
                out.push(buf as u8);
                buf >>= 8;
                cnt -= 8;
            }
        }
        if cnt > 0 {
            out.push(buf as u8);
        }
        out
    }

    #[test]
    fn lzw_dictionary_cycle_from_a_non_literal_first_code_is_an_error() {
        // The first code is taken as a literal. 300 is not one: its slot is written later with prefix 257 while
        // slot 257 was written with prefix 300, so decoding code 300 walks 300 -> 257 -> 300 forever.
        let mut codes = vec![300u16];
        codes.extend([65u16; 42]); // slots 257..=298
        codes.extend([257, 65, 300]);
        let chunk = lzw_chunk(&codes, 1 << 16);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(decompress(&chunk).is_err());
        });
        let result = rx.recv_timeout(std::time::Duration::from_secs(1));
        assert_eq!(result, Ok(true), "decompress must fail promptly");
    }

    #[test]
    fn lzw_random_streams_terminate() {
        let mut x = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        for round in 0..3000 {
            let src: Vec<u8> = (0..next() % 600)
                .map(|_| {
                    if round % 2 == 0 && next() % 4 == 0 {
                        0
                    } else {
                        next() as u8
                    }
                })
                .collect();
            let _ = lzw(&src, 1 << 16);
        }
    }

    #[test]
    fn installed_lzw_streams_start_with_a_literal_and_decode() {
        let Some(dir) = std::env::var_os("OPEN_SCRANTIC_DATA") else {
            return;
        };
        let data = crate::assets::ScreensaverData::load(std::path::Path::new(&dir)).expect("load");
        let mut streams = 0;
        for name in data.archive.files.keys() {
            let Ok(chunks) = data.archive.chunks(name) else {
                continue;
            }; // not a chunked resource
            for (tag, body) in chunks {
                if matches!(tag.as_str(), "BIN:" | "SCR:" | "TT3:" | "TTI:" | "ADS:") && body.first() == Some(&2) {
                    let first = body[5] as u16 | (body[6] as u16 & 1) << 8;
                    assert!(first < 256, "{name} {tag}: first code {first}");
                    decompress(body).unwrap_or_else(|e| panic!("{name} {tag}: {e}"));
                    streams += 1;
                }
            }
        }
        assert!(streams > 0);
    }

    #[test]
    fn a_ttm_with_too_many_instructions_is_rejected() {
        let ops = vec![0x0110u16; MAX_TTM_OPS + 1];
        let archive = archive_of(&[("T.TTM", ttm_body(1, &ops))]);
        assert!(archive.ttm("T.TTM").is_err());
        let ok = archive_of(&[("T.TTM", ttm_body(1, &ops[..MAX_TTM_OPS]))]);
        assert!(ok.ttm("T.TTM").is_ok());
    }

    #[test]
    fn a_bitmap_with_too_many_pixels_is_rejected() {
        // One 2000 x 2000 sprite; the pixel data is short and reads as zeros.
        let mut inf = 1u16.to_le_bytes().to_vec();
        inf.extend_from_slice(&2000u16.to_le_bytes());
        inf.extend_from_slice(&2000u16.to_le_bytes());
        let mut body = chunk(b"INF:", &inf);
        body.extend(chunk(b"BIN:", &fixtures::rle(&[0; 16])));
        let archive = archive_of(&[("B.BMP", body)]);
        assert!(archive.bmp("B.BMP").is_err());
    }
}
