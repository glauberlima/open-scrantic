# assets/

- **`assets/`** — `resource.rs` parses the Dynamix archive (chunked resources, RLE/LZW, 4 bpp
  BMP/SCR, PAL, TTM split into frames at `0x0FF0`, ADS). `exe.rs` reads the NE executable: the
  `WAVESFXn` sounds (mapped through the RT_NAMETABLE resource) and logic tables at fixed offsets in
  the data segment (scene catalog, walk routes/paths, RNG seed table, whitecap sequences). It
  validates known strings to reject other builds of the EXE.
