use std::rc::Rc;
use scrantic_harness::assets::resource::{Archive, Bmp};
use scrantic_harness::engine::{ads::AdsRuntime, gfx::Canvas, rng::Rng, ttm::{run_frame, Scene, TtmCtx}, Env};
fn chunk(tag: &[u8; 4], body: &[u8]) -> Vec<u8> { let mut v = tag.to_vec(); v.extend_from_slice(&(body.len() as u32).to_le_bytes()); v.extend_from_slice(body); v }
fn rle(data: &[u8]) -> Vec<u8> { let mut v = vec![1u8]; v.extend_from_slice(&(data.len() as u32).to_le_bytes()); for c in data.chunks(0x7F) { v.push(c.len() as u8); v.extend_from_slice(c); } v }
fn entry(name: &[u8], body: &[u8], out: &mut Vec<u8>) { let mut h = [0u8; 13]; h[..name.len()].copy_from_slice(name); out.extend_from_slice(&h); out.extend_from_slice(&(body.len() as u32).to_le_bytes()); out.extend_from_slice(body); }
fn map(offs: &[u32]) -> Vec<u8> { let mut m = vec![0u8; 21]; m[19] = offs.len() as u8; for o in offs { m.extend_from_slice(&[0;4]); m.extend_from_slice(&o.to_le_bytes()); } m }
fn ttm_body(op: u16, text: &[u8]) -> Vec<u8> {
    let mut code = op.to_le_bytes().to_vec(); code.extend_from_slice(text); code.push(0); if text.len() % 2 == 0 { code.push(0); }
    let mut r = chunk(b"PAG:", &[1, 0]); r.extend(chunk(b"TT3:", &rle(&code))); r }
fn frame(a: &Archive) {
    let ttm = a.ttm("X.TTM").expect("ttm"); let mut ctx = TtmCtx::new(ttm); let mut scene = Scene::new(0, 0, 0, 0);
    let (mut w, mut b, mut f) = (Canvas::default(), Canvas::default(), Canvas::default());
    let jw = Rc::new(Bmp { sprites: vec![] }); let mut rng = Rng::new([0; 56], 24, 55); let mut s = Vec::new();
    let mut env = Env { work: &mut w, bg: &mut b, front: &mut f, archive: a, johnwalk: &jw, rng: &mut rng, sounds: &mut s, offset: (0, 0), now: 0 };
    run_frame(&mut ctx, &mut scene, &mut env);
}
fn main() {
    let m = std::env::args().nth(1).unwrap_or_default();
    let esc = b"\x1b]0;T\x07\x1b[2J";
    match m.as_str() {
        "f02f" => { let mut v = vec![]; entry(b"X.TTM", &ttm_body(0xF02F, esc), &mut v); frame(&Archive::new(&map(&[0]), &v).unwrap()); }
        "ads" => { // ADS RES lists a TTM whose name carries ESC bytes; load error printed as engine/mod.rs:459 does
            let mut res = vec![1u8, 0, 1, 0]; res.extend_from_slice(esc); res.push(0);
            let mut body = chunk(b"RES:", &res); body.extend(chunk(b"SCR:", &rle(&[1,2,3,4])));
            let mut v = vec![]; entry(b"A.ADS", &body, &mut v);
            let a = Archive::new(&map(&[0]), &v).unwrap();
            match AdsRuntime::load(&a, 1, "A.ADS") { Ok(_) => println!("ok"), Err(e) => eprintln!("warning: {e}") }
        }
        "ads_name" => { // name as exe.ads_names would hold (<=15 bytes)
            let v = vec![]; let a = Archive::new(&map(&[]), &v).unwrap();
            match AdsRuntime::load(&a, 1, "\x1bc\x1b[2J\x07") { Ok(_) => println!("ok"), Err(e) => eprintln!("warning: {e}") }
        }
        "map" => { // RESOURCE.MAP entry whose name has ESC and whose data is out of range -> startup error
            let mut v = vec![]; let mut h = [0u8; 13]; h[..7].copy_from_slice(b"\x1b[2Jab\x07"); v.extend_from_slice(&h); v.extend_from_slice(&1000u32.to_le_bytes());
            match Archive::new(&map(&[0]), &v) { Ok(_) => println!("ok"), Err(e) => eprintln!("error: {e}") }
        }
        _ => {}
    }
}
