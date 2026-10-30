//! Sound effects with `sndPlaySound(SND_ASYNC)` semantics: a new sound cuts the previous one.
//! With `--soft-audio` the 8-bit 11,025 Hz samples are resampled once to the device rate with a
//! low-passed windowed sinc, and a cut fades the previous sound out over a few milliseconds.

use std::collections::HashMap;
use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use rodio::buffer::SamplesBuffer;
use rodio::cpal::BufferSize;
use rodio::{ChannelCount, CpalErrorKind, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};

/// Rate of the original WAVE resources.
const SOURCE_RATE: u32 = 11025;
/// Cut-off, as a fraction of the original Nyquist frequency, of the soft mode.
const SOFT_CUTOFF: f32 = 0.6;
/// Zero crossings on each side of the sinc kernel.
const LOBES: f32 = 8.0;
const FADE_MS: u32 = 3;
/// Device buffer. On Linux a fixed 8192 frames (about 170 ms at 48 kHz): rodio's default of 50 ms
/// underruns on some ALSA setups (USB DACs). Elsewhere the driver's own choice, so a sound is not
/// heard a sixth of a second after the picture that caused it.
#[cfg(target_os = "linux")]
const DEVICE_BUFFER: BufferSize = BufferSize::Fixed(8192);
#[cfg(not(target_os = "linux"))]
const DEVICE_BUFFER: BufferSize = BufferSize::Default;

fn sinc(x: f32) -> f32 {
    if x.abs() < 1e-6 { 1.0 } else { (PI * x).sin() / (PI * x) }
}

/// Resamples `src` from 11,025 Hz to `rate` with a Lanczos-windowed sinc low-passed at `cutoff`
/// (1.0 keeps the whole original band). Samples outside `src` count as silence.
fn resample(src: &[f32], rate: u32, cutoff: f32) -> Vec<f32> {
    let step = SOURCE_RATE as f32 / rate as f32;
    let reach = LOBES / cutoff;
    let len = (src.len() as u64 * rate as u64).div_ceil(SOURCE_RATE as u64) as usize;
    (0..len)
        .map(|i| {
            let t = i as f32 * step;
            let first = (t - reach).ceil() as i64;
            let last = (t + reach).floor() as i64;
            (first.max(0)..=last.min(src.len() as i64 - 1))
                .map(|j| {
                    let x = (t - j as f32) * cutoff;
                    src[j as usize] * cutoff * sinc(x) * sinc(x / LOBES)
                })
                .sum()
        })
        .collect()
}

struct Voice {
    samples: Arc<[f32]>,
    pos: usize,
}

impl Voice {
    fn next(&mut self) -> Option<f32> {
        let s = self.samples.get(self.pos).copied()?;
        self.pos += 1;
        Some(s)
    }
}

/// What is audible: the current sound and, briefly, the one it cut.
struct Mix {
    current: Option<Voice>,
    fading: Option<(Voice, u32)>,
    fade_len: u32,
}

impl Mix {
    fn play(&mut self, samples: Arc<[f32]>) {
        if let Some(old) = self.current.take().filter(|v| v.pos < v.samples.len()) {
            self.fading = Some((old, 0));
        }
        self.current = Some(Voice { samples, pos: 0 });
    }

    fn next_sample(&mut self) -> f32 {
        let mut out = self.current.as_mut().and_then(Voice::next).unwrap_or(0.0);
        if let Some((voice, n)) = &mut self.fading {
            match voice.next() {
                Some(s) if *n < self.fade_len => {
                    out += s * (1.0 - *n as f32 / self.fade_len as f32);
                    *n += 1;
                }
                _ => self.fading = None,
            }
        }
        out
    }
}

/// Endless mono source that the mixer keeps pulling; silent while nothing plays.
struct Stream {
    mix: Arc<Mutex<Mix>>,
    rate: SampleRate,
}

fn lock(mix: &Mutex<Mix>) -> MutexGuard<'_, Mix> {
    mix.lock().unwrap_or_else(|e| e.into_inner())
}

impl Iterator for Stream {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        Some(lock(&self.mix).next_sample())
    }
}

impl Source for Stream {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        ChannelCount::MIN
    }

    fn sample_rate(&self) -> SampleRate {
        self.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

enum Mode {
    /// Original behavior: the sample plays as is and dropping the previous player cuts it.
    Plain { current: Option<Player> },
    /// `--soft-audio`: resampled, low-passed and cross-faded.
    Soft {
        mix: Arc<Mutex<Mix>>,
        rate: u32,
        /// Sounds already resampled to the device rate, by `WAVESFXn` number.
        cache: HashMap<u32, Arc<[f32]>>,
    },
}

pub struct Audio {
    sink: MixerDeviceSink,
    mode: Mode,
    soft: bool,
    /// Set by the stream's error callback when the device failed in a way an underrun does not explain.
    broken: Arc<AtomicBool>,
}

/// Opens the default output device and builds the mode on top of it.
fn open(soft: bool, broken: &Arc<AtomicBool>) -> Result<(MixerDeviceSink, Mode), String> {
    let flag = Arc::clone(broken);
    let mut sink = DeviceSinkBuilder::from_default_device()
        .and_then(|b| {
            b.with_buffer_size(DEVICE_BUFFER)
                .with_error_callback(move |e| {
                    // An underrun is recovered by the backend; anything else can leave the stream dead.
                    if e.kind() != CpalErrorKind::Xrun && !flag.swap(true, Ordering::Relaxed) {
                        eprintln!("audio stream error: {e}");
                    }
                })
                .open_stream()
        })
        .map_err(|e| e.to_string())?;
    sink.log_on_drop(false);
    let mode = if soft {
        let rate = sink.config().sample_rate();
        let mix = Arc::new(Mutex::new(Mix {
            current: None,
            fading: None,
            fade_len: (rate.get() * FADE_MS / 1000).max(1),
        }));
        sink.mixer().add(Stream {
            mix: Arc::clone(&mix),
            rate,
        });
        Mode::Soft {
            mix,
            rate: rate.get(),
            cache: HashMap::new(),
        }
    } else {
        Mode::Plain { current: None }
    };
    Ok((sink, mode))
}

impl Audio {
    /// Returns None when no output device is available; the screensaver then runs silently.
    /// `soft` resamples the sounds to the device rate, low-passes them to hide the hiss of the
    /// 8-bit samples and fades a cut sound out; without it they play as the original did.
    pub fn new(soft: bool) -> Option<Self> {
        let broken = Arc::new(AtomicBool::new(false));
        match open(soft, &broken) {
            Ok((sink, mode)) => Some(Self {
                sink,
                mode,
                soft,
                broken,
            }),
            Err(e) => {
                eprintln!("warning: audio disabled: {e}");
                None
            }
        }
    }

    pub fn play(&mut self, bank: &HashMap<u32, Vec<f32>>, id: u32) {
        let Some(raw) = bank.get(&id) else { return };
        if self.broken.swap(false, Ordering::Relaxed) {
            match open(self.soft, &self.broken) {
                Ok((sink, mode)) => (self.sink, self.mode) = (sink, mode),
                // No device yet (for instance right after an unplug): try again with the next sound.
                Err(_) => self.broken.store(true, Ordering::Relaxed),
            }
        }
        match &mut self.mode {
            Mode::Plain { current } => {
                let player = Player::connect_new(self.sink.mixer());
                player.append(SamplesBuffer::new(
                    ChannelCount::MIN,
                    SampleRate::new(SOURCE_RATE).unwrap(),
                    raw.clone(),
                ));
                *current = Some(player); // dropping the previous player stops it
            }
            Mode::Soft { mix, rate, cache } => {
                let samples = Arc::clone(
                    cache
                        .entry(id)
                        .or_insert_with(|| resample(raw, *rate, SOFT_CUTOFF).into()),
                );
                lock(mix).play(samples);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resampling_scales_the_length_and_keeps_a_constant_level() {
        let out = resample(&[0.5; 200], 44100, 1.0);
        assert_eq!(out.len(), 800);
        assert!(out[100..700].iter().all(|&s| (s - 0.5).abs() < 0.01));
    }

    #[test]
    fn soft_mode_attenuates_the_top_of_the_band() {
        let tone: Vec<f32> = (0..400).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }).collect(); // Nyquist
        let peak = |c| {
            resample(&tone, 44100, c)[400..1200]
                .iter()
                .fold(0f32, |m, s| m.max(s.abs()))
        };
        assert!(peak(SOFT_CUTOFF) < 0.05 * peak(1.0).max(0.01));
    }

    #[test]
    fn a_cut_fades_the_previous_sound_out_and_then_ends_it() {
        let mut mix = Mix {
            current: None,
            fading: None,
            fade_len: 4,
        };
        mix.play(Arc::from(vec![1.0; 100]));
        assert_eq!(mix.next_sample(), 1.0);
        mix.play(Arc::from(vec![0.0; 100]));
        let faded: Vec<f32> = (0..5).map(|_| mix.next_sample()).collect();
        assert_eq!(faded, [1.0, 0.75, 0.5, 0.25, 0.0]);
        assert!(mix.fading.is_none());
    }

    #[test]
    fn silence_when_nothing_plays() {
        let mut mix = Mix {
            current: None,
            fading: None,
            fade_len: 4,
        };
        assert_eq!(mix.next_sample(), 0.0);
    }
}
