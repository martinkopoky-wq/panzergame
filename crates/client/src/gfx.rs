//! GPU-Helfer: Mischmodi (additives Licht, Multiplizieren, Überschreiben), Kameras für
//! Bildschirm und Welt, Licht-Puffer, Schrift.

use std::f32::consts::TAU;

use macroquad::miniquad::{BlendFactor, BlendState, BlendValue, Equation, PipelineParams};
use macroquad::prelude::*;
use panzer_core::consts::{H, VIEW_H, W};

use crate::sprites::Sprites;

pub const VW: f32 = W as f32;
pub const VH: f32 = H as f32;
pub const VIEW: f32 = VIEW_H as f32;

// Animationen skalieren Glyphen, statt für jeden Zwischenwert einen neuen Atlas-Eintrag anzulegen.
const TEXT_RASTER_SIZE: u16 = 48;
const TITLE_RASTER_SIZE: u16 = 72;

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    uv = texcoord;
}"#;

const FRAGMENT: &str = r#"#version 100
varying lowp vec4 color;
varying lowp vec2 uv;
uniform sampler2D Texture;
void main() {
    gl_FragColor = color * texture2D(Texture, uv);
}"#;

fn material(color: Option<BlendState>, alpha: Option<BlendState>) -> Material {
    load_material(
        ShaderSource::Glsl { vertex: VERTEX, fragment: FRAGMENT },
        MaterialParams {
            pipeline_params: PipelineParams { color_blend: color, alpha_blend: alpha, ..Default::default() },
            ..Default::default()
        },
    )
    .expect("Shader")
}

#[inline]
pub fn col(c: [u8; 3]) -> Color {
    Color::from_rgba(c[0], c[1], c[2], 255)
}

#[inline]
pub fn cola(c: [u8; 3], a: f32) -> Color {
    Color::new(c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, a.clamp(0.0, 1.0))
}

/// Farbe heller/dunkler (für additives Licht = Helligkeit)
#[inline]
pub fn colk(c: [u8; 3], k: f32) -> Color {
    let k = k.clamp(0.0, 4.0);
    Color::new(c[0] as f32 / 255.0 * k, c[1] as f32 / 255.0 * k, c[2] as f32 / 255.0 * k, 1.0)
}

struct Glow {
    c: Color,
    x: f32,
    y: f32,
    r: f32,
}

/// Linie bzw. Ring im Lichtpuffer
enum Lit {
    Line(Color, f32, f32, f32, f32, f32),
    Ring(Color, f32, f32, f32, f32),
}

/// Bildschirm-Layout: virtuelle 1000×680 Pixel, mit schwarzen Balken eingepasst
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub scale: f32,
    pub ox: f32,
    pub oy: f32,
}

impl Layout {
    pub fn now() -> Layout {
        let (w, h) = (screen_width(), screen_height());
        let scale = (w / VW).min(h / VH);
        Layout { scale, ox: (w - VW * scale) / 2.0, oy: (h - VH * scale) / 2.0 }
    }

    /// Maus in virtuellen Koordinaten
    pub fn mouse(&self) -> Vec2 {
        let (mx, my) = mouse_position();
        vec2((mx - self.ox) / self.scale, (my - self.oy) / self.scale)
    }

    fn viewport(&self) -> (i32, i32, i32, i32) {
        (self.ox as i32, self.oy as i32, (VW * self.scale) as i32, (VH * self.scale) as i32)
    }

    /// Kamera für Bildschirmkoordinaten (0..1000, 0..680)
    pub fn screen_camera(&self) -> Camera2D {
        let mut c = Camera2D::from_display_rect(Rect::new(0.0, 0.0, VW, VH));
        c.zoom.y = -c.zoom.y; // from_display_rect steht in macroquad 0.4 auf dem Kopf
        c.viewport = Some(self.viewport());
        c
    }

    /// Kamera für die Welt: src = sichtbarer Weltausschnitt, der aufs Spielfeld
    /// (0..1000, 0..VIEW) kommt, shake in Bildschirmpixeln
    pub fn world_camera(&self, src: Rect, shake: Vec2) -> Camera2D {
        let k = VW / src.w;
        let rect = Rect::new(src.x - shake.x / k, src.y - shake.y / k, src.w, src.h * VH / VIEW);
        let mut c = Camera2D::from_display_rect(rect);
        c.zoom.y = -c.zoom.y;
        c.viewport = Some(self.viewport());
        c
    }
}

/// Kamera, die in ein Render-Target zeichnet (Welt- bzw. Pixelkoordinaten)
pub fn rt_camera(rect: Rect, rt: &RenderTarget) -> Camera2D {
    let mut c = Camera2D::from_display_rect(rect);
    c.render_target = Some(rt.clone());
    c
}

pub struct Gfx {
    pub add: Material,
    pub mul: Material,
    pub replace: Material,
    pub spr: Sprites,
    pub font: Font,
    pub title: Font,
    glows: Vec<Glow>,
    lits: Vec<Lit>,
    /// Stromausfall: alle Lichter dieses Bildes (Farbe, x, y, Radius, Stufe)
    pub light_log: Vec<([u8; 3], f32, f32, f32, f32)>,
    pub dark: bool,
}

impl Gfx {
    pub fn new() -> Gfx {
        let additive = BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One);
        let keep_alpha = BlendState::new(Equation::Add, BlendFactor::Zero, BlendFactor::One);
        let multiply = BlendState::new(Equation::Add, BlendFactor::Zero, BlendFactor::Value(BlendValue::SourceColor));
        let mut font =
            load_ttf_font_from_bytes(include_bytes!("../../../assets/fonts/Roboto-Medium.ttf")).expect("Schrift");
        font.set_filter(FilterMode::Linear);
        let mut title = load_ttf_font_from_bytes(include_bytes!("../../../assets/fonts/RobotoCondensed-Bold.ttf"))
            .expect("Schrift");
        title.set_filter(FilterMode::Linear);
        let characters: Vec<char> = (' '..='~').chain("ÄÖÜäöüß×°·–—…•".chars()).collect();
        let dpi = miniquad::window::dpi_scale();
        font.populate_font_cache(&characters, (TEXT_RASTER_SIZE as f32 * dpi).ceil() as u16);
        title.populate_font_cache(&characters, (TITLE_RASTER_SIZE as f32 * dpi).ceil() as u16);
        Gfx {
            add: material(Some(additive), Some(keep_alpha)),
            mul: material(Some(multiply), Some(keep_alpha)),
            replace: material(None, None),
            spr: Sprites::build(),
            font,
            title,
            glows: Vec::with_capacity(2048),
            lits: Vec::with_capacity(512),
            light_log: Vec::new(),
            dark: false,
        }
    }

    /// Additives Licht an (x, y), level 0..8 = Helligkeit
    pub fn light(&mut self, c: [u8; 3], x: f32, y: f32, r: f32, level: f32) {
        if level < 0.5 || r < 1.0 {
            return;
        }
        let level = level.min(8.0);
        if self.dark {
            self.light_log.push((c, x, y, r, level));
        }
        self.glows.push(Glow { c: colk(c, level / 8.0), x, y, r });
    }

    /// Leuchtende Linie (Laser, Blitz, Funken)
    pub fn lit_line(&mut self, c: Color, x0: f32, y0: f32, x1: f32, y1: f32, w: f32) {
        self.lits.push(Lit::Line(c, x0, y0, x1, y1, w));
    }

    pub fn lit_ring(&mut self, c: Color, x: f32, y: f32, r: f32, w: f32) {
        if r >= w.max(1.0) {
            self.lits.push(Lit::Ring(c, x, y, r, w));
        }
    }

    pub fn lit_polyline(&mut self, c: [u8; 3], k: f32, pts: &[(f32, f32)], w: f32) {
        let color = colk(c, k);
        for p in pts.windows(2) {
            self.lit_line(color, p[0].0, p[0].1, p[1].0, p[1].1, w);
            if self.dark && w > 4.0 {
                // Stromausfall: die Linie beleuchtet ihre Umgebung
                let d = (p[1].0 - p[0].0).hypot(p[1].1 - p[0].1);
                for j in 0..=(d / 40.0) as usize {
                    let f = if d > 0.0 { j as f32 * 40.0 / d } else { 0.0 };
                    let (x, y) = (p[0].0 + (p[1].0 - p[0].0) * f, p[0].1 + (p[1].1 - p[0].1) * f);
                    self.light_log.push((c, x, y, 30.0, 6.0 * k.min(1.0)));
                }
            }
        }
    }

    /// Alle gesammelten Lichter additiv aufs Bild
    pub fn flush_light(&mut self) {
        if self.glows.is_empty() && self.lits.is_empty() {
            return;
        }
        gl_use_material(&self.add);
        let tex = &self.spr.falloff;
        for g in self.glows.drain(..) {
            draw_texture_ex(
                tex,
                g.x - g.r,
                g.y - g.r,
                g.c,
                DrawTextureParams { dest_size: Some(vec2(2.0 * g.r, 2.0 * g.r)), ..Default::default() },
            );
        }
        for l in self.lits.drain(..) {
            match l {
                Lit::Line(c, x0, y0, x1, y1, w) => {
                    draw_line(x0, y0, x1, y1, w, c);
                    if w >= 3.0 {
                        // runde Enden, sonst sieht man Lücken an Knicken
                        draw_circle(x0, y0, w / 2.0, c);
                    }
                }
                Lit::Ring(c, x, y, r, w) => draw_circle_lines(x, y, r - w / 2.0, w, c),
            }
        }
        gl_use_default_material();
    }

    /// Sprite zentriert und gedreht zeichnen. k = Skalierung relativ zur Originalgröße.
    pub fn sprite(&self, t: &Texture2D, x: f32, y: f32, a: f32, k: f32, c: Color) {
        let size = t.size() / self.spr.s * k;
        draw_texture_ex(
            t,
            x - size.x / 2.0,
            y - size.y / 2.0,
            c,
            DrawTextureParams { dest_size: Some(size), rotation: a, ..Default::default() },
        );
    }

    /// Weicher Schatten unter Panzern und Kisten
    pub fn shadow(&self, x: f32, y: f32, r: f32, alpha: f32) {
        let s = r * 4.0;
        draw_texture_ex(
            &self.spr.shadow,
            x - s / 2.0,
            y - s / 2.0,
            Color::new(1.0, 1.0, 1.0, alpha),
            DrawTextureParams { dest_size: Some(vec2(s, s)), ..Default::default() },
        );
    }

    /// Weiche Wolke (Rauch, Staub)
    pub fn puff(&self, x: f32, y: f32, r: f32, c: [u8; 3], alpha: f32) {
        let s = 2.0 * r + 2.0;
        draw_texture_ex(
            &self.spr.puff,
            x - s / 2.0,
            y - s / 2.0,
            cola(c, alpha),
            DrawTextureParams { dest_size: Some(vec2(s, s)), ..Default::default() },
        );
    }

    // --- Schrift ---

    pub fn font(&self, title: bool) -> &Font {
        if title {
            &self.title
        } else {
            &self.font
        }
    }

    pub fn measure(&self, text: &str, size: f32, title: bool) -> TextDimensions {
        let raster = if title { TITLE_RASTER_SIZE } else { TEXT_RASTER_SIZE };
        measure_text(text, Some(self.font(title)), raster, size.max(4.0) / raster as f32)
    }

    /// Einfacher Text, (x, y) = Anker, align: 0 = links, 0.5 = Mitte, 1 = rechts; y = Mitte
    pub fn text(&self, text: &str, x: f32, y: f32, size: f32, c: Color, align: f32) -> f32 {
        let d = self.measure(text, size, false);
        let base = y - d.height / 2.0 + d.offset_y;
        draw_text_ex(
            text,
            x - d.width * align,
            base,
            TextParams {
                font: Some(&self.font),
                font_size: TEXT_RASTER_SIZE,
                font_scale: size.max(4.0) / TEXT_RASTER_SIZE as f32,
                color: c,
                ..Default::default()
            },
        );
        d.width
    }

    /// Text mit dunklem Rand und Schlagschatten. scale skaliert alles, alpha blendet aus.
    #[allow(clippy::too_many_arguments)]
    pub fn fancy(
        &self,
        text: &str,
        cx: f32,
        cy: f32,
        size: f32,
        c: [u8; 3],
        ow: f32,
        title: bool,
        alpha: f32,
        scale: f32,
    ) -> f32 {
        if alpha <= 0.01 || scale <= 0.02 {
            return 0.0;
        }
        let size = (size * scale).max(4.0);
        let font = self.font(title);
        let fs = if title { TITLE_RASTER_SIZE } else { TEXT_RASTER_SIZE };
        let d = self.measure(text, size, title);
        let x = cx - d.width / 2.0;
        let base = cy - d.height / 2.0 + d.offset_y;
        let plain = ow <= 0.0;
        let ow = (ow * scale).max(1.0);
        let sh = 2.0 + size / 22.0;
        let params = |color: Color| TextParams {
            font: Some(font),
            font_size: fs,
            font_scale: size / fs as f32,
            color,
            ..Default::default()
        };
        // UI headings use the existing display face without arcade effects.
        if plain {
            draw_text_ex(text, x, base, params(cola(c, alpha)));
            return d.width;
        }
        draw_text_ex(text, x + sh, base + sh, params(Color::new(0.0, 0.0, 0.0, 0.47 * alpha)));
        let edge = Color::new(12.0 / 255.0, 12.0 / 255.0, 18.0 / 255.0, alpha);
        let n = if ow > 2.0 { 12 } else { 8 };
        for i in 0..n {
            let a = i as f32 * TAU / n as f32;
            draw_text_ex(text, x + a.cos() * ow, base + a.sin() * ow, params(edge));
        }
        // Verlauf von oben (heller) nach unten: zwei Lagen, die obere halb abgeschnitten
        let top = crate::raster::mix(c, [255, 255, 255], 0.45);
        draw_text_ex(text, x, base, params(cola(crate::raster::shade(c, 0.9), alpha)));
        draw_text_ex(text, x, base - 0.6, params(cola(top, alpha * 0.55)));
        d.width
    }

    /// Weiches Leuchten hinter einem Text (additiv)
    pub fn text_glow(&mut self, text: &str, cx: f32, cy: f32, size: f32, c: [u8; 3], level: f32, title: bool) {
        let d = self.measure(text, size, title);
        let n = ((d.width / (d.height.max(1.0) * 0.8)) as usize).max(1);
        for i in 0..n {
            let x = cx - d.width / 2.0 + (i as f32 + 0.5) * d.width / n as f32;
            self.light(c, x, cy, d.height * 1.1, level);
        }
    }
}

/// Bogen (y nach unten, Winkel im Uhrzeigersinn) von a0 nach a1
pub fn arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, w: f32, c: Color) {
    let n = (((a1 - a0).abs() * r / 4.0).ceil() as usize).clamp(3, 96);
    let mut last = (cx + a0.cos() * r, cy + a0.sin() * r);
    for i in 1..=n {
        let a = a0 + (a1 - a0) * i as f32 / n as f32;
        let p = (cx + a.cos() * r, cy + a.sin() * r);
        draw_line(last.0, last.1, p.0, p.1, w, c);
        last = p;
    }
}

/// Gefülltes Polygon (konvex oder sternförmig um den Schwerpunkt)
pub fn poly(pts: &[(f32, f32)], c: Color) {
    if pts.len() < 3 {
        return;
    }
    let (mut mx, mut my) = (0.0, 0.0);
    for p in pts {
        mx += p.0;
        my += p.1;
    }
    let m = vec2(mx / pts.len() as f32, my / pts.len() as f32);
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        draw_triangle(m, vec2(a.0, a.1), vec2(b.0, b.1), c);
    }
}

pub fn poly_lines(pts: &[(f32, f32)], w: f32, c: Color, closed: bool) {
    let n = pts.len();
    if n < 2 {
        return;
    }
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        draw_line(a.0, a.1, b.0, b.1, w, c);
    }
}

/// Eckpunkte eines um a gedrehten Rechtecks (Mitte x, y)
pub fn rot_rect(x: f32, y: f32, w: f32, h: f32, a: f32) -> [(f32, f32); 4] {
    let (c, s) = (a.cos() * 0.5, a.sin() * 0.5);
    [
        (x + c * w - s * h, y + s * w + c * h),
        (x - c * w - s * h, y - s * w + c * h),
        (x - c * w + s * h, y - s * w - c * h),
        (x + c * w + s * h, y + s * w - c * h),
    ]
}

/// Gepunktete Linie (Zielhilfen)
pub fn dotted(pts: &[(f32, f32)], c: Color) {
    let mut carry = 0.0;
    for p in pts.windows(2) {
        let ((x0, y0), (x1, y1)) = (p[0], p[1]);
        let seg = (x1 - x0).hypot(y1 - y0);
        let mut d = carry;
        while d < seg {
            draw_circle(x0 + (x1 - x0) * d / seg, y0 + (y1 - y0) * d / seg, 2.0, c);
            d += 10.0;
        }
        carry = d - seg;
    }
}

pub fn ease_out(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0);
    1.0 - (1.0 - k).powi(3)
}

/// Schießt etwas über das Ziel hinaus und federt zurück (für Pop-Animationen)
pub fn ease_back(k: f32) -> f32 {
    let k = k.clamp(0.0, 1.0) - 1.0;
    1.0 + 2.7 * k.powi(3) + 1.7 * k.powi(2)
}
