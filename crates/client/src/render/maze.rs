//! Labyrinth-Fläche als Textur: Boden + Wände (CPU) plus Spuren, Brandflecken und Trümmer,
//! die direkt hineingestempelt werden und bis zur nächsten Runde liegen bleiben.

use macroquad::prelude::*;
use panzer_core::Arena;

use crate::maze_art::{build_area, build_floor};
use crate::raster::Canvas;

pub struct MazeLayer {
    pub rev: u16,
    pub w: usize,
    pub h: usize,
    floor: Canvas,
    /// RGBA8 der fertigen Fläche (inkl. Spuren)
    px: Vec<u8>,
    pub tex: Texture2D,
    dirty: Vec<(usize, usize, usize, usize)>,
    /// wiederverwendeter Puffer der zusammengezogenen Upload-Rechtecke
    up: Vec<(usize, usize, usize, usize)>,
}

fn put(px: &mut [u8], w: usize, x0: usize, y0: usize, cv: &Canvas, area: (usize, usize, usize, usize)) {
    // area: Ausschnitt in cv (x, y, w, h), landet bei (x0, y0)
    let bytes = cv.to_rgba8();
    for y in 0..area.3 {
        let src = ((area.1 + y) * cv.w + area.0) * 4;
        let dst = ((y0 + y) * w + x0) * 4;
        px[dst..dst + area.2 * 4].copy_from_slice(&bytes[src..src + area.2 * 4]);
    }
}

impl MazeLayer {
    pub fn new(arena: &Arena) -> MazeLayer {
        let floor = build_floor(arena);
        let (w, h) = (floor.w, floor.h);
        let full = build_area(arena, &floor, (0, 0, w, h));
        let mut px = vec![0u8; w * h * 4];
        put(&mut px, w, 0, 0, &full, (0, 0, w, h));
        let tex = Texture2D::from_rgba8(w as u16, h as u16, &px);
        tex.set_filter(FilterMode::Linear);
        MazeLayer { rev: arena.rev, w, h, floor, px, tex, dirty: Vec::new(), up: Vec::new() }
    }

    /// Mitten in der Runde haben sich Wände geändert: nur diese Stellen neu (mit Rand)
    pub fn walls_changed(&mut self, old: &[panzer_core::geom::Wall], new: &Arena) {
        let before: Vec<[i64; 4]> = old.iter().map(|w| w.map(|v| v as i64)).collect();
        let after: Vec<[i64; 4]> = new.walls.iter().map(|w| w.map(|v| v as i64)).collect();
        let changed: Vec<[i64; 4]> = before
            .iter()
            .filter(|w| !after.contains(w))
            .chain(after.iter().filter(|w| !before.contains(w)))
            .copied()
            .collect();
        let (ww, wh) = (self.w as i64, self.h as i64);
        for [x, y, w, h] in changed {
            let r = ((x - 6).clamp(0, ww), (y - 6).clamp(0, wh), (x + w + 10).clamp(0, ww), (y + h + 12).clamp(0, wh));
            let big = ((r.0 - 40).max(0), (r.1 - 40).max(0), (r.2 + 40).min(ww), (r.3 + 40).min(wh));
            if r.2 <= r.0 || r.3 <= r.1 {
                continue;
            }
            let patch = build_area(
                new,
                &self.floor,
                (big.0 as usize, big.1 as usize, (big.2 - big.0) as usize, (big.3 - big.1) as usize),
            );
            let inner = ((r.0 - big.0) as usize, (r.1 - big.1) as usize, (r.2 - r.0) as usize, (r.3 - r.1) as usize);
            put(&mut self.px, self.w, r.0 as usize, r.1 as usize, &patch, inner);
            self.dirty.push((r.0 as usize, r.1 as usize, inner.2, inner.3));
        }
        self.rev = new.rev;
    }

    /// Fläche mit Deckung ins Bild mischen
    fn blend_px(&mut self, x: i64, y: i64, c: [u8; 3], a: f32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h || a <= 0.0 {
            return;
        }
        let i = (y as usize * self.w + x as usize) * 4;
        for k in 0..3 {
            let d = self.px[i + k] as f32;
            self.px[i + k] = (d + (c[k] as f32 - d) * a.min(1.0)) as u8;
        }
    }

    fn mark(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        let x0 = (x0.floor().max(0.0) as usize).min(self.w);
        let y0 = (y0.floor().max(0.0) as usize).min(self.h);
        let x1 = (x1.ceil().max(0.0) as usize).min(self.w);
        let y1 = (y1.ceil().max(0.0) as usize).min(self.h);
        if x1 > x0 && y1 > y0 {
            self.dirty.push((x0, y0, x1 - x0, y1 - y0));
        }
    }

    /// Dicke Linie (Kettenspuren), 2×2 Abtastpunkte
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, w: f32, c: [u8; 3], alpha: f32) {
        let r = w / 2.0;
        let (dx, dy) = (x1 - x0, y1 - y0);
        let l2 = dx * dx + dy * dy;
        let (bx0, by0, bx1, by1) =
            (x0.min(x1) - r - 1.0, y0.min(y1) - r - 1.0, x0.max(x1) + r + 1.0, y0.max(y1) + r + 1.0);
        for y in by0 as i64..=by1 as i64 {
            for x in bx0 as i64..=bx1 as i64 {
                let mut hits = 0;
                for (sx, sy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let (px, py) = (x as f32 + sx, y as f32 + sy);
                    let t = if l2 == 0.0 { 0.0 } else { (((px - x0) * dx + (py - y0) * dy) / l2).clamp(0.0, 1.0) };
                    if (px - x0 - dx * t).powi(2) + (py - y0 - dy * t).powi(2) <= r * r {
                        hits += 1;
                    }
                }
                if hits > 0 {
                    self.blend_px(x, y, c, alpha * hits as f32 / 4.0);
                }
            }
        }
        self.mark(bx0, by0, bx1 + 1.0, by1 + 1.0);
    }

    pub fn dot(&mut self, x: f32, y: f32, r: f32, c: [u8; 3], alpha: f32) {
        for py in (y - r - 1.0) as i64..=(y + r + 1.0) as i64 {
            for px in (x - r - 1.0) as i64..=(x + r + 1.0) as i64 {
                let d = ((px as f32 + 0.5 - x).powi(2) + (py as f32 + 0.5 - y).powi(2)).sqrt();
                let cov = (r + 0.5 - d).clamp(0.0, 1.0);
                self.blend_px(px, py, c, alpha * cov);
            }
        }
        self.mark(x - r - 1.0, y - r - 1.0, x + r + 2.0, y + r + 2.0);
    }

    /// Gefülltes Polygon (Trümmer)
    pub fn poly(&mut self, pts: &[(f32, f32)], c: [u8; 3]) {
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y) in pts {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        for y in y0 as i64..=y1 as i64 {
            for x in x0 as i64..=x1 as i64 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let mut inside = false;
                let mut j = pts.len() - 1;
                for i in 0..pts.len() {
                    let ((xi, yi), (xj, yj)) = (pts[i], pts[j]);
                    if (yi > py) != (yj > py) && px < (xj - xi) * (py - yi) / (yj - yi) + xi {
                        inside = !inside;
                    }
                    j = i;
                }
                if inside {
                    self.blend_px(x, y, c, 1.0);
                }
            }
        }
        self.mark(x0 - 1.0, y0 - 1.0, x1 + 2.0, y1 + 2.0);
    }

    /// Fertige Leinwand (Brandfleck, gelandeter Turm) zentriert hineinmischen
    pub fn stamp(&mut self, cv: &Canvas, cx: f32, cy: f32) {
        let (x0, y0) = ((cx - cv.w as f32 / 2.0) as i64, (cy - cv.h as f32 / 2.0) as i64);
        for y in 0..cv.h {
            for x in 0..cv.w {
                let p = cv.px[y * cv.w + x];
                if p[3] <= 0.004 {
                    continue;
                }
                let c = [(p[0] / p[3] * 255.0) as u8, (p[1] / p[3] * 255.0) as u8, (p[2] / p[3] * 255.0) as u8];
                self.blend_px(x0 + x as i64, y0 + y as i64, c, p[3]);
            }
        }
        self.mark(x0 as f32, y0 as f32, (x0 + cv.w as i64) as f32, (y0 + cv.h as i64) as f32);
    }

    /// Geänderte Stellen auf die GPU
    pub fn upload(&mut self) {
        if self.dirty.is_empty() {
            return;
        }
        // Nahe Rechtecke zusammenziehen: vorher ging entweder alles als ein Rechteck
        // (der gemeinsame Rahmen konnte dann großteils unveränderte Pixel laden) oder
        // alles einzeln. Jetzt paarweise, solange die Rahmenfläche höchstens das
        // Doppelte der Einzelflächen ist.
        let rects = std::mem::take(&mut self.dirty);
        let mut merged = std::mem::take(&mut self.up);
        merged.clear();
        for r in rects {
            let mut done = false;
            for m in merged.iter_mut() {
                let (nx0, ny0) = (m.0.min(r.0), m.1.min(r.1));
                let (nx1, ny1) = ((m.0 + m.2).max(r.0 + r.2), (m.1 + m.3).max(r.1 + r.3));
                if (nx1 - nx0) * (ny1 - ny0) <= 2 * (m.2 * m.3 + r.2 * r.3) {
                    *m = (nx0, ny0, nx1 - nx0, ny1 - ny0);
                    done = true;
                    break;
                }
            }
            if !done {
                merged.push(r);
            }
        }
        for &(x, y, w, h) in &merged {
            if w == 0 || h == 0 {
                continue;
            }
            let mut bytes = Vec::with_capacity(w * h * 4);
            for row in y..y + h {
                let s = (row * self.w + x) * 4;
                bytes.extend_from_slice(&self.px[s..s + w * 4]);
            }
            let img = Image { width: w as u16, height: h as u16, bytes };
            self.tex.update_part(&img, x as i32, y as i32, w as i32, h as i32);
        }
        self.up = merged;
    }
}
