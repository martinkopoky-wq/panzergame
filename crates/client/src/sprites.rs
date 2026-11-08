//! Alle Sprites und Power-up-Symbole, beim Start einmal per Software gezeichnet.

use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use macroquad::prelude::*;
use panzer_core::consts::*;
use panzer_core::items::{Item, ALL_ITEMS};

use crate::raster::*;

pub const TREAD_C: [u8; 3] = [34, 36, 42];
pub const TREAD_HI: [u8; 3] = [72, 76, 86];
pub const OUTLINE: [u8; 3] = [16, 17, 20];
pub const PRISM_COLORS: [[u8; 3]; 3] = [[255, 70, 70], [255, 220, 60], [70, 200, 255]];
pub const FW_COLORS: [[u8; 3]; 5] = [[255, 70, 70], [255, 220, 60], [80, 200, 255], [120, 255, 120], [255, 120, 230]];

pub fn tex(cv: &Canvas) -> Texture2D {
    let t = Texture2D::from_rgba8(cv.w as u16, cv.h as u16, &cv.to_rgba8());
    t.set_filter(FilterMode::Linear);
    t
}

/// Farbe der Kiste: Waffen orange, Effekte blau, Wundertüte gold.
pub fn box_color(item: Item) -> [u8; 3] {
    if item == Item::Mystery || item.mystery_only() {
        [255, 196, 40]
    } else if item.is_buff() {
        [60, 190, 255]
    } else {
        [255, 122, 48]
    }
}

/// Panzer von oben, Rohr nach +x. S = Skalierung der Zeichnung (Auflösung des Sprites).
pub fn tank_canvas(color: [u8; 3], wreck: bool, s: f32) -> Canvas {
    let n = (44.0 * s) as usize;
    let mut cv = Canvas::new(n, n);
    let r = |cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32, rad: f32, c: [u8; 3]| {
        cv.rrect(x * s, y * s, w * s, h * s, rad * s, rgb(c))
    };
    let color = if wreck { mix(color, [34, 28, 26], 0.74) } else { color };
    let dark = shade(color, 0.55);
    let light = mix(color, [255, 255, 255], 0.42);
    let (tread, tread_hi) = if wreck { ([26, 24, 24], [46, 42, 40]) } else { (TREAD_C, TREAD_HI) };
    for y in [5.0, 30.0] {
        // Ketten mit Gliedern
        r(&mut cv, 3.5, y - 0.5, 37.0, 10.0, 4.0, OUTLINE);
        r(&mut cv, 4.0, y, 36.0, 9.0, 3.0, tread);
        let mut x = 6.0;
        while x < 39.0 {
            r(&mut cv, x, y + 1.5, 1.6, 6.0, 0.0, tread_hi);
            x += 4.0;
        }
    }
    r(&mut cv, 5.5, 8.5, 33.0, 27.0, 5.0, OUTLINE);
    r(&mut cv, 6.0, 9.0, 32.0, 26.0, 4.0, dark); // Wanne
    r(&mut cv, 7.5, 10.5, 29.0, 22.5, 3.0, color);
    r(&mut cv, 9.0, 11.5, 26.0, 3.6, 2.0, light); // Lichtkante
    r(&mut cv, 8.5, 28.5, 27.0, 4.0, 2.0, shade(color, 0.78));
    for y in [14.5, 26.5] {
        r(&mut cv, 9.5, y, 5.0, 1.5, 0.0, dark); // Lüftungsgitter am Heck
    }
    let barrel = if wreck { [40, 36, 34] } else { [60, 63, 72] };
    r(&mut cv, 20.5, 19.0, 22.0, 6.0, 1.0, OUTLINE);
    r(&mut cv, 21.0, 19.5, 21.0, 5.0, 1.0, barrel); // Rohr
    r(&mut cv, 22.0, 20.3, 19.0, 1.3, 0.0, mix(barrel, [255, 255, 255], 0.35));
    r(&mut cv, 38.5, 18.3, 4.5, 7.4, 1.0, shade(barrel, 0.7));
    cv.circle(22.0 * s, 22.0 * s, 10.6 * s, rgb(OUTLINE)); // Turm
    cv.circle(22.0 * s, 22.0 * s, 10.0 * s, rgb(dark));
    cv.circle(22.0 * s, 22.0 * s, 8.4 * s, rgb(color));
    cv.circle(20.6 * s, 20.4 * s, 4.8 * s, rgb(light));
    cv.circle(21.6 * s, 21.6 * s, 4.9 * s, rgb(color));
    cv.circle(18.4 * s, 24.6 * s, 2.3 * s, rgb(dark)); // Luke
    cv.circle(18.0 * s, 24.0 * s, 1.0 * s, rgb(shade(color, 1.15)));
    if wreck {
        // Ruß und Brandlöcher
        let mut rnd = panzer_core::rng::Rng::new(color.iter().map(|&v| v as u64).sum());
        for _ in 0..10 {
            let (x, y) = (rnd.uniform(8.0, 36.0) as f32, rnd.uniform(9.0, 35.0) as f32);
            let rr = rnd.uniform(1.6, 4.8) as f32;
            cv.circle(x * s, y * s, rr * s, rgba([18, 16, 16], 175));
        }
    }
    cv
}

/// Abgesprengter Turm mit Rohr (fliegt beim Abschuss davon)
pub fn turret_canvas(color: [u8; 3], s: f32) -> Canvas {
    let (w, h) = ((34.0 * s) as usize, (22.0 * s) as usize);
    let mut cv = Canvas::new(w, h);
    let (cx, cy) = (11.0 * s, 11.0 * s);
    let dark = shade(color, 0.55);
    let light = mix(color, [255, 255, 255], 0.35);
    cv.rect(cx - 1.0 * s, cy - 2.5 * s, 22.0 * s, 5.0 * s, rgb([40, 38, 38]));
    cv.circle(cx, cy, 10.6 * s, rgb(OUTLINE));
    cv.circle(cx, cy, 10.0 * s, rgb(dark));
    cv.circle(cx, cy, 8.4 * s, rgb(color));
    cv.circle(cx - 1.4 * s, cy - 1.6 * s, 4.8 * s, rgb(light));
    cv.circle(cx - 0.4 * s, cy - 0.4 * s, 4.9 * s, rgb(color));
    cv
}

/// Kampfjet von oben (Nase nach +x) für den Luftschlag
pub fn jet_canvas(s: f32) -> Canvas {
    let (w, h) = (66.0, 58.0);
    let mut cv = Canvas::new((w * s) as usize, (h * s) as usize);
    let tf = |pts: &[(f32, f32)]| -> Vec<(f32, f32)> {
        pts.iter().map(|&(x, y)| ((x + w / 2.0) * s, (y + h / 2.0) * s)).collect()
    };
    let (body, dark, light) = ([128, 136, 150], [74, 80, 94], [196, 204, 216]);
    let wings = [
        (8.0, -3.0),
        (-8.0, -26.0),
        (-15.0, -26.0),
        (-9.0, -3.0),
        (-9.0, 3.0),
        (-15.0, 26.0),
        (-8.0, 26.0),
        (8.0, 3.0),
    ];
    let tail = [
        (-21.0, -2.0),
        (-29.0, -12.0),
        (-32.0, -12.0),
        (-28.0, -2.0),
        (-28.0, 2.0),
        (-32.0, 12.0),
        (-29.0, 12.0),
        (-21.0, 2.0),
    ];
    let hull = [(32.0, 0.0), (23.0, -3.6), (-27.0, -4.0), (-31.0, -2.0), (-31.0, 2.0), (-27.0, 4.0), (23.0, 3.6)];
    for (part, col) in [(&wings[..], dark), (&tail[..], dark), (&hull[..], body)] {
        cv.poly(&tf(part), rgb(col));
        cv.poly_line(&tf(part), s, rgb([30, 32, 38]));
    }
    cv.poly(&tf(&[(23.0, -3.2), (-26.0, -3.6), (-26.0, -1.0), (23.0, -0.6)]), rgb(light));
    cv.poly(&tf(&[(19.0, 0.0), (13.0, -2.0), (6.0, -2.0), (4.0, 0.0), (6.0, 2.0), (13.0, 2.0)]), rgb([70, 150, 210]));
    cv.poly(&tf(&[(-23.0, -2.0), (-29.0, -10.0), (-31.0, -10.0), (-27.0, -2.0)]), rgb([220, 40, 40]));
    cv.poly(&tf(&[(-23.0, 2.0), (-29.0, 10.0), (-31.0, 10.0), (-27.0, 2.0)]), rgb([220, 40, 40]));
    cv
}

// Rakete in lokalen Koordinaten (Spitze zeigt nach +x), Ursprung = Kollisionskreis
const ROCKET_BODY: [(f32, f32); 4] = [(-10.0, -4.0), (7.0, -4.0), (7.0, 4.0), (-10.0, 4.0)];
const ROCKET_NOSE: [(f32, f32); 5] = [(7.0, -4.0), (11.0, -3.0), (15.0, 0.0), (11.0, 3.0), (7.0, 4.0)];
const ROCKET_FINS: [[(f32, f32); 4]; 2] = [
    [(-2.0, -4.0), (-8.0, -4.0), (-13.0, -10.0), (-11.0, -4.0)],
    [(-2.0, 4.0), (-8.0, 4.0), (-13.0, 10.0), (-11.0, 4.0)],
];

/// Rakete auf die Leinwand (Symbole), mit Flamme der Länge flame
pub fn rocket_cv(cv: &mut Canvas, x: f32, y: f32, a: f32, color: [u8; 3], k: f32, flame: f32) {
    let xf = Xf::rot(x, y, k, a);
    let tf = |pts: &[(f32, f32)]| -> Vec<(f32, f32)> { pts.iter().map(|&(px, py)| xf.p(px, py)).collect() };
    if flame > 0.0 {
        let tail = -12.0;
        cv.poly(&tf(&[(tail, -3.0), (tail - flame, 0.0), (tail, 3.0)]), rgb([255, 140, 25]));
        cv.poly(&tf(&[(tail, -1.6), (tail - flame * 0.55, 0.0), (tail, 1.6)]), rgb([255, 235, 120]));
    }
    let dark = shade(color, 0.6);
    let outline = [55, 55, 60];
    for fin in ROCKET_FINS {
        cv.poly(&tf(&fin), rgb(dark));
        cv.poly_line(&tf(&fin), k, rgb(outline));
    }
    cv.poly(&tf(&[(-12.0, -2.5), (-9.0, -2.5), (-9.0, 2.5), (-12.0, 2.5)]), rgb(outline));
    cv.poly(&tf(&ROCKET_BODY), rgb([210, 212, 220]));
    cv.poly(&tf(&[(-10.0, 1.5), (7.0, 1.5), (7.0, 4.0), (-10.0, 4.0)]), rgb([175, 178, 190]));
    cv.poly(&tf(&[(-7.0, -4.0), (-5.0, -4.0), (-5.0, 4.0), (-7.0, 4.0)]), rgb(color));
    cv.poly(&tf(&ROCKET_NOSE), rgb(color));
    let mut out = vec![ROCKET_BODY[0]];
    out.extend_from_slice(&ROCKET_NOSE);
    out.push(ROCKET_BODY[3]);
    cv.poly_line(&tf(&out), k, rgb(outline));
}

/// Raketen-Sprite (Mitte = Kollisionskreis, Spitze nach +x)
pub fn rocket_canvas(color: [u8; 3], s: f32) -> Canvas {
    let (w, h) = (34.0, 24.0);
    let mut cv = Canvas::new((w * s) as usize, (h * s) as usize);
    rocket_cv(&mut cv, w / 2.0 * s, h / 2.0 * s, 0.0, color, s, 0.0);
    cv
}

pub fn ice_canvas(size: usize) -> Canvas {
    let s = size as f32;
    let mut cv = Canvas::new(size, size);
    cv.rrect(0.0, 0.0, s, s, s / 5.0, rgba([170, 220, 250], 150));
    cv.rrect_line(0.0, 0.0, s, s, s / 5.0, 2.0, rgba([235, 250, 255], 220));
    cv.line(s * 0.2, s * 0.35, s * 0.4, s * 0.15, 3.0, rgba([255, 255, 255], 200));
    cv.line(s * 0.25, s * 0.6, s * 0.6, s * 0.25, 2.0, rgba([255, 255, 255], 160));
    cv
}

pub fn crown_cv(cv: &mut Canvas, cx: f32, cy: f32, k: f32) {
    let pts: Vec<(f32, f32)> =
        [(-7.0, 4.0), (-7.0, -3.0), (-3.5, 0.5), (0.0, -5.0), (3.5, 0.5), (7.0, -3.0), (7.0, 4.0)]
            .iter()
            .map(|&(x, y)| (cx + x * k, cy + y * k))
            .collect();
    cv.poly(&pts, rgb([250, 200, 30]));
    cv.poly_line(&pts, k * 0.7, rgb([160, 110, 0]));
}

/// Weißer, weich auslaufender Kreis – Grundlage für alle Lichter (additiv)
pub fn falloff_canvas(r: usize) -> Canvas {
    let n = 2 * r;
    let mut cv = Canvas::new(n, n);
    for y in 0..n {
        for x in 0..n {
            let d = (((x as f32 + 0.5 - r as f32).powi(2) + (y as f32 + 0.5 - r as f32).powi(2)).sqrt() / r as f32)
                .min(1.0);
            let v = (1.0 - d).powi(2);
            cv.px[y * n + x] = [v, v, v, 1.0];
        }
    }
    cv
}

/// Weiche Wolke (Rauch, Staub) für normales Alpha-Blending, weiß zum Einfärben
pub fn puff_canvas(r: usize) -> Canvas {
    // wie früher: 5 Ringe, innen voll deckend, außen 1/5 (hier weich verlaufend)
    let n = 2 * r + 2;
    let mut cv = Canvas::new(n, n);
    let c = r as f32 + 1.0;
    for y in 0..n {
        for x in 0..n {
            let d = ((x as f32 + 0.5 - c).hypot(y as f32 + 0.5 - c)) / r as f32;
            let a = if d >= 1.0 { 0.0 } else { (0.2 + (1.0 - d) * 1.1).min(1.0) * (1.0 - d).min(0.15) / 0.15 };
            cv.px[y * n + x] = [a, a, a, a];
        }
    }
    cv
}

/// Weicher Schatten (schwarz)
pub fn shadow_canvas(r: usize) -> Canvas {
    let n = 4 * r;
    let mut cv = Canvas::new(n, n);
    cv.circle(2.0 * r as f32, 2.0 * r as f32, r as f32, [0.0, 0.0, 0.0, 125.0 / 255.0]);
    cv.blur((r / 3).max(1));
    cv
}

/// Brandfleck für den Boden
pub fn scorch_canvas(r: usize) -> Canvas {
    let n = 2 * r;
    let rf = r as f32;
    let mut cv = Canvas::new(n, n);
    let mut rnd = panzer_core::rng::Rng::new(r as u64);
    for i in 0..6 {
        let q = rf * (1.0 - i as f32 / 7.0) * 0.8;
        let (px, py) = (rf + rnd.uniform(-2.0, 2.0) as f32, rf + rnd.uniform(-2.0, 2.0) as f32);
        cv.circle(px, py, q, rgba([14, 11, 10], 26 + 22 * i as u8));
    }
    for _ in 0..(rf * 0.7) as usize {
        let (a, d) = (rnd.uniform(0.0, TAU as f64) as f32, rnd.uniform(0.45, 0.95) as f32 * rf);
        let rr = rnd.uniform(1.0, (rf * 0.1).max(1.5) as f64) as f32;
        cv.circle(rf + a.cos() * d, rf + a.sin() * d, rr, rgba([16, 13, 12], 80));
    }
    cv
}

/// Lichtkegel eines Scheinwerfers (Spitze in der Mitte, nach +x), für den Stromausfall
pub fn cone_canvas() -> Canvas {
    let n = 78usize;
    let half = 0.52f32;
    let size = 2 * n;
    let mut cv = Canvas::filled(size, size, [0.0, 0.0, 0.0, 1.0]);
    let mut i = n as i32;
    while i > 3 {
        let v = 250.0 * (1.0 - i as f32 / n as f32).powf(0.65) / 255.0;
        let mut pts = vec![(n as f32, n as f32)];
        for j in 0..13 {
            let b = -half + j as f32 * half / 6.0;
            pts.push((n as f32 + b.cos() * i as f32, n as f32 + b.sin() * i as f32));
        }
        cv.poly(&pts, [v, v * 0.96, v * 0.86, 1.0]);
        i -= 2;
    }
    cv.circle(n as f32, n as f32, 7.0, rgb([120, 116, 104]));
    cv.blur(3);
    cv
}

/// Helle Ränder, dunkle Mitte (Gefahr am Rand), additiv eingefärbt
pub fn edge_canvas() -> Canvas {
    let (w, h) = (48usize, 30usize);
    let mut cv = Canvas::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let d = (((x as f32 - 23.5) / 24.0).hypot((y as f32 - 14.5) / 15.0) / 1.3).min(1.0);
            let v = d.powf(2.4);
            cv.px[y * w + x] = [v, v, v, 1.0];
        }
    }
    cv
}

// ---------------------------------------------------------------------------
// Power-up-Symbole

const SHOT: [u8; 3] = SHOT_C;
const G: [u8; 3] = [80, 190, 60];
const RED: [u8; 3] = [220, 30, 30];
const BLUE: [u8; 3] = [60, 140, 240];
const ICE: [u8; 3] = [120, 200, 250];

/// Formen eines Symbols (Koordinaten in Symbol-Einheiten um die Mitte)
enum Sh {
    /// Kreis x, y, r, Rand (0 = gefüllt)
    C([u8; 3], f32, f32, f32, f32),
    /// Linie x0, y0, x1, y1, Breite
    L([u8; 3], f32, f32, f32, f32, f32),
    /// Polygon, Rand (0 = gefüllt)
    P([u8; 3], Vec<(f32, f32)>, f32),
    /// Linienzug, Breite
    Pl([u8; 3], Vec<(f32, f32)>, f32),
    /// Ellipse x, y, w, h, Rand
    E([u8; 3], f32, f32, f32, f32, f32),
    /// Rechteck x, y, w, h, Randfarbe
    R([u8; 3], f32, f32, f32, f32, Option<[u8; 3]>),
    /// Bogen x, y, r, von, bis, Breite
    A([u8; 3], f32, f32, f32, f32, f32, f32),
    /// Rakete x, y, Winkel, Größe, Flamme
    Rk([u8; 3], f32, f32, f32, f32, f32),
}

fn dots(col: [u8; 3], n: usize, big_r: f32, r: f32, rot: f32) -> Vec<Sh> {
    round_pts(n, big_r, None, rot).into_iter().map(|(x, y)| Sh::C(col, x, y, r, 0.0)).collect()
}

fn icon_shapes(item: Item) -> Option<Vec<Sh>> {
    use Sh::*;
    let saw = round_pts(8, 9.0, Some(6.5), 0.0);
    let hex = round_pts(6, 9.0, None, 0.0);
    let star = round_pts(5, 10.0, Some(4.3), -FRAC_PI_2);
    Some(match item {
        Item::Split => vec![
            L(SHOT, -7.0, 0.0, 0.0, 0.0, 2.6),
            L(SHOT, 0.0, 0.0, 8.0, -6.0, 2.6),
            L(SHOT, 0.0, 0.0, 8.0, 6.0, 2.6),
            C(SHOT, -8.0, 0.0, 2.8, 0.0),
            C([240, 120, 40], 8.0, -6.0, 3.0, 0.0),
            C([240, 120, 40], 8.0, 6.0, 3.0, 0.0),
        ],
        Item::Wave => {
            let pts: Vec<(f32, f32)> =
                (0..=16).map(|j| (-10.0 + j as f32 * 1.25, 5.0 * (j as f32 * 0.8).sin())).collect();
            vec![Pl([20, 140, 110], pts, 2.8), C([20, 140, 110], 10.0, 0.0, 3.2, 0.0)]
        }
        Item::Timebomb => vec![
            L([150, 90, 30], 3.0, -6.0, 8.0, -10.0, 1.8),
            C([230, 60, 40], 8.5, -10.0, 1.8, 0.0),
            C([46, 48, 58], 0.0, 2.0, 8.5, 0.0),
            C([236, 238, 244], 0.0, 2.0, 5.5, 0.0),
            L(SHOT, 0.0, 2.0, 0.0, -1.6, 1.4),
            L(SHOT, 0.0, 2.0, 3.0, 2.0, 1.4),
        ],
        Item::Napalm => vec![
            E([190, 60, 20], 0.0, 7.0, 10.0, 3.2, 0.0),
            P([235, 100, 25], vec![(0.0, -10.0), (6.0, -2.0), (5.0, 5.0), (0.0, 8.0), (-5.0, 5.0), (-6.0, -2.0)], 0.0),
            P([255, 205, 90], vec![(0.0, -3.0), (3.0, 2.0), (2.0, 5.0), (0.0, 6.5), (-2.0, 5.0), (-3.0, 2.0)], 0.0),
        ],
        Item::Fence => vec![
            R([50, 52, 62], -10.0, -8.0, 3.5, 16.0, None),
            R([50, 52, 62], 6.5, -8.0, 3.5, 16.0, None),
            L([235, 45, 45], -7.0, -3.5, 7.0, -3.5, 2.2),
            L([235, 45, 45], -7.0, 3.5, 7.0, 3.5, 2.2),
        ],
        Item::Sprinkler => {
            let mut v = dots([235, 120, 40], 8, 9.5, 1.7, 0.0);
            v.push(L(SHOT, -6.0, -6.0, 6.0, 6.0, 3.0));
            v.push(C([66, 70, 84], 0.0, 0.0, 4.5, 0.0));
            v.push(C([150, 156, 172], 0.0, 0.0, 2.0, 0.0));
            v
        }
        Item::Hook => vec![
            L([70, 74, 86], 0.0, -7.0, 0.0, 7.0, 2.8),
            L([70, 74, 86], -4.5, -3.5, 4.5, -3.5, 2.2),
            A([70, 74, 86], 0.0, 0.0, 8.0, -PI + 0.2, -0.2, 2.8),
            C([176, 150, 110], 0.0, -9.0, 2.4, 1.4),
        ],
        Item::Destruct => vec![
            C([56, 60, 72], 0.0, 1.0, 9.5, 0.0),
            C([205, 30, 30], 0.0, 0.0, 7.0, 0.0),
            C([255, 150, 135], -2.2, -2.2, 2.0, 0.0),
        ],
        Item::Rail => vec![
            L([30, 50, 110], -10.0, 4.0, 10.0, -4.0, 5.0),
            L([120, 230, 255], -10.0, 4.0, 10.0, -4.0, 2.0),
            C([60, 140, 255], -10.0, 4.0, 2.5, 1.0),
            C([60, 140, 255], 10.0, -4.0, 2.5, 1.0),
        ],
        Item::Rc => vec![
            Rk([40, 170, 90], 2.0, 1.0, -FRAC_PI_4, 0.65, 3.0),
            L(SHOT, -5.0, -3.0, -5.0, -10.0, 1.2),
            C([230, 40, 40], -5.0, -10.0, 1.8, 0.0),
        ],
        Item::Sentry => vec![
            C([60, 64, 76], 0.0, 2.0, 8.0, 0.0),
            L(SHOT, 0.0, 0.0, 10.0, -5.0, 3.5),
            C([150, 156, 172], 0.0, 0.0, 4.5, 0.0),
            C([230, 60, 50], 0.0, 0.0, 1.6, 0.0),
        ],
        Item::Wall => {
            let mut v = Vec::new();
            for (row, xs) in [(-5.0f32, &[-5.0f32, 5.0][..]), (0.0, &[-8.0, 0.0, 8.0][..]), (5.0, &[-5.0, 5.0][..])] {
                for &dx in xs {
                    v.push(R([175, 112, 76], dx - 4.0, row - 2.3, 8.0, 4.6, Some([95, 58, 40])));
                }
            }
            v
        }
        Item::Oil => vec![
            R([40, 42, 52], -6.0, -8.0, 12.0, 16.0, None),
            L([240, 160, 40], -6.0, -4.0, 6.0, -4.0, 1.6),
            L([240, 160, 40], -6.0, 3.0, 6.0, 3.0, 1.6),
            C([20, 20, 26], 7.0, 8.0, 3.0, 0.0),
        ],
        Item::Sticky => {
            let mut v = vec![C(G, 0.0, -1.0, 7.0, 0.0)];
            for (dx, ln) in [(-4.0, 9.0), (1.0, 10.0), (5.0, 8.0)] {
                v.push(L(G, dx, 0.0, dx, ln, 2.0));
            }
            v.push(C([200, 255, 170], -2.0, -3.0, 2.0, 0.0));
            v
        }
        Item::Gl => vec![
            E([88, 108, 52], 0.0, 2.0, 12.0, 15.0, 0.0),
            E([45, 56, 26], 0.0, 2.0, 12.0, 15.0, 1.0),
            R([60, 62, 70], -3.0, -8.0, 6.0, 4.0, None),
            C([60, 62, 70], 5.0, -8.0, 2.5, 1.0),
        ],
        Item::Swarm => [(-6.0, 4.0), (0.0, -2.0), (6.0, 4.0)]
            .iter()
            .map(|&(dx, dy)| Rk([235, 120, 20], dx, dy, -FRAC_PI_2, 0.4, 2.0))
            .collect(),
        Item::Saw => {
            vec![P([175, 180, 192], saw.clone(), 0.0), P([90, 94, 104], saw, 0.8), C([60, 62, 70], 0.0, 0.0, 2.5, 0.0)]
        }
        Item::Plasma => vec![
            C([40, 170, 120], 0.0, 0.0, 8.0, 0.0),
            C([130, 255, 190], 0.0, 0.0, 6.0, 0.0),
            C([240, 255, 245], -1.0, -1.0, 2.5, 0.0),
        ],
        Item::Fw => {
            let mut v = vec![L([150, 100, 60], -6.0, 9.0, 0.0, 0.0, 1.5)];
            for (j, c) in FW_COLORS[..4].iter().enumerate() {
                let a = -FRAC_PI_2 + (j as f32 - 1.5) * 0.7;
                v.push(L(*c, 3.0, -2.0, 3.0 + a.cos() * 8.0, -2.0 + a.sin() * 8.0, 2.0));
                v.push(C(*c, 3.0 + a.cos() * 8.0, -2.0 + a.sin() * 8.0, 1.5, 0.0));
            }
            v
        }
        Item::Push => (0..3).map(|j| A(BLUE, -6.0, 0.0, 4.0 + 3.5 * j as f32, -0.9, 0.9, 2.0)).collect(),
        Item::Spider => {
            let mut v: Vec<Sh> = Vec::new();
            for side in [-1.0f32, 1.0] {
                for j in 0..4 {
                    let a = side * (0.5 + j as f32 * 0.45);
                    v.push(Pl(
                        SHOT,
                        vec![(0.0, 0.0), (a.sin() * 7.0, -a.cos() * 5.0), (a.sin() * 10.0, 4.0 - a.cos() * 9.0)],
                        1.3,
                    ));
                }
            }
            v.push(C([35, 30, 34], 0.0, 1.0, 5.0, 0.0));
            v.push(C([230, 40, 40], -2.0, -1.0, 1.2, 0.0));
            v.push(C([230, 40, 40], 2.0, -1.0, 1.2, 0.0));
            v
        }
        Item::Ram => vec![
            R([110, 80, 60], -9.0, -3.0, 12.0, 6.0, None),
            P([190, 60, 40], vec![(3.0, -7.0), (10.0, 0.0), (3.0, 7.0)], 0.0),
            P([90, 24, 16], vec![(3.0, -7.0), (10.0, 0.0), (3.0, 7.0)], 1.0),
        ],
        Item::Nova => {
            let mut v = dots(SHOT, 8, 8.0, 2.0, 0.0);
            v.push(C([230, 120, 30], 0.0, 0.0, 3.0, 0.0));
            v
        }
        Item::Seek => {
            let mut v = vec![C(RED, 0.0, 0.0, 8.0, 1.5)];
            for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                v.push(L(RED, 6.0 * dx, 6.0 * dy, 10.0 * dx, 10.0 * dy, 1.5));
            }
            v.push(C(SHOT, 0.0, 0.0, 3.0, 0.0));
            v
        }
        Item::Phase => {
            let mut v = dots([60, 200, 230], 8, 8.0, 1.3, 0.0);
            v.push(C([60, 200, 230], 0.0, 0.0, 4.0, 0.0));
            v.push(C([220, 250, 255], 0.0, 0.0, 2.0, 0.0));
            v
        }
        Item::Cannon => vec![
            C([28, 28, 34], 0.0, 1.0, 8.0, 0.0),
            C([110, 112, 124], -3.0, -2.0, 2.5, 0.0),
            L([150, 100, 60], 4.0, -5.0, 8.0, -10.0, 1.5),
            C([255, 180, 40], 8.0, -10.0, 1.8, 0.0),
        ],
        Item::Arty => {
            [-7.0f32, 0.0, 7.0].iter().enumerate().map(|(j, &dx)| C(RED, dx, 3.0 - 3.0 * j as f32, 3.2, 1.2)).collect()
        }
        Item::Trap => vec![
            C([120, 124, 136], 0.0, 0.0, 9.0, 1.6),
            Pl(
                [60, 62, 70],
                (0..7).map(|j| (-8.0 + j as f32 * 2.7, if j % 2 == 1 { -3.0 } else { 3.0 })).collect(),
                1.6,
            ),
            C([200, 40, 40], 0.0, 0.0, 2.0, 0.0),
        ],
        Item::Jump => vec![
            P(
                [60, 150, 245],
                vec![(0.0, -10.0), (7.0, -2.0), (3.0, -2.0), (3.0, 4.0), (-3.0, 4.0), (-3.0, -2.0), (-7.0, -2.0)],
                0.0,
            ),
            L([255, 150, 40], -2.0, 6.0, -2.0, 10.0, 2.0),
            L([255, 150, 40], 2.0, 6.0, 2.0, 10.0, 2.0),
        ],
        Item::Frost => {
            let mut v = vec![C(ICE, 0.0, 0.0, 9.0, 1.0)];
            for &(x, y) in round_pts(6, 7.0, None, PI / 6.0).iter().take(3) {
                v.push(L([40, 130, 220], -x, -y, x, y, 2.0));
            }
            v.push(C([240, 250, 255], 0.0, 0.0, 2.0, 0.0));
            v
        }
        Item::Orbit => vec![
            L([120, 220, 255], 0.0, 0.0, 0.0, 10.0, 3.0),
            R([70, 74, 90], -3.0, -8.0, 6.0, 7.0, None),
            R([60, 110, 220], -10.0, -7.0, 6.0, 4.0, None),
            R([60, 110, 220], 4.0, -7.0, 6.0, 4.0, None),
        ],
        Item::Prism => {
            let mut v: Vec<Sh> = PRISM_COLORS
                .iter()
                .zip([-0.45f32, 0.0, 0.45])
                .map(|(c, a)| L(*c, 2.0, 0.0, 2.0 + a.cos() * 9.0, a.sin() * 9.0, 1.6))
                .collect();
            v.push(P([200, 225, 255], vec![(-4.0, -8.0), (4.0, 6.0), (-9.0, 6.0)], 0.0));
            v.push(P([90, 110, 160], vec![(-4.0, -8.0), (4.0, 6.0), (-9.0, 6.0)], 1.0));
            v
        }
        Item::Mirror => vec![
            P([205, 212, 228], hex.clone(), 0.0),
            P([110, 120, 140], hex, 1.2),
            L([255, 255, 255], -4.0, 2.0, 2.0, -5.0, 1.6),
        ],
        Item::Star => vec![P([255, 205, 40], star.clone(), 0.0), P([150, 100, 10], star, 1.0)],
        Item::Giant => {
            let cap: Vec<(f32, f32)> =
                (0..13).map(|j| j as f32 * PI / 12.0).map(|a| (a.cos() * 10.0, 1.0 - a.sin() * 10.0)).collect();
            let mut v = vec![R([240, 225, 195], -3.5, 0.0, 7.0, 9.0, None), P([50, 160, 60], cap, 0.0)];
            for (dx, dy) in [(-5.0, -4.0), (3.0, -6.0), (6.0, -1.0)] {
                v.push(C([225, 255, 210], dx, dy, 1.6, 0.0));
            }
            v
        }
        Item::Rapid => {
            let mut v = Vec::new();
            for j in 0..3 {
                let j = j as f32;
                v.push(L([240, 170, 30], -11.0 + 6.0 * j, 4.0 - 4.0 * j, -7.0 + 6.0 * j, 4.0 - 4.0 * j, 1.2));
                v.push(C(SHOT, -5.0 + 6.0 * j, 4.0 - 4.0 * j, 2.2, 0.0));
            }
            v
        }
        Item::Extra => {
            let mut v = dots([140, 146, 160], 8, 7.5, 2.2, 0.0);
            v.push(C([140, 146, 160], 0.0, 0.0, 7.0, 0.0));
            v.push(C([230, 60, 70], 0.0, 0.0, 3.0, 0.0));
            v
        }
        Item::Slow => vec![
            P([150, 90, 210], vec![(-6.0, -9.0), (6.0, -9.0), (0.0, 0.0)], 0.0),
            P([150, 90, 210], vec![(-6.0, 9.0), (6.0, 9.0), (0.0, 0.0)], 0.0),
            L([80, 40, 120], -7.0, -9.0, 7.0, -9.0, 1.5),
            L([80, 40, 120], -7.0, 9.0, 7.0, 9.0, 1.5),
        ],
        Item::Twin => {
            let mut v = Vec::new();
            for dy in [-4.0, 4.0] {
                v.push(L(SHOT, -9.0, dy, 4.0, dy, 2.0));
                v.push(C(SHOT, 6.0, dy, 2.5, 0.0));
            }
            v
        }
        Item::Tp => vec![E([150, 60, 210], 0.0, 0.0, 14.0, 19.0, 2.5), E([210, 160, 255], 0.0, 0.0, 7.0, 10.0, 0.0)],
        Item::Disco => {
            let mut v = vec![C([190, 196, 214], 0.0, 0.0, 9.0, 0.0)];
            for d in [-4.0, 0.0, 4.0] {
                v.push(L([110, 116, 136], -8.0, d, 8.0, d, 0.5));
                v.push(L([110, 116, 136], d, -8.0, d, 8.0, 0.5));
            }
            v.push(C([255, 255, 255], -3.0, -3.0, 2.0, 0.0));
            v
        }
        Item::Radar => vec![
            C([20, 60, 30], 0.0, 0.0, 9.0, 0.0),
            C([80, 230, 110], 0.0, 0.0, 9.0, 1.3),
            C([80, 230, 110], 0.0, 0.0, 4.5, 0.5),
            L([170, 255, 180], 0.0, 0.0, 6.0, -6.0, 1.6),
            C([255, 90, 80], -4.0, 3.0, 1.5, 0.0),
        ],
        Item::Meteor => vec![
            L([255, 120, 30], 2.0, -2.0, 10.0, -10.0, 4.0),
            L([255, 200, 60], 2.0, -2.0, 7.0, -7.0, 2.5),
            C([110, 80, 60], -2.0, 2.0, 6.0, 0.0),
            C([70, 50, 40], -4.0, 3.0, 1.8, 0.0),
        ],
        Item::Gold => vec![
            C([150, 100, 10], 0.0, 0.0, 8.0, 0.0),
            C([255, 205, 50], 0.0, 0.0, 7.0, 0.0),
            C([255, 245, 190], -2.0, -2.0, 2.5, 0.0),
        ],
        Item::Shear => vec![
            C([70, 74, 86], 0.0, 0.0, 2.0, 0.0),
            L([200, 205, 215], -8.0, -6.0, 6.0, 6.0, 2.2),
            L([200, 205, 215], -8.0, 6.0, 6.0, -6.0, 2.2),
        ],
        Item::Ring => vec![
            C([255, 170, 60], 0.0, 0.0, 9.0, 0.0),
            C([30, 24, 20], 0.0, 0.0, 5.5, 0.0),
            C([255, 210, 120], 0.0, 0.0, 2.5, 0.0),
        ],
        Item::Dist => vec![
            C([90, 96, 110], 0.0, 0.0, 7.0, 0.0),
            C([250, 220, 60], 0.0, 0.0, 3.0, 0.0),
            C([255, 250, 200], -1.0, -1.0, 1.2, 0.0),
        ],
        Item::Eater => vec![
            C([110, 40, 140], 0.0, 0.0, 8.0, 0.0),
            C([24, 10, 30], 0.0, 0.0, 3.5, 0.0),
            C([220, 120, 255], -2.5, -2.5, 1.5, 0.0),
        ],
        Item::Double => vec![
            C([255, 120, 60], 0.0, 0.0, 7.0, 0.0),
            C([255, 230, 200], -2.0, -2.0, 2.0, 0.0),
            C([255, 120, 60], 5.0, 0.0, 2.5, 0.0),
        ],
        Item::Corner => vec![
            L([120, 220, 255], -8.0, 6.0, 0.0, 6.0, 2.4),
            L([120, 220, 255], 0.0, 6.0, 0.0, -6.0, 2.4),
            C([230, 250, 255], 0.0, -6.0, 2.5, 0.0),
        ],
        Item::Chain => vec![
            C([200, 200, 220], -5.0, 0.0, 3.0, 0.0),
            C([200, 200, 220], 5.0, 0.0, 3.0, 0.0),
            L([150, 150, 170], -5.0, 0.0, 5.0, 0.0, 1.6),
        ],
        Item::Hammer => vec![
            L([120, 90, 60], -2.0, 6.0, 4.0, -6.0, 2.4),
            C([90, 90, 100], 5.0, -7.0, 4.0, 0.0),
        ],
        Item::Pause => vec![
            C([120, 170, 255], 0.0, 0.0, 8.0, 0.0),
            L([240, 240, 255], -3.0, -4.0, -3.0, 4.0, 2.0),
            L([240, 240, 255], 3.0, -4.0, 3.0, 4.0, 2.0),
        ],
        Item::Decoy => vec![
            C([255, 220, 60], 0.0, 0.0, 8.0, 0.0),
            C([255, 250, 200], -2.5, -2.5, 2.0, 0.0),
        ],
        Item::Shadow => vec![
            C([40, 40, 52], 0.0, 0.0, 8.0, 0.0),
            C([90, 90, 110], 0.0, 0.0, 3.0, 0.0),
        ],
        Item::Twist => vec![
            C([255, 120, 255], 0.0, 0.0, 9.0, 0.0),
            A([255, 120, 255], 0.0, 0.0, 6.0, -PI * 0.7, PI * 0.7, 2.0),
        ],
        Item::Switch => vec![
            L([120, 200, 255], -6.0, 7.0, 6.0, -7.0, 2.2),
            C([230, 250, 255], 6.0, -7.0, 2.2, 0.0),
        ],
        Item::Afterburn => vec![
            L([255, 140, 40], -8.0, 0.0, 8.0, 0.0, 3.0),
            C([255, 230, 160], 8.0, 0.0, 2.2, 0.0),
        ],
        Item::Thorn => vec![
            L([200, 200, 210], -7.0, 4.0, 0.0, -6.0, 2.0),
            L([200, 200, 210], 0.0, 4.0, 7.0, -6.0, 2.0),
            C([120, 120, 130], 0.0, 6.0, 2.5, 0.0),
        ],
        Item::Plate => vec![
            C([120, 110, 90], 0.0, 0.0, 9.0, 0.0),
            C([255, 200, 80], 0.0, 0.0, 5.0, 0.0),
        ],
        Item::Pincer => vec![
            L([255, 140, 60], -7.0, -4.0, 0.0, 3.0, 2.2),
            L([255, 140, 60], 7.0, -4.0, 0.0, 3.0, 2.2),
            C([255, 200, 120], 0.0, 4.0, 2.0, 0.0),
        ],
        Item::Cross => vec![
            L([255, 70, 70], -8.0, 0.0, 8.0, 0.0, 1.6),
            L([255, 70, 70], 0.0, -8.0, 0.0, 8.0, 1.6),
            C([255, 70, 70], 0.0, 0.0, 3.0, 0.0),
        ],
        Item::Pendel => vec![
            C([70, 74, 86], 0.0, 0.0, 3.0, 0.0),
            L([200, 200, 220], 0.0, 0.0, 9.0, 0.0, 2.8),
        ],
        Item::Verdict => vec![
            C([70, 70, 90], 0.0, 0.0, 8.0, 0.0),
            C([255, 80, 80], 0.0, 0.0, 4.0, 0.0),
            L([255, 255, 255], -2.0, -6.0, -2.0, 6.0, 1.4),
        ],
        Item::Catcher => vec![
            C([120, 80, 170], 0.0, 0.0, 8.5, 0.0),
            C([200, 170, 255], 0.0, 0.0, 4.5, 0.0),
        ],
        Item::Parry => vec![
            L([200, 220, 240], -7.0, 6.0, 7.0, -6.0, 2.6),
            L([120, 120, 130], -3.0, 2.0, 3.0, 2.0, 2.2),
        ],
        Item::Rust => vec![
            C([170, 110, 60], 0.0, 0.0, 8.0, 0.0),
            C([120, 170, 90], -3.0, -2.0, 3.0, 0.0),
        ],
        Item::Cushion => vec![
            C([90, 200, 170], 0.0, 0.0, 8.0, 0.0),
            C([200, 255, 240], -2.0, -2.0, 2.2, 0.0),
        ],
        Item::Cracker => vec![
            C([60, 60, 60], 0.0, 0.0, 6.5, 0.0),
            C([255, 200, 60], 0.0, 0.0, 2.5, 0.0),
        ],
        Item::Disarm => vec![
            C([240, 200, 60], 0.0, 0.0, 8.0, 0.0),
            L([30, 30, 30], -5.0, -5.0, 5.0, 5.0, 2.4),
            L([30, 30, 30], -5.0, 5.0, 5.0, -5.0, 2.4),
        ],
        _ => return None,
    })
}

fn draw_shapes(cv: &mut Canvas, cx: f32, cy: f32, k: f32, shapes: &[Sh]) {
    let at = |x: f32, y: f32| (cx + x * k, cy + y * k);
    let px = |v: f32| (v * k).max(1.0);
    for s in shapes {
        match s {
            Sh::C(c, x, y, r, w) => {
                let (x, y) = at(*x, *y);
                if *w > 0.0 {
                    cv.ring(x, y, px(*r), px(*w), rgb(*c));
                } else {
                    cv.circle(x, y, px(*r), rgb(*c));
                }
            }
            Sh::L(c, x0, y0, x1, y1, w) => {
                let (a, b) = (at(*x0, *y0), at(*x1, *y1));
                cv.line(a.0, a.1, b.0, b.1, px(*w), rgb(*c));
            }
            Sh::P(c, pts, w) => {
                let pts: Vec<_> = pts.iter().map(|&(x, y)| at(x, y)).collect();
                if *w > 0.0 {
                    cv.poly_line(&pts, px(*w), rgb(*c));
                } else {
                    cv.poly(&pts, rgb(*c));
                }
            }
            Sh::Pl(c, pts, w) => {
                let pts: Vec<_> = pts.iter().map(|&(x, y)| at(x, y)).collect();
                cv.polyline(&pts, px(*w), rgb(*c));
            }
            Sh::E(c, x, y, w, h, lw) => {
                let (x, y) = at(*x, *y);
                if *lw > 0.0 {
                    cv.ellipse_line(x, y, px(*w) / 2.0, px(*h) / 2.0, px(*lw), rgb(*c));
                } else {
                    cv.ellipse(x, y, px(*w) / 2.0, px(*h) / 2.0, rgb(*c));
                }
            }
            Sh::R(c, x, y, w, h, border) => {
                let (x, y) = at(*x, *y);
                cv.rect(x, y, px(*w), px(*h), rgb(*c));
                if let Some(b) = border {
                    cv.rrect_line(x, y, px(*w), px(*h), 0.0, px(0.8), rgb(*b));
                }
            }
            Sh::A(c, x, y, r, a0, a1, w) => {
                let (x, y) = at(*x, *y);
                cv.arc(x, y, *r * k, *a0, *a1, px(*w), rgb(*c));
            }
            Sh::Rk(c, x, y, a, size, flame) => {
                let (x, y) = at(*x, *y);
                rocket_cv(cv, x, y, *a, *c, size * k, *flame * k);
            }
        }
    }
}

/// Power-up-Symbol, k = Skalierung
pub fn draw_icon(cv: &mut Canvas, item: Item, cx: f32, cy: f32, k: f32) {
    let s = SHOT;
    let p = |v: f32| v * k;
    match item {
        Item::Frag => {
            cv.circle(cx, cy, p(6.0), rgb(s));
            for i in 0..6 {
                let a = i as f32 * PI / 3.0;
                cv.circle(cx + a.cos() * p(9.0), cy + a.sin() * p(9.0), p(1.6), rgb(s));
            }
        }
        Item::Mg => {
            for dx in [-6.0, 0.0, 6.0] {
                cv.circle(cx + p(dx), cy, p(2.6), rgb(s));
            }
        }
        Item::Laser => {
            let pts = [
                (cx - p(9.0), cy + p(4.0)),
                (cx - p(3.0), cy - p(4.0)),
                (cx + p(3.0), cy + p(4.0)),
                (cx + p(9.0), cy - p(4.0)),
            ];
            cv.polyline(&pts, p(2.5), rgb([230, 20, 20]));
        }
        Item::Rocket => rocket_cv(cv, cx + 2.0 * k, cy - 2.0 * k, -FRAC_PI_4, [235, 120, 20], 0.7 * k, 4.0 * k),
        Item::Shield => cv.ring(cx, cy, p(8.0), p(2.5), rgb([40, 120, 230])),
        Item::Shotgun => {
            for dy in [-7.0, 0.0, 7.0] {
                let end = (cx + p(8.0), cy + p(dy));
                cv.line(cx - p(8.0), cy, end.0, end.1, p(1.5), rgb(s));
                cv.circle(end.0, end.1, p(2.0), rgb(s));
            }
        }
        Item::Bouncy => {
            cv.circle(cx, cy, p(7.0), rgb([235, 70, 160]));
            cv.circle(cx - p(2.0), cy - p(2.0), p(2.0), rgb([255, 200, 230]));
        }
        Item::Mine => {
            for a in [0.8f32, 2.4, 3.9, 5.5] {
                cv.line(cx, cy, cx + a.cos() * p(9.0), cy + a.sin() * p(9.0), p(2.0), rgb(s));
            }
            cv.circle(cx, cy, p(6.0), rgb([60, 60, 60]));
            cv.circle(cx, cy, p(2.0), rgb([240, 40, 40]));
        }
        Item::Swap => {
            let c = rgb([150, 60, 210]);
            cv.line(cx - p(8.0), cy - p(4.0), cx + p(6.0), cy - p(4.0), p(2.0), c);
            cv.poly(&[(cx + p(9.0), cy - p(4.0)), (cx + p(4.0), cy - p(8.0)), (cx + p(4.0), cy)], c);
            cv.line(cx + p(8.0), cy + p(4.0), cx - p(6.0), cy + p(4.0), p(2.0), c);
            cv.poly(&[(cx - p(9.0), cy + p(4.0)), (cx - p(4.0), cy), (cx - p(4.0), cy + p(8.0))], c);
        }
        Item::Rang => {
            let pts = [(cx - p(8.0), cy + p(5.0)), (cx, cy - p(6.0)), (cx + p(8.0), cy + p(5.0))];
            cv.polyline(&pts, p(3.5), rgb([165, 100, 35]));
        }
        Item::Ice | Item::Frozen => {
            for i in 0..3 {
                let a = i as f32 * PI / 3.0;
                let (dx, dy) = (a.cos() * p(8.0), a.sin() * p(8.0));
                cv.line(cx - dx, cy - dy, cx + dx, cy + dy, p(2.0), rgb([60, 160, 235]));
            }
            cv.circle(cx, cy, p(2.0), rgb([230, 245, 255]));
        }
        Item::Hole => {
            cv.circle(cx, cy, p(8.0), rgb([10, 10, 15]));
            cv.ring(cx, cy, p(8.0), p(2.0), rgb([150, 70, 220]));
            cv.ring(cx + p(2.0), cy - p(1.0), p(3.0), k.max(1.0), rgb([150, 70, 220]));
        }
        Item::Strike => {
            let c = rgb([220, 30, 30]);
            cv.ring(cx, cy, p(7.0), p(2.0), c);
            cv.line(cx - p(10.0), cy, cx + p(10.0), cy, p(1.5), c);
            cv.line(cx, cy - p(10.0), cx, cy + p(10.0), p(1.5), c);
        }
        Item::Flame => {
            let outer = [
                (cx + p(1.0), cy - p(10.0)),
                (cx + p(7.0), cy - p(1.0)),
                (cx + p(6.0), cy + p(6.0)),
                (cx, cy + p(9.0)),
                (cx - p(6.0), cy + p(6.0)),
                (cx - p(7.0), cy),
                (cx - p(3.0), cy - p(4.0)),
            ];
            cv.poly(&outer, rgb([235, 80, 20]));
            let inner = [(cx, cy - p(3.0)), (cx + p(3.5), cy + p(3.0)), (cx, cy + p(7.0)), (cx - p(3.5), cy + p(3.0))];
            cv.poly(&inner, rgb([255, 205, 60]));
        }
        Item::Zap => {
            let pts = [
                (cx + p(3.0), cy - p(10.0)),
                (cx - p(6.0), cy + p(1.0)),
                (cx - p(0.5), cy + p(1.0)),
                (cx - p(3.0), cy + p(10.0)),
                (cx + p(6.0), cy - p(2.0)),
                (cx + p(1.0), cy - p(2.0)),
            ];
            cv.poly(&pts, rgb([40, 150, 255]));
            cv.poly_line(&pts, p(1.0).max(1.0), rgb([15, 60, 140]));
        }
        Item::Nuke => {
            cv.circle(cx, cy, p(10.0), rgb([250, 205, 30]));
            cv.ring(cx, cy, p(10.0), p(1.2).max(1.0), rgb([40, 34, 10]));
            for j in 0..3 {
                let a0 = j as f32 * TAU / 3.0 - FRAC_PI_2 - PI / 6.0;
                let arc: Vec<f32> = (0..7).map(|q| a0 + PI / 3.0 * q as f32 / 6.0).collect();
                let mut pts: Vec<(f32, f32)> =
                    arc.iter().map(|a| (cx + a.cos() * p(8.5), cy + a.sin() * p(8.5))).collect();
                pts.extend(arc.iter().rev().map(|a| (cx + a.cos() * p(2.8), cy + a.sin() * p(2.8))));
                cv.poly(&pts, rgb([25, 22, 10]));
            }
            cv.circle(cx, cy, p(1.8), rgb([25, 22, 10]));
        }
        Item::Turbo => {
            for dx in [-4.0, 3.0] {
                let pts = [(cx + p(dx - 3.0), cy - p(6.0)), (cx + p(dx + 3.0), cy), (cx + p(dx - 3.0), cy + p(6.0))];
                cv.polyline(&pts, p(3.0), rgb([245, 130, 20]));
            }
        }
        Item::Mini => {
            cv.rect(cx - p(3.0), cy, p(6.0), p(8.0), rgb([240, 225, 195]));
            cv.half_circle_top(cx, cy + p(1.0), p(9.0), rgb([215, 40, 40]));
            for (dx, dy) in [(-4.0, -4.0), (3.0, -5.0), (5.0, -1.0)] {
                cv.circle(cx + p(dx), cy + p(dy), p(1.5), rgb([255, 255, 255]));
            }
        }
        Item::Ghost => {
            let body = rgb([250, 250, 250]);
            cv.circle(cx, cy - p(2.0), p(7.0), body);
            cv.rect(cx - p(7.0), cy - p(2.0), p(14.0), p(8.0), body);
            for dx in [-5.0, 0.0, 5.0] {
                cv.circle(cx + p(dx), cy + p(6.0), p(2.4), body);
            }
            cv.ring(cx, cy, p(7.0), k.max(1.0), rgb([120, 120, 130]));
            for dx in [-3.0, 3.0] {
                cv.circle(cx + p(dx), cy - p(2.0), p(1.5), rgb(s));
            }
        }
        Item::Invis => {
            for j in 0..10 {
                let a = j as f32 * PI / 5.0;
                cv.circle(cx + a.cos() * p(8.0), cy + a.sin() * p(8.0), p(1.6), rgb([110, 120, 140]));
            }
        }
        Item::Confuse => {
            let pts: Vec<(f32, f32)> = (0..28)
                .map(|j| {
                    let j = j as f32;
                    (cx + (j * 0.5).cos() * (1.0 + j * 0.3) * k, cy + (j * 0.5).sin() * (1.0 + j * 0.3) * k)
                })
                .collect();
            cv.polyline(&pts, p(2.0), rgb([225, 175, 20]));
        }
        Item::Mystery => {
            // Fragezeichen aus Formen (ohne Schrift)
            let c = rgb([110, 70, 0]);
            cv.arc(cx, cy - p(3.0), p(5.5), -0.6, PI + 0.3, p(3.0), c);
            cv.line(cx + p(4.6), cy - p(1.0), cx, cy + p(3.0), p(3.0), c);
            cv.line(cx, cy + p(3.0), cx, cy + p(4.5), p(3.0), c);
            cv.circle(cx, cy + p(9.0), p(1.9), c);
        }
        Item::Potato => {
            cv.ellipse(cx, cy, p(10.0), p(6.5), rgb([175, 120, 60]));
            cv.ellipse_line(cx, cy, p(10.0), p(6.5), p(1.5), rgb([110, 70, 30]));
            for (dx, dy) in [(-4.0, -2.0), (2.0, 2.0), (5.0, -2.0)] {
                cv.circle(cx + p(dx), cy + p(dy), p(1.0), rgb([110, 70, 30]));
            }
        }
        _ => {
            if let Some(shapes) = icon_shapes(item) {
                draw_shapes(cv, cx, cy, k, &shapes);
            }
        }
    }
}

/// Kiste: helle Platte mit Symbol in einem Rahmen in der Farbe der Kategorie
pub fn box_canvas(item: Item, s: f32) -> Canvas {
    let n = 32.0;
    let mut cv = Canvas::new((n * s) as usize, (n * s) as usize);
    let frame = box_color(item);
    cv.rrect(0.0, 0.0, n * s, n * s, 8.0 * s, rgb([14, 15, 19]));
    cv.rrect(s, s, (n - 2.0) * s, (n - 2.0) * s, 7.0 * s, rgb(frame));
    cv.rrect(s, s, (n - 2.0) * s, 6.0 * s, 4.0 * s, rgb(mix(frame, [255, 255, 255], 0.45)));
    cv.rect(s, 4.0 * s, (n - 2.0) * s, 1.5 * s, rgb(frame));
    let face = if item == Item::Mystery { [255, 232, 140] } else { [230, 234, 242] };
    cv.rrect(4.0 * s, 4.0 * s, (n - 8.0) * s, (n - 8.0) * s, 4.0 * s, rgb(shade(face, 0.72)));
    cv.rrect(4.0 * s, 3.5 * s, (n - 8.0) * s, (n - 9.0) * s, 4.0 * s, rgb(face));
    for (x, y) in [(2.7, 2.7), (n - 2.7, 2.7), (2.7, n - 2.7), (n - 2.7, n - 2.7)] {
        cv.circle(x * s, y * s, 1.1 * s, rgb(shade(frame, 0.55))); // Nieten
    }
    draw_icon(&mut cv, item, n * s / 2.0, n * s / 2.0, 0.88 * s);
    cv
}

/// Symbol auf heller Plakette (HUD, über dem Panzer)
pub fn badge_canvas(item: Item, s: f32) -> Canvas {
    let n = 24.0;
    let mut cv = Canvas::new((n * s) as usize, (n * s) as usize);
    let c = n * s / 2.0;
    cv.circle(c, c, 12.0 * s, rgb([14, 15, 19]));
    cv.circle(c, c, 11.0 * s, rgb([228, 232, 240]));
    cv.ring(c, c, 11.0 * s, 2.0 * s, rgb(box_color(item)));
    draw_icon(&mut cv, item, c, c, 0.72 * s);
    cv
}

/// Alle fertigen Texturen
pub struct Sprites {
    /// Auflösung der Sprites (4 = 4-fach, wird beim Zeichnen verkleinert)
    pub s: f32,
    pub tanks: Vec<Texture2D>,
    pub wrecks: Vec<Texture2D>,
    pub turrets: Vec<Texture2D>,
    pub turret_wrecks: Vec<Texture2D>,
    pub rockets: HashMap<[u8; 3], Texture2D>,
    pub boxes: HashMap<Item, Texture2D>,
    /// Symbol ohne Plakette (24 Einheiten)
    pub icons: HashMap<Item, Texture2D>,
    pub badges: HashMap<Item, Texture2D>,
    pub jet: Texture2D,
    pub ice: Texture2D,
    pub crown: Texture2D,
    pub falloff: Texture2D,
    pub puff: Texture2D,
    pub shadow: Texture2D,
    pub scorch: Texture2D,
    pub cone: Texture2D,
    pub edge: Texture2D,
}

impl Sprites {
    pub fn build() -> Sprites {
        let s = 3.0;
        let tanks = COLORS.iter().map(|&c| tex(&tank_canvas(c, false, s))).collect();
        let wrecks = COLORS.iter().map(|&c| tex(&tank_canvas(c, true, s))).collect();
        let turrets = COLORS.iter().map(|&c| tex(&turret_canvas(c, s))).collect();
        let turret_wrecks = COLORS.iter().map(|&c| tex(&turret_canvas(mix(c, [34, 28, 26], 0.7), s))).collect();
        let mut rockets = HashMap::new();
        for c in COLORS.iter().copied().chain([[40, 200, 110], [255, 90, 90], [235, 120, 20]]) {
            rockets.insert(c, tex(&rocket_canvas(c, s)));
        }
        let mut boxes = HashMap::new();
        let mut icons = HashMap::new();
        let mut badges = HashMap::new();
        for &item in ALL_ITEMS {
            boxes.insert(item, tex(&box_canvas(item, s)));
            let mut cv = Canvas::new((24.0 * s) as usize, (24.0 * s) as usize);
            draw_icon(&mut cv, item, 12.0 * s, 12.0 * s, s);
            icons.insert(item, tex(&cv));
            badges.insert(item, tex(&badge_canvas(item, s)));
        }
        let mut crown = Canvas::new((16.0 * s) as usize, (12.0 * s) as usize);
        crown_cv(&mut crown, 8.0 * s, 6.5 * s, s);
        Sprites {
            s,
            tanks,
            wrecks,
            turrets,
            turret_wrecks,
            rockets,
            boxes,
            icons,
            badges,
            jet: tex(&jet_canvas(2.0)),
            ice: tex(&ice_canvas(120)),
            crown: tex(&crown),
            falloff: tex(&falloff_canvas(64)),
            puff: tex(&puff_canvas(32)),
            shadow: tex(&shadow_canvas(24)),
            scorch: tex(&scorch_canvas(48)),
            cone: tex(&cone_canvas()),
            edge: tex(&edge_canvas()),
        }
    }

    pub fn rocket(&self, c: [u8; 3]) -> &Texture2D {
        self.rockets.get(&c).unwrap_or_else(|| &self.rockets[&COLORS[0]])
    }
}

/// App-Symbol: Panzer schräg auf dunkler, abgerundeter Kachel mit warmem Leuchten
pub fn icon_canvas(size: usize) -> Canvas {
    let n = size as f32;
    let mut cv = Canvas::new(size, size);
    cv.rrect(n * 0.04, n * 0.04, n * 0.92, n * 0.92, n * 0.2, rgb([20, 22, 29]));
    // Leuchten hinter dem Panzer
    let c = n / 2.0;
    for y in 0..size {
        for x in 0..size {
            let d = ((x as f32 + 0.5 - c).hypot(y as f32 + 0.5 - c) / (n * 0.42)).min(1.0);
            let k = (1.0 - d).powi(2) * 0.55;
            let p = &mut cv.px[y * size + x];
            if p[3] > 0.0 {
                p[0] = (p[0] + 1.0 * k * p[3]).min(p[3]);
                p[1] = (p[1] + 0.55 * k * p[3]).min(p[3]);
                p[2] = (p[2] + 0.12 * k * p[3]).min(p[3]);
            }
        }
    }
    cv.rrect_line(n * 0.04, n * 0.04, n * 0.92, n * 0.92, n * 0.2, n * 0.012, rgb([255, 196, 60]));
    let tank = tank_canvas(COLORS[0], false, n * 0.6 / 44.0);
    let rot = crate::raster::rotate_canvas(&tank, FRAC_PI_4);
    cv.blit(&rot, ((n - rot.w as f32) / 2.0) as i64, ((n - rot.h as f32) / 2.0) as i64);
    cv
}

#[cfg(test)]
mod tests {
    /// Erzeugt assets/icon/panzer-1024.png (danach packaging/make_icons.py für .ico/.icns).
    /// cargo test -p panzer --release export_icon -- --ignored
    #[test]
    #[ignore]
    fn export_icon() {
        let cv = super::icon_canvas(1024);
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/icon");
        std::fs::create_dir_all(dir).unwrap();
        let img = macroquad::texture::Image { width: 1024, height: 1024, bytes: cv.to_rgba8() };
        img.export_png(&format!("{dir}/panzer-1024.png"));
    }

    /// Erzeugt website/img/items.png (alle Power-up-Symbole als Raster, Reihenfolge wie ALL_ITEMS)
    /// und die Liste dazu in website/index.html zwischen den power-ups-Markern.
    /// cargo test -p panzer --release export_items -- --ignored
    #[test]
    #[ignore]
    fn export_items() {
        use panzer_core::items::{Item, ALL_ITEMS};
        const CELL: usize = 96;
        const COLS: usize = 10;
        // "Eingefroren" ist nur eine Anzeige, kein Power-up
        let items: Vec<Item> = ALL_ITEMS.iter().copied().filter(|i| *i != Item::Frozen).collect();
        assert_eq!(items.len(), 93, "neue Zahl auch auf Website, im README, Launcher und in build.sh eintragen");
        let rows = items.len().div_ceil(COLS);
        let mut cv = crate::raster::Canvas::new(CELL * COLS, CELL * rows);
        for (i, item) in items.iter().enumerate() {
            let (cx, cy) = ((i % COLS * CELL + CELL / 2) as f32, (i / COLS * CELL + CELL / 2) as f32);
            super::draw_icon(&mut cv, *item, cx, cy, 3.0);
        }
        // export_png spiegelt vertikal, deshalb vorher umdrehen
        let bytes: Vec<u8> = cv.to_rgba8().chunks(CELL * COLS * 4).rev().flatten().copied().collect();
        let img = macroquad::texture::Image { width: (CELL * COLS) as u16, height: (CELL * rows) as u16, bytes };
        img.export_png(concat!(env!("CARGO_MANIFEST_DIR"), "/../../website/img/items.png"));

        // Laufband auf der Startseite: Position im Raster und Name jedes Symbols
        let mut list = format!("<ul class=\"items\" style=\"--rows:{rows}\">\n");
        for (i, item) in items.iter().enumerate() {
            let name = item.name().replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            list += &format!("          <li><span class=\"ico\" style=\"--x:{};--y:{}\"></span>{name}</li>\n", i % COLS, i / COLS);
        }
        list += "        </ul>";
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../website/index.html");
        let html = std::fs::read_to_string(path).unwrap();
        let (start, end) = ("<!-- power-ups:start -->", "<!-- power-ups:end -->");
        let a = html.find(start).expect("Startmarker fehlt in index.html") + start.len();
        let b = html.find(end).expect("Endmarker fehlt in index.html");
        std::fs::write(path, format!("{}\n        {list}\n        {}", &html[..a], &html[b..])).unwrap();
    }
}
