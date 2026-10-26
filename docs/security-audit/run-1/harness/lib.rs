//! Audit harness: compiles the target's std-only modules unchanged (read-only #[path] includes).
#![allow(dead_code, unused_imports, clippy::all)]
#[path = "/home/user/open-scrantic/src/assets/mod.rs"]
pub mod assets;
pub mod engine {
    #[path = "/home/user/open-scrantic/src/engine/ads.rs"]
    pub mod ads;
    #[path = "/home/user/open-scrantic/src/engine/gfx.rs"]
    pub mod gfx;
    #[path = "/home/user/open-scrantic/src/engine/rng.rs"]
    pub mod rng;
    #[path = "/home/user/open-scrantic/src/engine/ttm.rs"]
    pub mod ttm;
    use std::rc::Rc;
    use crate::assets::resource::{Archive, Bmp};
    /// Copied verbatim from src/engine/mod.rs:47-57.
    pub struct Env<'a> {
        pub work: &'a mut gfx::Canvas,
        pub bg: &'a mut gfx::Canvas,
        pub front: &'a mut gfx::Canvas,
        pub archive: &'a Archive,
        pub johnwalk: &'a Rc<Bmp>,
        pub rng: &'a mut rng::Rng,
        pub sounds: &'a mut Vec<u32>,
        pub offset: (i32, i32),
        pub now: u32,
    }
}
