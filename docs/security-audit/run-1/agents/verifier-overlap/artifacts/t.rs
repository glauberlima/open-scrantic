// Overlapping-entry amplification: headers every 17 bytes inside one volume; each entry's size runs to end of volume.
use scrantic_harness::assets::resource::Archive;
fn hwm() -> u64 { let s = std::fs::read_to_string("/proc/self/status").unwrap();
  for l in s.lines() { if l.starts_with("VmHWM:") { return l.split_whitespace().nth(1).unwrap().parse().unwrap(); } } 0 }
fn rss() -> u64 { let s = std::fs::read_to_string("/proc/self/status").unwrap();
  for l in s.lines() { if l.starts_with("VmRSS:") { return l.split_whitespace().nth(1).unwrap().parse().unwrap(); } } 0 }
fn main() {
  let a: Vec<usize> = std::env::args().skip(1).map(|x| x.parse().unwrap()).collect();
  let (vol, n) = (a[0], a[1]);
  let mut v = vec![0x41u8; vol];
  let mut map = vec![0u8; 21 + n * 8];
  map[19..21].copy_from_slice(&(n as u16).to_le_bytes());
  for i in 0..n {
    let off = i * 17;
    let name = format!("F{i:05}.TTM");
    v[off..off + 13].fill(0);
    v[off..off + name.len()].copy_from_slice(name.as_bytes());
    let size = (vol - off - 17) as u32;
    v[off + 13..off + 17].copy_from_slice(&size.to_le_bytes());
    map[21 + i * 8 + 4..21 + i * 8 + 8].copy_from_slice(&(off as u32).to_le_bytes());
  }
  let base = rss();
  let t = std::time::Instant::now();
  let ar = Archive::new(&map, &v).unwrap();
  let after = rss();
  println!("vol={} B n={} map={} B  rss_before={} KiB rss_after={} KiB retained~{} KiB ({:.1}x vol) peak_hwm={} KiB t={:?}",
    vol, n, map.len(), base, after, after - base, (after - base) as f64 * 1024.0 / vol as f64, hwm(), t.elapsed());
  std::mem::forget(ar);
}
