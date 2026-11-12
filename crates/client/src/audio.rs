//! Synthetisierte Soundeffekte und ein kleiner Synthwave-Loop, ganz ohne Dateien.

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use macroquad::audio::{load_sound_from_bytes, play_sound, set_sound_volume, stop_sound, PlaySoundParams, Sound};
use panzer_core::rng::Rng;

const RATE: u32 = 44100;

#[derive(Clone, Copy, PartialEq)]
enum Wave {
    Sin,
    Saw,
    Sq,
    Noise,
}

/// Schicht: Startfrequenz, Endfrequenz, Dauer, Welle, Lautstärke, Rauschanteil, Abklingen,
/// Verzögerung, anschwellend
struct Layer(f64, f64, f64, Wave, f64, f64, f64, f64, bool);

fn l(f0: f64, f1: f64, dur: f64, w: Wave, vol: f64, noise: f64, decay: f64) -> Layer {
    Layer(f0, f1, dur, w, vol, noise, decay, 0.0, false)
}

fn ld(f0: f64, f1: f64, dur: f64, w: Wave, vol: f64, noise: f64, decay: f64, delay: f64) -> Layer {
    Layer(f0, f1, dur, w, vol, noise, decay, delay, false)
}

fn sw(f0: f64, f1: f64, dur: f64, w: Wave, vol: f64, noise: f64, delay: f64) -> Layer {
    Layer(f0, f1, dur, w, vol, noise, 0.0, delay, true)
}

fn sound_table() -> Vec<(&'static str, Vec<Layer>)> {
    use Wave::*;
    vec![
        ("shot", vec![l(880.0, 320.0, 0.09, Sq, 0.12, 0.0, 5.0), l(3200.0, 900.0, 0.05, Noise, 0.1, 1.0, 8.0)]),
        ("mg", vec![l(700.0, 420.0, 0.05, Sq, 0.1, 0.2, 6.0), l(2600.0, 900.0, 0.04, Noise, 0.1, 1.0, 8.0)]),
        ("sg", vec![l(300.0, 110.0, 0.2, Sq, 0.18, 0.5, 8.0), l(2600.0, 300.0, 0.28, Noise, 0.32, 1.0, 6.0)]),
        ("rocket", vec![l(110.0, 240.0, 0.45, Saw, 0.14, 0.6, 3.0), l(1600.0, 500.0, 0.5, Noise, 0.2, 1.0, 3.0)]),
        ("frag", vec![l(220.0, 90.0, 0.15, Sin, 0.3, 0.0, 5.0)]),
        ("laser", vec![l(1800.0, 180.0, 0.3, Saw, 0.12, 0.0, 4.0), l(2600.0, 260.0, 0.32, Sin, 0.1, 0.0, 3.0)]),
        (
            "boom",
            vec![
                l(95.0, 32.0, 0.8, Sin, 0.5, 0.0, 4.0),
                l(2600.0, 200.0, 0.75, Noise, 0.5, 1.0, 4.5),
                ld(6000.0, 2400.0, 0.16, Noise, 0.18, 1.0, 9.0, 0.02),
            ],
        ),
        (
            "bigboom",
            vec![
                l(75.0, 22.0, 1.4, Sin, 0.6, 0.0, 3.0),
                l(2000.0, 110.0, 1.3, Noise, 0.6, 1.0, 3.2),
                ld(6000.0, 1800.0, 0.3, Noise, 0.24, 1.0, 8.0, 0.03),
            ],
        ),
        (
            "nuke",
            vec![
                l(60.0, 16.0, 3.0, Sin, 0.62, 0.0, 1.6),
                l(1500.0, 50.0, 3.0, Noise, 0.75, 1.0, 1.5),
                l(7000.0, 2200.0, 0.5, Noise, 0.35, 1.0, 6.0),
            ],
        ),
        ("pick", vec![l(600.0, 1150.0, 0.13, Sq, 0.1, 0.0, 2.0), ld(900.0, 1800.0, 0.12, Sin, 0.08, 0.0, 2.0, 0.06)]),
        (
            "mystery",
            vec![
                l(520.0, 520.0, 0.07, Sq, 0.07, 0.0, 3.0),
                ld(660.0, 660.0, 0.07, Sq, 0.07, 0.0, 3.0, 0.07),
                ld(780.0, 780.0, 0.07, Sq, 0.07, 0.0, 3.0, 0.14),
                ld(1040.0, 1040.0, 0.25, Sq, 0.09, 0.0, 3.0, 0.21),
            ],
        ),
        ("tele", vec![l(300.0, 1500.0, 0.25, Sin, 0.2, 0.0, 2.0), l(2000.0, 7000.0, 0.25, Noise, 0.08, 1.0, 3.0)]),
        ("ice", vec![l(1900.0, 2700.0, 0.22, Sin, 0.1, 0.1, 3.0)]),
        ("freeze", vec![l(3600.0, 1800.0, 0.4, Noise, 0.26, 1.0, 4.0), l(2600.0, 3400.0, 0.3, Sin, 0.07, 0.0, 4.0)]),
        ("bnc", vec![l(280.0, 620.0, 0.12, Sin, 0.22, 0.0, 4.0)]),
        ("rang", vec![l(200.0, 420.0, 0.3, Saw, 0.08, 0.4, 3.0)]),
        ("mine", vec![l(1200.0, 1200.0, 0.05, Sq, 0.12, 0.0, 6.0)]),
        ("hole", vec![l(90.0, 38.0, 0.8, Saw, 0.18, 0.2, 1.5), l(700.0, 60.0, 0.9, Noise, 0.2, 1.0, 1.5)]),
        ("strike", vec![l(1000.0, 560.0, 0.45, Sq, 0.08, 0.0, 1.0)]),
        ("whistle", vec![l(2000.0, 480.0, 0.95, Sin, 0.11, 0.0, 0.3)]),
        ("jet", vec![sw(300.0, 3000.0, 1.3, Noise, 0.32, 1.0, 0.0)]),
        ("blip", vec![l(520.0, 760.0, 0.08, Sin, 0.16, 0.0, 3.0)]),
        (
            "win",
            vec![
                l(523.0, 523.0, 0.13, Sq, 0.09, 0.0, 3.0),
                ld(659.0, 659.0, 0.13, Sq, 0.09, 0.0, 3.0, 0.11),
                ld(784.0, 784.0, 0.13, Sq, 0.09, 0.0, 3.0, 0.22),
                ld(1046.0, 1046.0, 0.45, Sq, 0.11, 0.0, 3.0, 0.33),
                ld(1046.0, 1046.0, 0.45, Sin, 0.08, 0.0, 3.0, 0.33),
            ],
        ),
        ("tink", vec![l(2700.0, 1900.0, 0.05, Sin, 0.08, 0.2, 8.0)]),
        ("shield", vec![l(1500.0, 700.0, 0.3, Sq, 0.07, 0.0, 5.0), l(6500.0, 2500.0, 0.32, Noise, 0.24, 1.0, 6.0)]),
        ("zap", vec![l(150.0, 90.0, 0.42, Sq, 0.12, 0.6, 3.0), l(7500.0, 2500.0, 0.42, Noise, 0.32, 1.0, 4.0)]),
        ("flame", vec![l(1500.0, 500.0, 0.22, Noise, 0.16, 1.0, 4.0)]),
        ("tick", vec![l(1400.0, 1400.0, 0.05, Sq, 0.14, 0.0, 6.0)]),
        ("siren", vec![sw(620.0, 940.0, 0.42, Saw, 0.11, 0.0, 0.0), sw(940.0, 620.0, 0.42, Saw, 0.11, 0.0, 0.42)]),
        ("drop", vec![l(150.0, 60.0, 0.18, Sin, 0.26, 0.0, 6.0), l(1000.0, 200.0, 0.15, Noise, 0.14, 1.0, 8.0)]),
        (
            "round",
            vec![
                sw(220.0, 220.0, 0.7, Saw, 0.05, 0.0, 0.0),
                sw(330.0, 330.0, 0.7, Saw, 0.04, 0.0, 0.0),
                sw(440.0, 440.0, 0.6, Sq, 0.03, 0.0, 0.08),
            ],
        ),
        ("ann", vec![l(660.0, 990.0, 0.16, Sq, 0.11, 0.0, 3.0), ld(990.0, 1320.0, 0.26, Sq, 0.11, 0.0, 3.0, 0.12)]),
        ("slowmo", vec![l(170.0, 38.0, 1.1, Sin, 0.22, 0.0, 1.4), l(1000.0, 70.0, 1.2, Noise, 0.14, 1.0, 1.4)]),
        ("clank", vec![l(520.0, 300.0, 0.12, Sq, 0.12, 0.5, 7.0)]),
        (
            "rail",
            vec![
                l(4200.0, 900.0, 0.35, Saw, 0.13, 0.0, 4.0),
                l(8000.0, 1500.0, 0.3, Noise, 0.3, 1.0, 5.0),
                l(90.0, 40.0, 0.4, Sin, 0.35, 0.0, 4.0),
            ],
        ),
        ("saw", vec![l(900.0, 1300.0, 0.45, Saw, 0.07, 0.3, 2.0)]),
        // Sägeblatt schleift an der Wand: kreischender Sägezahn plus helles Rauschen
        ("grind", vec![l(2300.0, 1700.0, 0.2, Saw, 0.06, 0.5, 5.0), l(7000.0, 3000.0, 0.2, Noise, 0.14, 1.0, 6.0)]),
        // Kanonenkugel prallt ab: dumpfer Schlag
        ("thud", vec![l(120.0, 45.0, 0.3, Sin, 0.42, 0.0, 6.0), l(900.0, 150.0, 0.25, Noise, 0.24, 1.0, 6.0)]),
        // Klebebombe haftet
        ("splat", vec![l(520.0, 140.0, 0.14, Sin, 0.18, 0.3, 6.0), l(1800.0, 400.0, 0.12, Noise, 0.16, 1.0, 8.0)]),
        // Geschossschere: zwei metallische Klicks
        (
            "snip",
            vec![
                l(3300.0, 2700.0, 0.04, Sq, 0.07, 0.3, 9.0),
                ld(4300.0, 3100.0, 0.05, Sq, 0.07, 0.3, 9.0, 0.07),
                l(8000.0, 4000.0, 0.14, Noise, 0.12, 1.0, 6.0),
            ],
        ),
        // Entwaffnungsfächer: Luftstoß
        ("whoosh", vec![sw(400.0, 2000.0, 0.3, Noise, 0.22, 1.0, 0.0), l(320.0, 150.0, 0.25, Sin, 0.08, 0.0, 4.0)]),
        // Konterklinge: Klinge trifft Kugel
        (
            "clang",
            vec![
                l(1250.0, 1180.0, 0.35, Sq, 0.06, 0.1, 6.0),
                l(1870.0, 1800.0, 0.3, Sin, 0.08, 0.0, 5.0),
                l(6000.0, 2000.0, 0.08, Noise, 0.18, 1.0, 9.0),
            ],
        ),
        // Zündpause: fallender Ton und zwei Uhrticks
        (
            "pause",
            vec![
                l(900.0, 300.0, 0.4, Sin, 0.13, 0.0, 3.0),
                l(1400.0, 1400.0, 0.04, Sq, 0.08, 0.0, 6.0),
                ld(1400.0, 1400.0, 0.04, Sq, 0.06, 0.0, 6.0, 0.16),
            ],
        ),
        ("plasma", vec![l(180.0, 520.0, 0.5, Sq, 0.1, 0.3, 2.0), l(2200.0, 600.0, 0.45, Noise, 0.12, 1.0, 3.0)]),
        ("cannon", vec![l(70.0, 30.0, 0.6, Sin, 0.55, 0.0, 4.0), l(1800.0, 150.0, 0.5, Noise, 0.45, 1.0, 4.0)]),
        ("orbit", vec![sw(300.0, 1800.0, 1.4, Sin, 0.1, 0.0, 0.0)]),
        ("beam", vec![l(120.0, 110.0, 1.4, Saw, 0.12, 0.5, 0.6), l(5000.0, 1800.0, 1.4, Noise, 0.18, 1.0, 0.8)]),
        ("push", vec![l(900.0, 120.0, 0.35, Noise, 0.35, 1.0, 4.0), l(240.0, 80.0, 0.3, Sin, 0.25, 0.0, 4.0)]),
        ("jump", vec![l(200.0, 900.0, 0.22, Sin, 0.22, 0.0, 3.0), l(1500.0, 300.0, 0.2, Noise, 0.1, 1.0, 5.0)]),
        ("build", vec![l(160.0, 90.0, 0.18, Sq, 0.2, 0.4, 6.0), l(900.0, 300.0, 0.15, Noise, 0.18, 1.0, 7.0)]),
        ("mirror", vec![l(2400.0, 4200.0, 0.12, Sin, 0.12, 0.0, 5.0)]),
        ("trap", vec![l(1400.0, 500.0, 0.12, Sq, 0.2, 0.3, 8.0), l(3000.0, 1200.0, 0.1, Noise, 0.2, 1.0, 9.0)]),
        (
            "fw",
            vec![l(1200.0, 1200.0, 0.05, Noise, 0.3, 1.0, 8.0), ld(3000.0, 1800.0, 0.6, Noise, 0.12, 1.0, 3.0, 0.05)],
        ),
        (
            "disco",
            [523.0, 784.0, 659.0, 1046.0]
                .iter()
                .enumerate()
                .map(|(j, &f)| ld(f, f, if j < 3 { 0.11 } else { 0.3 }, Sq, 0.08, 0.0, 3.0, 0.12 * j as f64))
                .collect(),
        ),
    ]
}

/// Mindestabstand zwischen zwei gleichen Klängen (z. B. 7 Schrotkugeln -> 1 Knall)
fn gap(name: &str) -> f64 {
    match name {
        "flame" => 0.11,
        "tink" => 0.06,
        "boom" => 0.05,
        "siren" => 0.5,
        "rocket" => 0.08,
        _ => 0.035,
    }
}

fn synth(layers: &[Layer]) -> Vec<f64> {
    let rate = RATE as f64;
    let total = layers.iter().map(|ly| (rate * (ly.2 + ly.7)) as usize).max().unwrap_or(1);
    let mut buf = vec![0.0; total + 1];
    for ly in layers {
        let &Layer(f0, f1, dur, wave, vol, noise, decay, delay, swell) = ly;
        let start = (rate * delay) as usize;
        let n = (rate * dur) as usize;
        let mut rnd = Rng::new((f0 * 13.0 + f1) as u64); // gleicher Klang bei jedem Start
        let (mut phase, mut lp) = (0.0f64, 0.0f64);
        let att = rate * 0.004;
        for j in 0..n {
            let t = j as f64 / n as f64;
            let f = f0 + (f1 - f0) * t;
            let v = if wave == Wave::Noise {
                let c = (f / 5000.0).min(1.0);
                lp += (rnd.uniform(-1.0, 1.0) - lp) * c;
                lp * (0.5 / c.max(0.05)).sqrt()
            } else {
                phase += f / rate;
                let ph = phase.fract();
                let mut tone = match wave {
                    Wave::Sin => (TAU * ph).sin(),
                    Wave::Saw => 2.0 * ph - 1.0,
                    _ => {
                        if ph < 0.5 {
                            1.0
                        } else {
                            -1.0
                        }
                    }
                };
                if noise > 0.0 {
                    lp += (rnd.uniform(-1.0, 1.0) - lp) * 0.25;
                    tone = tone * (1.0 - noise) + lp * 2.0 * noise;
                }
                tone
            };
            let mut env = if swell { (PI * t).sin() } else { (-decay * t).exp() };
            if (j as f64) < att {
                env *= j as f64 / att;
            }
            buf[start + j] += v * env * vol;
        }
    }
    buf
}

/// Mono-Puffer -> Stereo-WAV mit Panorama (0 = links, 1 = rechts)
fn wav(buf: &[f64], pan: f64, vol: f64) -> Vec<u8> {
    let (gl, gr) = ((2.0 * (1.0 - pan)).min(1.0) * vol, (2.0 * pan).min(1.0) * vol);
    let n = buf.len();
    let mut out = Vec::with_capacity(44 + n * 4);
    let data_len = (n * 4) as u32;
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&2u16.to_le_bytes()); // Stereo
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 4).to_le_bytes());
    out.extend_from_slice(&4u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for &v in buf {
        let l = ((v * gl).clamp(-1.0, 1.0) * 32767.0) as i16;
        let r = ((v * gr).clamp(-1.0, 1.0) * 32767.0) as i16;
        out.extend_from_slice(&l.to_le_bytes());
        out.extend_from_slice(&r.to_le_bytes());
    }
    out
}

pub struct Sfx {
    pub on: bool,
    /// Name -> [links, Mitte, rechts]
    sounds: HashMap<&'static str, [Sound; 3]>,
    last: HashMap<&'static str, f64>,
}

impl Sfx {
    pub async fn new() -> Sfx {
        let mut sounds = HashMap::new();
        for (name, layers) in sound_table() {
            let buf = synth(&layers);
            let mut v = Vec::new();
            for pan in [0.15, 0.5, 0.85] {
                match load_sound_from_bytes(&wav(&buf, pan, 1.0)).await {
                    Ok(s) => v.push(s),
                    Err(_) => break,
                }
            }
            if let Ok(arr) = <[Sound; 3]>::try_from(v) {
                sounds.insert(name, arr);
            }
        }
        Sfx { on: true, sounds, last: HashMap::new() }
    }

    /// pan: 0 = ganz links, 1 = ganz rechts, None = Mitte
    pub fn play(&mut self, name: &'static str, pan: Option<f64>, vol: f64) {
        if !self.on {
            return;
        }
        let Some(s) = self.sounds.get(name) else { return };
        let now = macroquad::time::get_time();
        if now - self.last.get(name).copied().unwrap_or(-1.0) < gap(name) {
            return;
        }
        self.last.insert(name, now);
        let idx = match pan {
            Some(p) if p < 0.35 => 0,
            Some(p) if p > 0.65 => 2,
            _ => 1,
        };
        play_sound(&s[idx], PlaySoundParams { looped: false, volume: vol as f32 });
    }
}

// --- Musik -------------------------------------------------------------------

const MUSIC_BPM: f64 = 116.0;
const MUSIC_VOL: f32 = 0.3;
/// Takte: Grundton der Bass-Oktave (Hz) und Akkordtöne in Halbtönen (Am, F, C, G)
const CHORDS: [(f64, [i32; 4]); 4] =
    [(55.0, [0, 3, 7, 12]), (43.65, [0, 4, 7, 12]), (65.41, [0, 4, 7, 12]), (49.0, [0, 4, 7, 12])];

pub struct Music {
    pub on: bool,
    sound: Option<Sound>,
    playing: bool,
    vol: f32,
}

impl Music {
    pub async fn new() -> Music {
        let buf = build_loop();
        let sound = load_sound_from_bytes(&wav(&buf, 0.5, 1.0)).await.ok();
        Music { on: true, sound, playing: false, vol: 0.0 }
    }

    /// Jeden Frame: an/aus, in der Zeitlupe leiser
    pub fn update(&mut self, enabled: bool, ts: f64) {
        let Some(s) = &self.sound else { return };
        if !(self.on && enabled) {
            if self.playing {
                stop_sound(s);
                self.playing = false;
            }
            return;
        }
        let vol = MUSIC_VOL * (0.4 + 0.6 * ts as f32);
        if !self.playing {
            play_sound(s, PlaySoundParams { looped: true, volume: vol });
            self.playing = true;
            self.vol = vol;
        } else if (vol - self.vol).abs() > 0.01 {
            set_sound_volume(s, vol);
            self.vol = vol;
        }
    }
}

/// 8 Takte: erst ohne, dann mit Melodie
fn build_loop() -> Vec<f64> {
    let rate = RATE as f64;
    let step = 60.0 / MUSIC_BPM / 4.0; // Sechzehntel
    let n = (rate * step * 16.0) as usize;
    let drums = drums(rate, n, step);
    let mut plain = Vec::new();
    let mut lead = Vec::new();
    for (root, chord) in CHORDS {
        let mut base = drums.clone();
        bass(&mut base, rate, step, root);
        pad(&mut base, rate, n, root, &chord);
        let mut with = base.clone();
        let mut a = vec![0.0; n];
        arp(&mut a, rate, step, root, &chord);
        for (w, v) in with.iter_mut().zip(a) {
            *w += v;
        }
        plain.extend(base);
        lead.extend(with);
    }
    plain.extend(lead);
    plain
}

fn drums(rate: f64, n: usize, step: f64) -> Vec<f64> {
    let mut buf = vec![0.0; n];
    let mut rnd = Rng::new(5);
    for k in 0..16 {
        let at = (k as f64 * step * rate) as usize;
        if k % 4 == 0 {
            // Kick: Sinus mit fallender Tonhöhe
            let mut phase = 0.0;
            for j in 0..((0.28 * rate) as usize).min(n - at) {
                let t = j as f64 / rate;
                phase += (48.0 + 110.0 * (-t * 28.0).exp()) / rate;
                buf[at + j] += (TAU * phase).sin() * (-t * 9.0).exp() * 0.62;
            }
        }
        if k == 4 || k == 12 {
            // Clap: Rauschen, mehrfach angerissen
            let mut lp = 0.0;
            for j in 0..((0.22 * rate) as usize).min(n - at) {
                let t = j as f64 / rate;
                lp += (rnd.uniform(-1.0, 1.0) - lp) * 0.55;
                let hit = if t < 0.03 { (-((t * 1000.0) % 11.0) / 3.0).exp() } else { 1.0 };
                buf[at + j] += lp * (-t * 16.0).exp() * 0.32 * hit;
            }
        }
        if k % 2 == 1 || k % 4 == 2 {
            // Hi-Hats, auf den Offbeats lauter
            let vol = if k % 4 == 2 { 0.1 } else { 0.045 };
            let mut lp = 0.0;
            for j in 0..((0.06 * rate) as usize).min(n - at) {
                let t = j as f64 / rate;
                let x = rnd.uniform(-1.0, 1.0);
                lp += (x - lp) * 0.5;
                buf[at + j] += (x - lp) * (-t * 70.0).exp() * vol;
            }
        }
    }
    buf
}

/// Achtel-Bass (gefilterter Sägezahn), jede zweite Note eine Oktave höher
fn bass(buf: &mut [f64], rate: f64, step: f64, root: f64) {
    let n = buf.len();
    for k in (0..16).step_by(2) {
        let f = root * if k % 4 == 2 { 2.0 } else { 1.0 };
        let at = (k as f64 * step * rate) as usize;
        let (mut phase, mut lp) = (0.0f64, 0.0f64);
        for j in 0..((step * 1.8 * rate) as usize).min(n - at) {
            let t = j as f64 / rate;
            phase += f / rate;
            let saw = 2.0 * phase.fract() - 1.0;
            lp += (saw - lp) * (0.05 + 0.25 * (-t * 18.0).exp()); // Filter schließt sich
            let env = (t * 400.0).min(1.0) * (-t * 7.0).exp();
            buf[at + j] += lp * env * 0.5;
        }
    }
}

/// Leiser Flächenklang über den ganzen Takt (Grundton + Quinte, zwei Oktaven höher)
fn pad(buf: &mut [f64], rate: f64, n: usize, root: f64, chord: &[i32; 4]) {
    for semi in [chord[0], chord[2]] {
        let f = root * 4.0 * 2f64.powf(semi as f64 / 12.0);
        let fd = f * 1.004; // leicht verstimmt -> Schwebung
        for (j, b) in buf.iter_mut().enumerate().take(n) {
            let t = j as f64 / rate;
            let env = (t * 3.0).min(1.0) * ((n - j) as f64 / rate * 6.0).min(1.0);
            *b += ((TAU * f * t).sin() + (TAU * fd * t).sin()) * env * 0.028;
        }
    }
}

/// Arpeggio in Sechzehnteln mit Echo
fn arp(buf: &mut [f64], rate: f64, step: f64, root: f64, chord: &[i32; 4]) {
    let n = buf.len();
    let notes = [chord[0], chord[1], chord[2], chord[3], chord[2], chord[1]];
    let echo = (3.0 * step * rate) as usize;
    for k in 0..16 {
        let f = root * 8.0 * 2f64.powf(notes[k % notes.len()] as f64 / 12.0);
        let at = (k as f64 * step * rate) as usize;
        let mut phase = 0.0f64;
        for j in 0..(step * 1.5 * rate) as usize {
            let t = j as f64 / rate;
            phase += f / rate;
            let sq = if phase.fract() < 0.5 { 0.6 } else { -0.6 };
            let v = sq * (t * 600.0).min(1.0) * (-t * 22.0).exp() * 0.12;
            buf[(at + j) % n] += v;
            buf[(at + j + echo) % n] += v * 0.35;
        }
    }
}
