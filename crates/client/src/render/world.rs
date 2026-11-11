//! Spielwelt zeichnen: Kisten, Geschosse, Panzer, Effekte, Nebel und Stromausfall.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use macroquad::prelude::*;
use panzer_core::consts::*;
use panzer_core::items::{Eff, Item, Kind};
use panzer_core::physics::{ahead, bullet_params, fence_ends, first_wall, rail_path, strike_target, tank_scale};
use panzer_core::rng::Rng;
use panzer_core::snap::{FxData, TankView, View};
use panzer_core::Arena;

use super::{Renderer, Roster};
use crate::gfx::*;
use crate::particles::{o, P};
use crate::raster::{mix, shade};
use crate::sprites::{box_color, FW_COLORS, PRISM_COLORS};

const DARKC: [u8; 3] = [14, 14, 18];

/// Zackige Blitzlinie per Mittelpunkt-Verschiebung
fn bolt_points(x0: f32, y0: f32, x1: f32, y1: f32, rnd: &mut Rng, depth: usize) -> Vec<(f32, f32)> {
    let mut pts = vec![(x0, y0), (x1, y1)];
    let mut amp = (x1 - x0).hypot(y1 - y0) * 0.16;
    for _ in 0..depth {
        let mut out = vec![pts[0]];
        for w in pts.windows(2) {
            let ((ax, ay), (bx, by)) = (w[0], w[1]);
            let (nx, ny) = (by - ay, ax - bx);
            let d = nx.hypot(ny).max(1e-3);
            let off = rnd.uniform(-amp as f64, amp as f64) as f32;
            out.push(((ax + bx) / 2.0 + nx / d * off, (ay + by) / 2.0 + ny / d * off));
            out.push((bx, by));
        }
        pts = out;
        amp *= 0.55;
    }
    pts
}

fn pf(p: &[(i16, i16)]) -> Vec<(f32, f32)> {
    p.iter().map(|&(x, y)| (x as f32, y as f32)).collect()
}

/// Rakete: Flamme + Sprite
fn rocket(g: &Gfx, x: f32, y: f32, a: f32, c: [u8; 3], k: f32, flame: f32) {
    if flame > 0.0 {
        let (ca, sa) = (a.cos() * k, a.sin() * k);
        let tf = |px: f32, py: f32| vec2(x + px * ca - py * sa, y + px * sa + py * ca);
        let tail = -12.0;
        draw_triangle(tf(tail, -3.0), tf(tail - flame, 0.0), tf(tail, 3.0), col([255, 140, 25]));
        draw_triangle(tf(tail, -1.6), tf(tail - flame * 0.55, 0.0), tf(tail, 1.6), col([255, 235, 120]));
    }
    g.sprite(g.spr.rocket(c), x, y, a, k, WHITE);
}

impl Renderer {
    pub(super) fn trail(&self, g: &mut Gfx, sid: u32, c: [u8; 3], r: f32, level: f32) {
        let Some(h) = self.hist.get(&sid) else { return };
        if h.len() < 2 {
            return;
        }
        let pts = &h[h.len().saturating_sub(32)..];
        let m = pts.len() as f32;
        for (j, segment) in pts.windows(2).enumerate() {
            let f = (j + 1) as f32 / m;
            draw_line(
                segment[0].0,
                segment[0].1,
                segment[1].0,
                segment[1].1,
                (r * 0.32 * f).max(0.7),
                cola(c, 0.55 * f),
            );
        }
        for (j, &(hx, hy, _)) in pts[..pts.len() - 1].iter().enumerate() {
            let f = (j + 1) as f32 / m;
            g.light(c, hx, hy, r * (0.45 + 0.55 * f), level * f);
        }
    }

    /// Leuchtende Kugel mit Schweif (die meisten neuen Geschosse)
    pub(super) fn ball(&self, g: &mut Gfx, sid: u32, x: f32, y: f32, r: f32, c: [u8; 3], trail: bool) {
        let bright = mix(c, [255, 255, 255], 0.35);
        if trail {
            self.trail(g, sid, bright, r + 3.0, 5.0);
        }
        g.light(bright, x, y, r * 2.5 + 8.0, 6.0);
        draw_circle(x, y, r + 1.0, col(DARKC));
        draw_circle(x, y, r, col(mix(c, [255, 255, 255], 0.6)));
        if r > 2.0 {
            draw_circle(x - r * 0.3, y - r * 0.3, (r / 2.0).floor().max(1.0), WHITE);
        }
    }

    pub(super) fn draw_boxes(&mut self, g: &mut Gfx, view: &View, now: f64) {
        let mut live = Vec::new();
        for b in &view.boxes {
            let key = (b.x as i32, b.y as i32, b.item);
            live.push(key);
            let born = *self.box_seen.entry(key).or_insert(now);
            let k = ((now - born) / 0.45) as f32;
            let (x, y) = (b.x as f32, b.y as f32);
            let c = box_color(b.item);
            let tex = &g.spr.boxes[&b.item].clone();
            if k < 1.0 {
                // fällt von oben herein
                let z = 1.0 + 2.2 * (1.0 - ease_out(k));
                g.shadow(x + 4.0, y + 6.0, 13.0 / z + 2.0, 1.0);
                g.sprite(
                    tex,
                    x,
                    y - 40.0 * (1.0 - ease_out(k)),
                    -(1.0 - k) * FRAC_PI_2,
                    z,
                    Color::new(1.0, 1.0, 1.0, (k * 3.0).min(1.0)),
                );
                continue;
            }
            if self.landed.insert(key) {
                self.parts.burst(P::Dust, x, y, 12.0, 110.0, 0.6, 6.0, o([170, 170, 180]).drag(4.0));
                self.parts.add(P::Ring, x, y, 0.0, 0.0, 0.35, 40.0, o(c));
                self.sound("drop", Some(x), 1.0);
            }
            let t = now as f32;
            let pulse = 0.5 + 0.5 * (t * 4.0 + x * 0.05).sin();
            g.light(c, x, y, 34.0, 2.0 + 3.0 * pulse);
            let bob = (t * 3.0 + x * 0.1).sin() * 2.0;
            g.shadow(x + 3.0, y + 6.0, 13.0, 1.0);
            g.sprite(tex, x, y + bob - 2.0, 0.0, 1.0, WHITE);
            if b.item == Item::Mystery && self.parts.u(0.0, 1.0) < 0.08 {
                let (px, py) = (x + self.parts.u(-14.0, 14.0), y + self.parts.u(-14.0, 14.0));
                self.parts.add(P::Star, px, py, 0.0, -12.0, 0.6, 1.6, o(ACCENT_C));
            }
        }
        self.box_seen.retain(|k, _| live.contains(k));
        self.landed.retain(|k| live.contains(k));
    }

    pub(super) fn draw_shots(&mut self, g: &mut Gfx, view: &View, arena: &Arena, now: f64) {
        let bullet_r = bullet_params(arena.mutator).r as f32;
        let t = now as f32;
        for p in &view.shots {
            let (sid, x, y, owner) = (p.id, p.x as f32, p.y as f32, p.owner);
            let (age, life, ang) = (p.age as f32, p.life as f32, p.ang as f32);
            let c = COLORS[owner];
            match p.kind {
                Kind::B | Kind::Mg | Kind::Sg | Kind::Frg => {
                    let r = match p.kind {
                        Kind::B => bullet_r,
                        Kind::Mg => MG_R as f32,
                        Kind::Sg => 3.0,
                        _ => 2.0,
                    };
                    let hot = match p.kind {
                        Kind::B => c,
                        Kind::Mg => [255, 220, 120],
                        _ => [255, 190, 90],
                    };
                    let bright = mix(hot, [255, 255, 255], 0.35);
                    self.trail(g, sid, bright, r + 3.0, if p.kind == Kind::B { 5.0 } else { 4.0 });
                    g.light(bright, x, y, r * 2.5 + 8.0, 6.0);
                    draw_circle(x, y, r + 1.0, col(DARKC));
                    draw_circle(x, y, r, col(mix(hot, [255, 255, 255], 0.6)));
                    if r > 2.0 {
                        draw_circle(x - r * 0.3, y - r * 0.3, (r / 2.0).floor().max(1.0), WHITE);
                    }
                }
                Kind::Frag => {
                    let blink = (age * (3.0 + 12.0 * age / FRAG_FUSE as f32)) as i32 % 2 == 1;
                    let fr = FRAG_R as f32;
                    draw_circle(x, y, fr + 1.0, col(DARKC));
                    draw_circle(x, y, fr, col([62, 64, 74]));
                    for j in 0..6 {
                        let b = j as f32 * PI / 3.0 + age * 2.0;
                        draw_circle(x + b.cos() * fr, y + b.sin() * fr, 2.0, col([110, 112, 124]));
                    }
                    draw_circle(x - 2.0, y - 2.0, 2.0, col([150, 152, 166]));
                    draw_circle(x, y, 2.0, col(if blink { [255, 60, 60] } else { [90, 20, 20] }));
                    if blink {
                        g.light([255, 50, 40], x, y, 22.0, 6.0);
                    }
                    g.light(c, x, y, 18.0, 2.0);
                }
                Kind::Rocket => {
                    rocket(g, x, y, ang, c, 1.0, 9.0 + 4.0 * (t * 45.0 + sid as f32).sin());
                    let (fx, fy) = (x - ang.cos() * 16.0, y - ang.sin() * 16.0);
                    g.light([255, 140, 50], fx, fy, 30.0, 6.0);
                    g.light([255, 230, 160], fx, fy, 10.0, 8.0);
                }
                Kind::Bnc => {
                    self.trail(g, sid, [255, 90, 190], 9.0, 5.0);
                    g.light([255, 80, 180], x, y, 22.0, 5.0);
                    draw_circle(x, y, BOUNCY_R as f32 + 1.0, col(DARKC));
                    draw_circle(x, y, BOUNCY_R as f32, col([240, 80, 170]));
                    draw_circle(x - 2.0, y - 2.0, 2.0, col([255, 210, 235]));
                    arc(x, y, BOUNCY_R as f32 + 3.0, t * 7.0, t * 7.0 + PI, 1.3, col([255, 169, 221]));
                }
                Kind::Swap => {
                    self.trail(g, sid, [170, 90, 255], 9.0, 5.0);
                    g.light([160, 70, 255], x, y, 24.0, 6.0);
                    draw_circle(x, y, SWAP_R as f32, col([150, 60, 210]));
                    draw_circle(x, y, 2.0, col([230, 200, 255]));
                    g.lit_ring(col([150, 90, 255]), x, y, SWAP_R as f32 + 4.0 + 2.0 * (t * 20.0).sin(), 1.0);
                }
                Kind::Ice => {
                    self.trail(g, sid, [120, 210, 255], 8.0, 5.0);
                    g.light([90, 190, 255], x, y, 20.0, 6.0);
                    let pts = rot_rect(x, y, 12.0, 6.0, t * 9.0 + sid as f32);
                    poly(&pts, col([170, 225, 255]));
                    poly_lines(&pts, 1.0, col([240, 252, 255]), true);
                }
                Kind::Rang => {
                    let a = t * 18.0 + sid as f32;
                    g.light([255, 170, 80], x, y, 16.0, 2.0);
                    for da in [0.0, 2.0] {
                        let end = (x + (a + da).cos() * 9.0, y + (a + da).sin() * 9.0);
                        draw_line(x, y, end.0, end.1, 6.0, col([120, 70, 25]));
                        draw_line(x, y, end.0, end.1, 3.0, col([200, 130, 55]));
                    }
                }
                Kind::Mine | Kind::Trap => {
                    let mine = p.kind == Kind::Mine;
                    // eigene Minen und scharf werdende sieht man, die anderen nur, wenn man genau hinschaut
                    let mut alpha = if age < MINE_ARM as f32 || self.me.is_none() || Some(owner) == self.me {
                        1.0
                    } else if mine {
                        28.0 / 255.0
                    } else {
                        40.0 / 255.0
                    };
                    if self.my_eff.has(Eff::Radar) {
                        alpha = 1.0;
                    }
                    let item = if mine { Item::Mine } else { Item::Trap };
                    let icon = &g.spr.icons[&item];
                    g.sprite(icon, x, y, 0.0, if mine { 0.95 } else { 1.0 }, Color::new(1.0, 1.0, 1.0, alpha));
                    if mine {
                        let lit = age > MINE_ARM as f32 && (t * 3.0 + sid as f32) as i32 % 2 == 1;
                        if lit {
                            draw_circle(x, y, 3.0, cola([255, 120, 120], alpha));
                        }
                        draw_circle_lines(x, y, 10.0, 1.0, cola(c, alpha));
                        if lit && alpha >= 1.0 {
                            g.light([255, 40, 30], x, y, 14.0, 5.0);
                        }
                    }
                }
                Kind::Hole => self.draw_hole(g, x, y, age, t),
                Kind::Strike => self.draw_strike(g, sid, x, y, age, t),
                Kind::Fl => {
                    let k = (age / FLAME_LIFE as f32).min(1.0);
                    let fc = crate::particles::FIRE_GRAD[((k * 5.5) as usize).min(6)];
                    g.light(fc, x, y, 12.0 + 34.0 * k, 8.0 - 3.0 * k);
                    if k < 0.5 {
                        g.light([255, 240, 200], x, y, 8.0 + 10.0 * k, 6.0);
                    }
                }
                Kind::Nuke => self.draw_nuke(g, sid, x, y, age, t),
                Kind::Split => {
                    g.light([255, 150, 60], x, y, 18.0, 4.0);
                    self.ball(g, sid, x, y, 4.0, [255, 140, 50], true);
                    let a = self.heading(sid).unwrap_or(0.0);
                    for side in [-1.0, 1.0] {
                        let b = a + side * 0.65;
                        draw_line(x, y, x - b.cos() * 9.0, y - b.sin() * 9.0, 2.0, col([255, 196, 113]));
                    }
                }
                Kind::Wave => {
                    self.trail(g, sid, [90, 240, 190], 9.0, 8.0);
                    g.light([70, 230, 180], x, y, 20.0, 5.0);
                    draw_circle(x, y, 5.0, col(DARKC));
                    draw_circle(x, y, 4.0, col([150, 255, 215]));
                }
                Kind::Napalm => {
                    self.trail(g, sid, [255, 130, 40], 10.0, 5.0);
                    g.light([255, 120, 30], x, y, 26.0, 6.0);
                    draw_circle(x, y, 6.0, col(DARKC));
                    draw_circle(x, y, 5.0, col([235, 90, 25]));
                    draw_circle(x - 1.5, y - 1.5, 2.0, col([255, 220, 150]));
                }
                Kind::Timebomb => self.draw_timebomb(g, x, y, age, life),
                Kind::Burn => self.draw_burn(g, x, y, age, life),
                Kind::Fence => self.draw_fence(g, arena, x, y, ang, owner, age, life, t),
                Kind::Sprinkler => self.draw_sprinkler(g, x, y, owner, age, ang, t),
                Kind::Hook => {
                    if let Some(tk) = view.tank(owner) {
                        let (ox, oy) = (tk.x as f32, tk.y as f32);
                        draw_line(ox, oy, x, y, 3.0, col(DARKC));
                        draw_line(ox, oy, x, y, 1.5, col([176, 150, 110]));
                    }
                    draw_circle(x, y, 5.0, col(DARKC));
                    draw_circle(x, y, 4.0, col([170, 176, 190]));
                    draw_circle(x, y, 1.5, col(c));
                    let a = self.heading(sid).unwrap_or(ang);
                    for side in [-1.0, 1.0] {
                        let b = a + side * 0.7;
                        let (tx, ty) = (x + b.cos() * 8.0, y + b.sin() * 8.0);
                        draw_line(x, y, tx, ty, 2.5, col([205, 215, 228]));
                        draw_line(tx, ty, tx - a.cos() * 4.0, ty - a.sin() * 4.0, 2.0, col([205, 215, 228]));
                    }
                }
                Kind::Destruct => self.draw_destruct(g, x, y, age, life),
                Kind::Double
                | Kind::Corner
                | Kind::Chain
                | Kind::Hammer
                | Kind::Decoy
                | Kind::Shadow
                | Kind::Twist
                | Kind::Switch
                | Kind::Afterburn
                | Kind::Thorn
                | Kind::Plate
                | Kind::Pincer
                | Kind::Cross
                | Kind::Verdict
                | Kind::Catcher
                | Kind::Rust
                | Kind::Cushion
                | Kind::Cracker
                | Kind::Pendel
                | Kind::Ring
                | Kind::Dist
                | Kind::Eater => self.draw_weapon_shot(g, p, view, now),
                Kind::Nv | Kind::Sb | Kind::Spk | Kind::Spb => {
                    let cc = if p.kind == Kind::Spk { FW_COLORS[sid as usize % FW_COLORS.len()] } else { c };
                    self.ball(g, sid, x, y, if p.kind == Kind::Spk { 2.0 } else { 3.0 }, cc, true);
                }
                Kind::Seek => {
                    self.ball(g, sid, x, y, 4.0, c, true);
                    g.lit_ring(col([255, 70, 60]), x, y, 8.0 + 2.0 * (t * 14.0 + sid as f32).sin(), 1.0);
                }
                Kind::Gold => {
                    self.trail(g, sid, [255, 210, 80], 10.0, 7.0);
                    self.ball(g, sid, x, y, 5.0, [255, 200, 40], false);
                }
                Kind::Phase => {
                    self.trail(g, sid, [80, 220, 255], 10.0, 6.0);
                    g.light([80, 220, 255], x, y, 22.0, 6.0);
                    draw_circle(x, y, 4.0, col([180, 245, 255]));
                    draw_circle_lines(x, y, 6.0, 1.0, col([60, 200, 240]));
                }
                Kind::Cannon => {
                    self.trail(g, sid, [255, 150, 60], 16.0, 4.0);
                    let cr = CANNON_R as f32;
                    g.shadow(x + 3.0, y + 4.0, cr + 2.0, 1.0);
                    draw_circle(x, y, cr + 1.0, col([10, 10, 14]));
                    draw_circle(x, y, cr, col([40, 40, 48]));
                    draw_circle(x - 4.0, y - 4.0, 3.0, col([120, 122, 136]));
                }
                Kind::Saw => {
                    let a = t * 22.0 + sid as f32;
                    let sr = SAW_R as f32;
                    let pts: Vec<(f32, f32)> = (0..16)
                        .map(|j| {
                            let r = if j % 2 == 0 { sr } else { sr - 3.0 };
                            (x + (a + j as f32 * PI / 8.0).cos() * r, y + (a + j as f32 * PI / 8.0).sin() * r)
                        })
                        .collect();
                    g.light([230, 235, 255], x, y, 20.0, 3.0);
                    poly(&pts, col([190, 196, 210]));
                    poly_lines(&pts, 1.0, col([80, 84, 96]), true);
                    draw_circle(x, y, 3.0, col(c));
                }
                Kind::Plasma => {
                    self.trail(g, sid, [90, 255, 170], 14.0, 6.0);
                    g.light([60, 255, 150], x, y, 40.0, 7.0);
                    g.light([220, 255, 235], x, y, 14.0, 8.0);
                    draw_circle(x, y, PLASMA_R as f32, col([120, 255, 190]));
                    draw_circle(x, y, 3.0, col([240, 255, 245]));
                    let mut rnd = Rng::new(sid as u64 * 7 + (t * 20.0) as u64);
                    for _ in 0..3 {
                        let b = rnd.uniform(0.0, TAU as f64) as f32;
                        let pts = bolt_points(x, y, x + b.cos() * 16.0, y + b.sin() * 16.0, &mut rnd, 2);
                        g.lit_polyline([90, 255, 170], 1.0, &pts, 1.0);
                    }
                }
                Kind::Rc | Kind::Mrk | Kind::Fw => {
                    let k = match p.kind {
                        Kind::Rc => 0.9,
                        Kind::Mrk => 0.55,
                        _ => 0.6,
                    };
                    let rc = match p.kind {
                        Kind::Rc => [40, 200, 110],
                        Kind::Mrk => c,
                        _ => [255, 90, 90],
                    };
                    let a = if p.kind == Kind::Fw { self.heading(sid).unwrap_or(0.0) } else { ang };
                    rocket(g, x, y, a, rc, k, (9.0 + 4.0 * (t * 45.0 + sid as f32).sin()) * k);
                    g.light([255, 140, 50], x - a.cos() * 16.0 * k, y - a.sin() * 16.0 * k, 28.0 * k, 6.0);
                    if p.kind == Kind::Rc && (t * 6.0) as i32 % 2 == 1 {
                        g.light([80, 255, 120], x, y, 10.0, 7.0); // Funk-Antenne blinkt
                    }
                }
                Kind::Sticky => {
                    let blink = (age * (4.0 + 8.0 * (age / 3.0).min(1.0))) as i32 % 2 == 1;
                    g.light([120, 255, 80], x, y, 18.0, if blink { 7.0 } else { 4.0 });
                    draw_circle(x, y, 6.0, col([40, 90, 30]));
                    draw_circle(x, y, 5.0, col([110, 220, 70]));
                    draw_circle(x, y, 2.0, col(if blink { [255, 70, 60] } else { [90, 30, 30] }));
                }
                Kind::Gl => {
                    let k = (age / (GL_DIST / GL_SPEED) as f32).min(1.0);
                    let h = 4.0 * k * (1.0 - k);
                    g.shadow(x, y, 5.0, 1.0);
                    let gy = y - 34.0 * h;
                    draw_circle(x, gy, 6.0 + 2.0 * h, col([30, 36, 20]));
                    draw_circle(x, gy, 5.0 + 2.0 * h, col([100, 122, 58]));
                    g.light([255, 120, 60], x, gy, 10.0, 2.0 + 3.0 * ((t * 12.0) as i32 % 2) as f32);
                }
                Kind::Oil | Kind::Oilf => self.draw_oil(g, x, y, p.kind, age, t),
                Kind::Sentry => self.draw_sentry(g, x, y, owner, age, ang, t),
                Kind::Spider => self.draw_spider(g, x, y, owner, age, ang, t),
                Kind::Arty => self.draw_target(g, sid, x, y, age, life, ARTY_BLAST as f32),
                Kind::Meteor => self.draw_meteor(g, sid, x, y, age, life),
                Kind::Orbit => self.draw_orbit(g, sid, x, y, age, ang, t),
                _ => {}
            }
        }
    }

    /// Flugrichtung aus der Geschichte (Feuerwerk hat keinen Winkel im Snapshot)
    pub(super) fn heading(&self, sid: u32) -> Option<f32> {
        let h = self.hist.get(&sid)?;
        if h.len() < 2 {
            return None;
        }
        let (a, b) = (h[h.len() - 2], h[h.len() - 1]);
        Some((b.1 - a.1).atan2(b.0 - a.0))
    }

    fn draw_oil(&self, g: &mut Gfx, x: f32, y: f32, kind: Kind, age: f32, t: f32) {
        let life = if kind == Kind::Oil { OIL_LIFE } else { OIL_BURN } as f32;
        let k = (age / 0.4).min((life - age) / 0.6).min(1.0);
        if k <= 0.0 {
            return;
        }
        let r = (OIL_R as f32 * (0.5 + 0.5 * (age / 0.4).min(1.0))).round().max(3.0);
        g.puff(x, y, r, [16, 14, 20], 200.0 / 255.0 * k);
        for (j, c) in [[90, 60, 140], [40, 110, 120]].iter().enumerate() {
            // Ölfilm schillert
            let j = j as f32;
            let rr = r * (0.6 - 0.2 * j);
            arc(x - 4.0 + 6.0 * j, y - 3.0 + 3.0 * j, rr, -(t * 0.7 + j) - 1.8, -(t * 0.7 + j), 2.0, cola(*c, k));
        }
        if kind == Kind::Oilf {
            g.light([255, 120, 30], x, y, OIL_R as f32 * 2.2, 7.0 * k);
            g.light([255, 220, 120], x, y, OIL_R as f32, 6.0 * k);
        }
    }

    /// Zeitbombe: tickt immer schneller, der Ring zeigt, wie weit sie reicht.
    fn draw_timebomb(&self, g: &mut Gfx, x: f32, y: f32, age: f32, life: f32) {
        let left = (life - age).max(0.0);
        let rate = if left < 1.2 { 14.0 } else { 5.0 };
        let blink = (age * rate) as i32 % 2 == 1;
        g.shadow(x + 2.0, y + 3.0, 10.0, 1.0);
        let icon = &g.spr.icons[&Item::Timebomb];
        g.sprite(icon, x, y, 0.0, 1.0, WHITE);
        super::weapons::field(x, y, TIMEBOMB_BLAST as f32, [255, 116, 84], age, life);
        if blink {
            g.light([255, 60, 40], x, y, 26.0, 6.0);
        }
        g.text(&format!("{}", left.ceil() as i32), x, y - 22.0, 15.0, col(TEXT_C), 0.5);
    }

    /// Napalm: brennende Fläche, das Feuer selbst kommt als Partikel aus events.rs.
    fn draw_burn(&self, g: &mut Gfx, x: f32, y: f32, age: f32, life: f32) {
        let k = (age / 0.3).min((life - age) / 0.6).min(1.0);
        if k <= 0.0 {
            return;
        }
        let r = NAPALM_R as f32;
        g.puff(x, y, r, [34, 16, 8], 0.75 * k);
        draw_circle_lines(x, y, r, 1.5, cola([255, 140, 60], 0.5 * k));
        g.light([255, 110, 30], x, y, r * 1.7, 7.0 * k);
        g.light([255, 215, 120], x, y, r * 0.8, 5.0 * k);
    }

    /// Laserschranke: blinkt, bis sie scharf ist, und kurz bevor sie ausgeht.
    #[allow(clippy::too_many_arguments)]
    fn draw_fence(
        &self,
        g: &mut Gfx,
        arena: &Arena,
        x: f32,
        y: f32,
        ang: f32,
        owner: usize,
        age: f32,
        life: f32,
        t: f32,
    ) {
        let ((x0, y0), (x1, y1)) = fence_ends(arena, x as f64, y as f64, ang as f64);
        let (x0, y0, x1, y1) = (x0 as f32, y0 as f32, x1 as f32, y1 as f32);
        for (px, py) in [(x0, y0), (x1, y1)] {
            draw_circle(px, py, 5.0, col(DARKC));
            draw_circle(px, py, 3.0, col(COLORS[owner]));
        }
        let armed = age > FENCE_ARM as f32;
        if !armed {
            if (t * 10.0) as i32 % 2 == 0 {
                draw_line(x0, y0, x1, y1, 1.0, cola([255, 90, 90], 0.6));
            }
            return;
        }
        if life - age < 1.0 && (t * 12.0) as i32 % 2 == 1 {
            return;
        }
        g.lit_line(col([255, 50, 50]), x0, y0, x1, y1, 4.0);
        draw_line(x0, y0, x1, y1, 2.0, col([255, 215, 215]));
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_sprinkler(&self, g: &mut Gfx, x: f32, y: f32, owner: usize, age: f32, ang: f32, t: f32) {
        let z = ease_back((age / 0.4).min(1.0));
        g.shadow(x + 3.0, y + 4.0, 12.0, 1.0);
        draw_circle(x, y, (11.0 * z).max(1.0), col(DARKC));
        draw_circle(x, y, (10.0 * z).max(1.0), col([66, 70, 84]));
        for side in [0.0, PI] {
            let end = (x + (ang + side).cos() * 15.0 * z, y + (ang + side).sin() * 15.0 * z);
            draw_line(x, y, end.0, end.1, 6.0, col(DARKC));
            draw_line(x, y, end.0, end.1, 3.0, col([150, 156, 172]));
        }
        draw_circle(x, y, (5.0 * z).max(1.0), col(COLORS[owner]));
        g.light([255, 190, 90], x, y, 10.0, 3.0 + 2.0 * (t * 20.0).sin());
    }

    /// Selbstzerstörung: rotes Pulsieren am Panzer, der Ring zeigt die Reichweite.
    fn draw_destruct(&self, g: &mut Gfx, x: f32, y: f32, age: f32, life: f32) {
        let k = (age / life).clamp(0.0, 1.0);
        let blink = (age * (6.0 + 20.0 * k)) as i32 % 2 == 1;
        g.light([255, 40, 30], x, y, 30.0 + 60.0 * k, if blink { 8.0 } else { 4.0 });
        draw_circle_lines(x, y, DESTRUCT_BLAST as f32, 2.0, cola([255, 70, 60], 0.3 + 0.5 * k));
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_sentry(&self, g: &mut Gfx, x: f32, y: f32, owner: usize, age: f32, ang: f32, t: f32) {
        let z = ease_back((age / 0.4).min(1.0));
        g.shadow(x + 3.0, y + 4.0, 12.0, 1.0);
        draw_circle(x, y, (11.0 * z).max(1.0), col([14, 15, 19]));
        draw_circle(x, y, (10.0 * z).max(1.0), col([66, 70, 84]));
        draw_circle_lines(x, y, (9.0 * z).max(1.0), 2.0, col(COLORS[owner]));
        let end = (x + ang.cos() * 15.0 * z, y + ang.sin() * 15.0 * z);
        draw_line(x, y, end.0, end.1, 6.0, col([14, 15, 19]));
        draw_line(x, y, end.0, end.1, 3.0, col([150, 156, 172]));
        draw_circle(x, y, (5.0 * z).max(1.0), col([120, 126, 142]));
        if SENTRY_LIFE as f32 - age > 2.0 || (t * 8.0) as i32 % 2 == 1 {
            g.light([255, 60, 40], x, y, 8.0, 5.0);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_spider(&self, g: &mut Gfx, x: f32, y: f32, owner: usize, age: f32, ang: f32, t: f32) {
        let c = col([30, 26, 30]);
        for side in [-1.0f32, 1.0] {
            for j in 0..4 {
                let wig = (t * 18.0 + j as f32 * 1.7 + if side > 0.0 { 3.0 } else { 0.0 }).sin() * 0.25;
                let a = ang + side * (0.6 + j as f32 * 0.42 + wig);
                let (kx, ky) = (x + a.cos() * 6.0, y + a.sin() * 6.0);
                let (fx, fy) = (x + (a + side * 0.4).cos() * 11.0, y + (a + side * 0.4).sin() * 11.0);
                draw_line(x, y, kx, ky, 2.0, c);
                draw_line(kx, ky, fx, fy, 2.0, c);
            }
        }
        draw_circle(x, y, 6.0, c);
        for side in [-1.0f32, 1.0] {
            let (ex, ey) = (x + (ang + side * 0.5).cos() * 4.0, y + (ang + side * 0.5).sin() * 4.0);
            draw_circle(ex, ey, 1.5, col(COLORS[owner]));
            if age > MINE_ARM as f32 {
                g.light([255, 50, 40], ex, ey, 5.0, 3.0); // scharf: Augen glühen
            }
        }
    }

    /// Zielkreis mit Einschlag-Schatten (Artillerie, Meteor)
    #[allow(clippy::too_many_arguments)]
    fn draw_target(&mut self, g: &mut Gfx, sid: u32, x: f32, y: f32, age: f32, life: f32, blast: f32) {
        let k = (age / life).min(1.0);
        let blink = (age * (4.0 + 14.0 * k)) as i32 % 2 == 1;
        let c = col(if blink { [255, 60, 50] } else { [255, 160, 140] });
        super::weapons::field(x, y, blast, [255, 116, 84], age, life);
        g.light([255, 30, 20], x, y, blast, if blink { 3.0 } else { 1.0 });
        draw_circle_lines(x, y, blast - 1.0, 2.0, c);
        draw_circle_lines(x, y, (blast * (1.0 - k)).round().max(3.0), 1.0, c);
        if life - age < 0.8 && self.marks.insert((sid, 1)) {
            self.sound("whistle", Some(x), 0.6);
        }
        if life - age < 0.45 {
            let fk = 1.0 - (life - age) / 0.45;
            g.puff(x, y, 2.0 + 9.0 * fk, [8, 8, 8], (60.0 + 150.0 * fk) / 255.0);
        }
    }

    /// Meteor: Zielkreis, dann fällt ein glühender Brocken schräg von oben herein
    fn draw_meteor(&mut self, g: &mut Gfx, sid: u32, x: f32, y: f32, age: f32, life: f32) {
        self.draw_target(g, sid, x, y, age, life, METEOR_BLAST as f32);
        let k = 1.0 - (life - age) / 0.7;
        if k < 0.0 {
            return;
        }
        let (mx, my) = (x + 260.0 * (1.0 - k), y - 380.0 * (1.0 - k));
        g.light([255, 120, 30], mx, my, 40.0, 7.0);
        g.light([255, 240, 200], mx, my, 14.0, 8.0);
        draw_circle(mx, my, 8.0, col([90, 60, 44]));
        draw_circle_lines(mx, my, 7.0, 2.0, col([255, 160, 60]));
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_orbit(&mut self, g: &mut Gfx, sid: u32, x: f32, y: f32, age: f32, ang: f32, t: f32) {
        let (od, ot, or) = (ORBIT_DELAY as f32, ORBIT_TIME as f32, ORBIT_R as f32);
        if age < od {
            // Zielerfassung
            let k = age / od;
            let rr = or * (2.4 - 1.4 * k);
            g.light([60, 160, 255], x, y, rr, 1.0 + 3.0 * k);
            for j in 0..3 {
                let a0 = t * 3.0 + j as f32 * 2.09;
                arc(x, y, rr, -a0 - 1.4, -a0, 2.0, col([120, 220, 255]));
            }
            let d = ORBIT_SPEED as f32 * ot;
            dotted(&[(x, y), (x + ang.cos() * d, y + ang.sin() * d)], col([90, 170, 230]));
            if k > 0.5 && self.marks.insert((sid, 2)) {
                self.sound("orbit", Some(x), 1.0);
            }
            return;
        }
        if self.marks.insert((sid, 3)) {
            self.sound("beam", Some(x), 1.0);
            self.shake(0.25, Some((x, y)));
        }
        let gk = ((od + ot - age) / 0.3).min(1.0);
        g.light([80, 170, 255], x, y, or * 3.5, 8.0 * gk);
        g.light([255, 255, 255], x, y, or * 1.3, 8.0 * gk);
        g.lit_ring(col(shade([150, 220, 255], gk)), x, y, or + 4.0 * (t * 30.0).sin(), 3.0);
        draw_circle(x, y, (or * 0.7 * gk).max(1.0), col([235, 248, 255]));
        if self.marks.insert((sid ^ ((age * 10.0) as u32) << 20, 4)) {
            if let Some(m) = self.maze.as_mut() {
                let cv = self.scorch.entry(10).or_insert_with(|| crate::sprites::scorch_canvas(10));
                m.stamp(cv, x, y);
            }
        }
    }

    fn draw_hole(&self, g: &mut Gfx, x: f32, y: f32, age: f32, t: f32) {
        let (travel, time) = (HOLE_TRAVEL as f32, HOLE_TIME as f32);
        if age < travel {
            g.light([150, 60, 255], x, y, 22.0, 5.0);
            draw_circle(x, y, HOLE_R as f32, col([6, 4, 10]));
            draw_circle_lines(x, y, HOLE_R as f32 - 1.0, 2.0, col([170, 90, 255]));
            return;
        }
        let k = ((age - travel) / 0.4).min((travel + time - age) / 0.4).min(1.0);
        if k <= 0.0 {
            return;
        }
        g.light([90, 30, 170], x, y, 130.0 * k, 8.0);
        g.light([200, 120, 255], x, y, 50.0 * k, 6.0);
        g.lit_ring(col([70, 40, 110]), x, y, HOLE_RANGE as f32 * k, 1.0);
        for j in 0..26 {
            // Teilchen, die spiralförmig hineinstürzen
            let ph = (t * 0.55 + j as f32 / 26.0).fract();
            let rr = HOLE_RANGE as f32 * 0.85 * (1.0 - ph) * k;
            let b = j as f32 * 2.39 + t * (1.5 + 5.0 * ph);
            g.light([210, 150, 255], x + b.cos() * rr, y + b.sin() * rr, 4.0, 7.0 * ph);
        }
        for j in 0..3 {
            let rr = (16.0 + 11.0 * j as f32) * k;
            let a0 = t * (6.0 - j as f32) + j as f32 * 2.0;
            arc(x, y, rr, -a0 - 2.2, -a0, 2.0, col([190, 110, 255]));
        }
        let kill = (HOLE_KILL as f32 * k).round().max(1.0);
        draw_circle(x, y, kill, col([2, 1, 4]));
        draw_circle_lines(x, y, kill, 1.0, col([230, 190, 255]));
    }

    fn draw_strike(&mut self, g: &mut Gfx, sid: u32, x: f32, y: f32, age: f32, t: f32) {
        let (delay, blast) = (STRIKE_DELAY as f32, STRIKE_BLAST as f32);
        let k = (age / delay).min(1.0);
        let blink = (age * (4.0 + 14.0 * k)) as i32 % 2 == 1;
        let c = col(if blink { [255, 50, 50] } else { [255, 150, 150] });
        super::weapons::field(x, y, blast, [255, 116, 84], age, delay);
        g.light([255, 30, 20], x, y, blast, if blink { 5.0 } else { 2.0 });
        draw_circle_lines(x, y, blast - 1.0, 2.0, c);
        draw_circle_lines(x, y, (blast * (1.0 - k)).round().max(4.0) - 1.0, 2.0, c);
        for j in 0..4 {
            let b = t * 2.0 + j as f32 * FRAC_PI_2;
            let (p0, p1) = (
                (x + b.cos() * (blast - 9.0), y + b.sin() * (blast - 9.0)),
                (x + b.cos() * (blast + 9.0), y + b.sin() * (blast + 9.0)),
            );
            draw_line(p0.0, p0.1, p1.0, p1.1, 3.0, c);
        }
        draw_line(x - 12.0, y, x + 12.0, y, 2.0, c);
        draw_line(x, y - 12.0, x, y + 12.0, 2.0, c);
        // Der Jet überfliegt das Ziel, danach fällt die Bombe (ihr Schatten wird größer)
        let pk = (age - delay * 0.15) / (delay * 0.6);
        if (0.0..=1.0).contains(&pk) {
            let ang = (sid as f32 * 2.39) % TAU;
            let d = (pk - 0.5) * 1100.0;
            self.jets.push((x + ang.cos() * d, y + ang.sin() * d, ang));
            if self.marks.insert((sid, 5)) {
                self.sound("jet", Some(x), 1.0);
            }
        }
        if age > delay - 0.95 && self.marks.insert((sid, 1)) {
            self.sound("whistle", Some(x), 1.0);
        }
        if age > delay - 0.45 {
            let fk = (age - (delay - 0.45)) / 0.45;
            g.puff(x, y, 3.0 + 10.0 * fk, [8, 8, 8], (60.0 + 150.0 * fk) / 255.0);
        }
    }

    fn draw_nuke(&mut self, g: &mut Gfx, sid: u32, x: f32, y: f32, age: f32, t: f32) {
        let left = NUKE_FUSE as f32 - age;
        let fast = left < 1.2;
        let blink = (age * if fast { 6.0 } else { 2.5 }) as i32 % 2 == 1;
        let warn = if blink { [255, 40, 30] } else { [255, 170, 60] };
        let pulse = 0.5 + 0.5 * (t * if fast { 14.0 } else { 6.0 }).sin();
        let nb = NUKE_BLAST as f32;
        g.light([255, 40, 20], x, y, nb * 0.9, 1.0 + 2.0 * pulse);
        draw_circle_lines(x, y, nb - 1.0, 2.0, col(warn));
        for j in (0..24).step_by(2) {
            // gestrichelter Warnkreis, der sich dreht
            let b0 = j as f32 * PI / 12.0 + t * 0.6;
            arc(x, y, nb - 9.0, -b0 - PI / 14.0, -b0, 3.0, col(warn));
        }
        g.shadow(x + 3.0, y + 5.0, 12.0, 1.0);
        g.sprite(&g.spr.icons[&Item::Nuke], x, y, 0.0, 1.2, WHITE);
        g.light([255, 220, 60], x, y, 26.0, if blink { 7.0 } else { 4.0 });
        let n = left.ceil().max(1.0) as u32;
        g.fancy(&n.to_string(), x, y - 30.0, 30.0, if fast { [255, 80, 60] } else { ACCENT_C }, 3.0, true, 1.0, 1.0);
        if self.marks.insert((sid, 10 + n.min(200) as u8)) {
            self.sound("siren", Some(x), 1.0);
        }
    }

    /// Deckkraft eines Panzers (Tarnkappe, Geist, Einblenden am Rundenstart)
    fn tank_alpha(&self, tk: &TankView, now: f64) -> f32 {
        let eff = &tk.eff;
        let mut alpha = 1.0f32;
        if eff.has(Eff::Invis) {
            if self.me.is_some() && Some(tk.slot) != self.me {
                return if self.my_eff.has(Eff::Radar) { 90.0 / 255.0 } else { 0.0 };
                // für die anderen weg
            }
            alpha = if self.me.is_none() { 35.0 / 255.0 } else { 100.0 / 255.0 };
        }
        if eff.has(Eff::Ghost) {
            let gt = eff.get(Eff::Ghost) as f32;
            alpha = alpha.min(if gt > 1.2 || (gt * 8.0) as i32 % 2 == 1 { 110.0 / 255.0 } else { 220.0 / 255.0 });
        }
        let spawn = (now - self.round_start) as f32;
        if spawn < 0.5 {
            alpha = alpha.min((spawn / 0.5).max(0.2));
        }
        alpha
    }

    pub(super) fn draw_tank(&mut self, g: &mut Gfx, tk: &TankView, now: f64) {
        if tk.hidden {
            return;
        }
        let (x, y, a) = (tk.x as f32, tk.y as f32, tk.a as f32);
        let i = tk.slot.min(MAX_PLAYERS - 1);
        if !tk.alive {
            g.sprite(&g.spr.wrecks[i], x, y, a, 1.0, WHITE);
            return;
        }
        let alpha = self.tank_alpha(tk, now);
        if alpha <= 0.0 {
            return;
        }
        let eff = &tk.eff;
        let k = tank_scale(eff) as f32;
        let tr = TANK_R as f32 * k;
        let t = now as f32;
        let spawn = (now - self.round_start) as f32;
        if spawn < 0.9 {
            // Lichtsäule beim Erscheinen
            let f = 1.0 - spawn / 0.9;
            g.light(COLORS[i], x, y, 46.0, 8.0 * f);
            g.lit_ring(col(shade(COLORS[i], f)), x, y, 14.0 + 40.0 * (1.0 - f), 2.0);
        }
        if alpha > 90.0 / 255.0 {
            g.shadow(x + 3.0, y + 5.0, 17.0 * k, 1.0);
        }
        if eff.has(Eff::Turbo) {
            if let Some(trail) = self.after.get(&tk.slot) {
                for (j, &(hx, hy, ha, _)) in trail.iter().enumerate() {
                    g.sprite(
                        &g.spr.tanks[i],
                        hx,
                        hy,
                        ha,
                        k,
                        Color::new(1.0, 1.0, 1.0, (45.0 + 30.0 * j as f32) / 255.0),
                    );
                }
            }
            for (c, rr) in [([255, 140, 25], 5.0), ([255, 235, 120], 2.5)] {
                let fl = tr + 4.0 + 2.0 * (t * 40.0 + i as f32).sin();
                draw_circle(x - a.cos() * fl, y - a.sin() * fl, rr * k + 1.0, col(c));
            }
            g.light([255, 150, 50], x - a.cos() * (tr + 6.0), y - a.sin() * (tr + 6.0), 20.0, 5.0);
        }
        g.sprite(&g.spr.tanks[i], x, y, a, k, Color::new(1.0, 1.0, 1.0, alpha));
        if alpha < 100.0 / 255.0 {
            return;
        }
        if g.dark {
            // Scheinwerfer vorne am Panzer
            g.light([255, 245, 215], x + a.cos() * 15.0 * k, y + a.sin() * 15.0 * k, 12.0, 6.0);
        }
        if eff.has(Eff::Frozen) {
            let s = 40.0 * k;
            draw_texture_ex(
                &g.spr.ice,
                x - s / 2.0,
                y - s / 2.0,
                WHITE,
                DrawTextureParams { dest_size: Some(vec2(s, s)), ..Default::default() },
            );
            g.light([120, 200, 255], x, y, 30.0, 3.0);
        }
        let sh = tk.shield as f32;
        if sh > 0.0 && (sh > 2.0 || (sh * 6.0) as i32 % 2 == 1) {
            // blinkt kurz vor Ablauf
            g.light([40, 110, 255], x, y, tr + 22.0, 3.0);
            g.lit_ring(col([60, 150, 255]), x, y, tr + 8.0, 3.0);
            draw_circle_lines(x, y, tr + 8.0, 1.0, col([170, 220, 255]));
            for j in 0..6 {
                // Wabenmuster wandert über den Schild
                let b = t * 1.6 + j as f32 * PI / 3.0;
                g.light([140, 210, 255], x + b.cos() * (tr + 8.0), y + b.sin() * (tr + 8.0), 5.0, 6.0);
            }
        }
        if eff.has(Eff::Ghost) {
            g.light([180, 190, 255], x, y, 30.0, 2.0);
        }
        if eff.has(Eff::Spawn) && (eff.get(Eff::Spawn) > 0.6 || (t * 10.0) as i32 % 2 == 1) {
            // Spawn-Schutz
            let c = mix(COLORS[i], [255, 255, 255], 0.5);
            g.lit_ring(col(c), x, y, tr + 7.0 + 1.5 * (t * 12.0).sin(), 2.0);
            g.light(c, x, y, tr + 18.0, 3.0);
        }
        if eff.has(Eff::Mirror) {
            // Spiegelschild: silbernes Sechseck, das sich dreht
            let pts: Vec<(f32, f32)> = (0..6)
                .map(|j| {
                    (
                        x + (t * 2.0 + j as f32 * PI / 3.0).cos() * (tr + 9.0),
                        y + (t * 2.0 + j as f32 * PI / 3.0).sin() * (tr + 9.0),
                    )
                })
                .collect();
            poly_lines(&pts, 2.0, col([215, 225, 240]), true);
            g.light([200, 215, 240], x, y, tr + 16.0, 3.0);
        }
        if eff.has(Eff::Star) {
            let c = FW_COLORS[(t * 12.0) as usize % FW_COLORS.len()];
            g.light(c, x, y, tr + 30.0, 7.0);
            g.lit_ring(col(c), x, y, tr + 5.0, 2.0);
        }
        if eff.has(Eff::Ram) {
            // Rammsporn vorne
            let (fx, fy, nx, ny) = (a.cos(), a.sin(), -a.sin(), a.cos());
            let base = tr * 0.9;
            let pts = [
                (x + fx * base + nx * tr * 0.9, y + fy * base + ny * tr * 0.9),
                (x + fx * (tr + 11.0), y + fy * (tr + 11.0)),
                (x + fx * base - nx * tr * 0.9, y + fy * base - ny * tr * 0.9),
            ];
            poly(&pts, col([200, 70, 40]));
            poly_lines(&pts, 2.0, col([70, 20, 12]), true);
            g.light([255, 90, 40], x + fx * (tr + 8.0), y + fy * (tr + 8.0), 16.0, 5.0);
        }
        if eff.has(Eff::Steer) && (t * 5.0) as i32 % 2 == 1 {
            g.light([80, 255, 120], x, y - 4.0, 12.0, 7.0); // lenkt gerade: Funklicht
        }
        if Some(i) == self.me && eff.has(Eff::Radar) {
            // Radar: Suchstrahl rund um den eigenen Panzer
            let b = t * 3.0;
            g.lit_line(col([40, 140, 60]), x, y, x + b.cos() * 150.0, y + b.sin() * 150.0, 2.0);
            g.lit_ring(col([25, 90, 40]), x, y, 150.0, 1.0);
        }
    }

    /// Alles über dem Panzer: Waffe, Krone, Kartoffel, Sprüche, Nachladen, Markierung
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_tank_ui(
        &mut self,
        g: &mut Gfx,
        tk: &TankView,
        arena: &Arena,
        view: &View,
        roster: &Roster,
        leader: Option<usize>,
        now: f64,
    ) {
        if !tk.alive || tk.hidden || self.tank_alpha(tk, now) < 100.0 / 255.0 {
            return; // getarnt: nichts am Panzer, das ihn verraten würde
        }
        let (x, y, a) = (tk.x as f32, tk.y as f32, tk.a as f32);
        let i = tk.slot.min(MAX_PLAYERS - 1);
        let eff = &tk.eff;
        let k = tank_scale(eff) as f32;
        let tr = TANK_R as f32 * k;
        let t = now as f32;
        if tk.reload > 0.0 && tk.weapon.is_none() {
            // lokal: Sekunden bei allen, im Netz nur beim eigenen Panzer
            let life = bullet_params(arena.mutator).life as f32;
            let frac = (tk.reload as f32 / life).min(1.0);
            let rr = tr + 10.0;
            draw_circle_lines(x, y, rr - 1.0, 2.0, col([60, 64, 76]));
            arc(x, y, rr - 1.5, -FRAC_PI_2, -FRAC_PI_2 - TAU * frac, 3.0, col(COLORS[i]));
            if self.me.is_none() || Some(i) == self.me {
                g.fancy(&format!("{:.1}", tk.reload), x, y - tr - 22.0, 18.0, TEXT_C, 2.0, false, 1.0, 1.0);
            }
        }
        if eff.has(Eff::Confuse) {
            for j in 0..3 {
                let b = t * 5.0 + j as f32 * 2.09;
                let (sx, sy) = (x + b.cos() * 12.0, y - tr - 10.0 + b.sin() * 3.0);
                let pts: Vec<(f32, f32)> = (0..10)
                    .map(|q| {
                        let r = if q % 2 == 0 { 4.0 } else { 1.8 };
                        (sx + (t * 6.0 + q as f32 * 1.257).cos() * r, sy + (t * 6.0 + q as f32 * 1.257).sin() * r)
                    })
                    .collect();
                poly(&pts, col([255, 220, 40]));
            }
        }
        if Some(i) == leader {
            g.sprite(&g.spr.crown, x, y - tr - 4.0, 0.0, 0.9, WHITE);
            if (t * 1.3 + i as f32) as i32 % 4 == 0 {
                g.light([255, 230, 120], x, y - tr - 9.0, 7.0, 6.0); // Glitzern
            }
        }
        if let Some(w) = tk.weapon {
            let (bx, by) = (x, y - tr - 17.0);
            g.sprite(&g.spr.badges[&w], bx, by, 0.0, 1.0, WHITE);
            if Some(i) == self.me {
                self.aim_help(g, w, x, y, a, arena);
            }
        }
        if tk.charge > 0.0 {
            // Railgun lädt: Ring füllt sich, voll = weiß
            let frac = (tk.charge as f32 / RAIL_CHARGE as f32).min(1.0);
            let c = if frac >= 1.0 { [255, 255, 255] } else { mix(COLORS[i], [120, 220, 255], 0.5) };
            arc(x, y, tr + 7.0, -FRAC_PI_2, -FRAC_PI_2 - TAU * frac, 3.0, col(c));
            g.light([120, 220, 255], x + a.cos() * tr, y + a.sin() * tr, 10.0 + 14.0 * frac, 3.0 + 4.0 * frac);
        }
        if eff.has(Eff::Extra) {
            g.sprite(&g.spr.icons[&Item::Extra], x - tr - 9.0, y - tr - 4.0, 0.0, 0.55, WHITE);
            // Ersatzteil im Gepäck
        }
        if eff.has(Eff::Potato) {
            let left = eff.get(Eff::Potato) as f32;
            let jig = if left < 3.0 { (t * 40.0).sin() * 2.0 } else { 0.0 };
            let (px, py) = (x + tr + 10.0 + jig, y - tr - 6.0);
            g.sprite(&g.spr.icons[&Item::Potato], px, py, 0.0, 1.0, WHITE);
            draw_circle(px + 9.0, py - 6.0, 2.0 + ((t * 12.0) as i32 % 2) as f32, col([255, 200, 40]));
            g.light([255, 120, 30], px, py, 22.0, if left < 3.0 { 7.0 } else { 4.0 });
            g.fancy(
                &format!("{}", left.ceil() as i32),
                px,
                py - 17.0,
                22.0,
                if left < 3.0 { [255, 80, 60] } else { TEXT_C },
                2.0,
                true,
                1.0,
                1.0,
            );
        }
        if tk.taunt > 0 {
            let txt = TAUNTS[(tk.taunt as usize - 1).min(TAUNTS.len() - 1)];
            let d = g.measure(txt, 15.0, false);
            let (bw, bh) = (d.width + 16.0, 24.0);
            let (bx, by) = (x - bw / 2.0, y - tr - 40.0 - bh / 2.0);
            draw_rectangle(bx - 1.0, by - 1.0, bw + 2.0, bh + 2.0, col([14, 15, 19]));
            draw_rectangle(bx, by, bw, bh, col([250, 250, 252]));
            draw_rectangle_lines(bx, by, bw, bh, 2.0, col(COLORS[i]));
            draw_triangle(vec2(x - 5.0, by + bh), vec2(x + 5.0, by + bh), vec2(x, by + bh + 7.0), col(COLORS[i]));
            g.text(txt, x, by + bh / 2.0, 15.0, col(INK_C), 0.5);
        }
        let spawn = (now - self.round_start) as f32;
        if spawn < 2.6 {
            // am Rundenstart: wer ist wer?
            let fade = ((2.6 - spawn) / 0.5).min(1.0);
            let bob = (t * 7.0).sin() * 3.0;
            if self.me.is_some() && Some(i) == self.me {
                let c = mix(COLORS[i], [255, 255, 255], 0.4);
                let (ty, pts, ly) = if y - tr - 60.0 > 0.0 {
                    // Pfeil von oben auf den Panzer
                    let ty = y - tr - 30.0 + bob;
                    (ty, [(x - 9.0, ty - 8.0), (x + 9.0, ty - 8.0), (x, ty + 4.0)], ty - 22.0)
                } else {
                    let ty = y + tr + 30.0 - bob;
                    (ty, [(x - 9.0, ty + 8.0), (x + 9.0, ty + 8.0), (x, ty - 4.0)], ty + 22.0)
                };
                let _ = ty;
                poly(&pts.map(|(px, py)| (px, py + 1.0)), col([14, 15, 19]));
                poly(&pts, cola(COLORS[i], fade));
                g.fancy("DU", x, ly, 24.0, c, 2.0, true, fade, 1.0);
            } else if !self.quiet {
                let mut name = roster.name(i).to_string();
                if view.is_bot(i) && !name.contains("Bot") {
                    name.push_str(" (Bot)");
                }
                g.fancy(
                    &name,
                    x,
                    y - tr - 20.0 + bob,
                    18.0,
                    mix(COLORS[i], [255, 255, 255], 0.35),
                    2.0,
                    false,
                    fade,
                    1.0,
                );
            }
        }
    }

    /// Zielhilfen für den eigenen Panzer
    fn aim_help(&self, _g: &mut Gfx, w: Item, x: f32, y: f32, a: f32, arena: &Arena) {
        let red = col([255, 90, 90]);
        let (xd, yd, ad) = (x as f64, y as f64, a as f64);
        let pts =
            |v: Vec<(f64, f64)>| -> Vec<(f32, f32)> { v.into_iter().map(|(a, b)| (a as f32, b as f32)).collect() };
        match w {
            Item::Laser => dotted(&pts(arena.raycast(xd, yd, ad, 320.0, &[]).0), red),
            Item::Strike => {
                let (tx, ty) = strike_target(arena, xd, yd, ad);
                dotted(&[(x, y), (tx as f32, ty as f32)], red);
                draw_circle_lines(tx as f32, ty as f32, STRIKE_BLAST as f32, 1.0, red);
            }
            Item::Zap => {
                arc(x, y, ZAP_RANGE as f32, a - ZAP_CONE as f32, a + ZAP_CONE as f32, 1.0, col([90, 160, 255]))
            }
            Item::Rail => {
                let (end, holes) = rail_path(arena, xd, yd, ad);
                dotted(&[(x, y), (end.0 as f32, end.1 as f32)], col([120, 220, 255]));
                for (hx, hy) in holes {
                    draw_circle_lines(hx as f32, hy as f32, 4.0, 1.0, col([120, 220, 255]));
                }
            }
            Item::Prism => {
                let p = pts(arena.raycast(xd, yd, ad, 260.0, &[]).0);
                dotted(&p[..p.len().min(2)], col([200, 220, 255]));
            }
            Item::Gl => {
                let (tx, ty) = ahead(arena, xd, yd, ad, GL_DIST, 12.0);
                dotted(&[(x, y), (tx as f32, ty as f32)], red);
                draw_circle_lines(tx as f32, ty as f32, GL_BLAST as f32, 1.0, red);
            }
            Item::Arty => {
                for d in ARTY_DIST {
                    let (tx, ty) = ahead(arena, xd, yd, ad, d, 12.0);
                    draw_circle_lines(tx as f32, ty as f32, ARTY_BLAST as f32, 1.0, red);
                }
            }
            Item::Orbit => {
                let (tx, ty) = ahead(arena, xd, yd, ad, ORBIT_DIST, 12.0);
                draw_circle_lines(tx as f32, ty as f32, ORBIT_R as f32, 1.0, col([120, 220, 255]));
                let (ex, ey) = ahead(arena, tx, ty, ad, ORBIT_SPEED * ORBIT_TIME, 12.0);
                dotted(&[(tx as f32, ty as f32), (ex as f32, ey as f32)], col([90, 170, 230]));
            }
            Item::Jump => {
                let (tx, ty) = ahead(arena, xd, yd, ad, JUMP_DIST, 12.0);
                dotted(&[(x, y), (tx as f32, ty as f32)], col([120, 180, 255]));
                draw_circle_lines(tx as f32, ty as f32, TANK_R as f32, 1.0, col([120, 180, 255]));
            }
            Item::Frost => draw_circle_lines(x, y, FROST_R as f32, 1.0, col([150, 210, 255])),
            Item::Push => {
                arc(x, y, PUSH_RANGE as f32, a - PUSH_CONE as f32, a + PUSH_CONE as f32, 1.0, col([150, 200, 255]))
            }
            Item::Phase => dotted(&[(x, y), (x + a.cos() * 300.0, y + a.sin() * 300.0)], col([80, 220, 255])),
            Item::Fence => {
                let ((x0, y0), (x1, y1)) = fence_ends(arena, xd, yd, ad + std::f64::consts::FRAC_PI_2);
                dotted(&[(x0 as f32, y0 as f32), (x1 as f32, y1 as f32)], red);
            }
            Item::Hook => {
                let d = first_wall(arena, xd, yd, ad, HOOK_SPEED * HOOK_LIFE) as f32;
                dotted(&[(x, y), (x + a.cos() * d, y + a.sin() * d)], col([176, 150, 110]));
            }
            Item::Timebomb => draw_circle_lines(x, y, TIMEBOMB_BLAST as f32, 1.0, red),
            Item::Destruct => draw_circle_lines(x, y, DESTRUCT_BLAST as f32, 1.0, red),
            Item::Hammer | Item::Chain | Item::Shear | Item::Disarm | Item::Parry => {
                let (r, half) = match w {
                    Item::Hammer => (HAMMER_RANGE, HAMMER_HALF),
                    Item::Chain => (CHAIN_RANGE, CHAIN_HALF),
                    Item::Shear => (SHEAR_RANGE, SHEAR_ARC),
                    Item::Disarm => (DISARM_RANGE, DISARM_HALF),
                    _ => (PARRY_REACH, PARRY_HALF),
                };
                super::weapons::sector(x, y, a, r as f32, half as f32, [164, 205, 238]);
            }
            Item::Ring | Item::Decoy | Item::Switch | Item::Rust | Item::Cushion | Item::Pendel | Item::Pincer => {
                let (dist, radius) = match w {
                    Item::Ring => (RING_THROW, RING_OUT),
                    Item::Decoy => (DECOY_THROW, DECOY_SENSE),
                    Item::Switch => (SWITCH_DIST, SWITCH_R),
                    Item::Rust => (RUST_THROW, RUST_R),
                    Item::Cushion => (CUSHION_THROW, CUSHION_R),
                    Item::Pendel => (40.0, PENDEL_LEN + PENDEL_R),
                    _ => (PINCER_DIST, PINCER_LAT),
                };
                let (tx, ty) = ahead(arena, xd, yd, ad, dist, 12.0);
                dotted(&[(x, y), (tx as f32, ty as f32)], cola([164, 205, 238], 0.7));
                draw_circle_lines(tx as f32, ty as f32, radius as f32, 1.0, cola([164, 205, 238], 0.7));
                if w == Item::Ring {
                    draw_circle_lines(tx as f32, ty as f32, RING_IN as f32, 1.0, col([184, 228, 198]));
                }
            }
            Item::Afterburn => {
                dotted(&[(x, y), (x + a.cos() * AFTERBURN_LEN as f32, y + a.sin() * AFTERBURN_LEN as f32)], red);
            }
            Item::Pause | Item::Twist => {
                let r = if w == Item::Pause { PAUSE_RANGE } else { TWIST_RANGE } as f32;
                draw_circle_lines(x, y, r, 1.0, col([184, 178, 241]));
            }
            _ => {}
        }
    }

    /// Effekte, die der Server meldet, zeichnen sich nach ihrem Alter
    pub(super) fn draw_fx(&mut self, g: &mut Gfx, view: &View, now: f64) {
        let t = now as f32;
        for fx in &view.fx {
            let k = (fx.age / fx.life).min(1.0) as f32;
            let f = 1.0 - k;
            match &fx.data {
                FxData::Boom { x, y, size } => {
                    let (x, y, z) = (*x as f32, *y as f32, *size as f32 / 100.0);
                    g.light([255, 150, 60], x, y, (20.0 + 44.0 * ease_out(k)) * z, 8.0 * (1.0 - k).powf(1.4));
                    g.light([255, 244, 210], x, y, (10.0 + 18.0 * k) * z, 8.0 * (1.0 - k).powi(2));
                }
                FxData::Laser { slot, pts } => {
                    if pts.len() < 2 {
                        continue;
                    }
                    let ff = (1.0 - k).powf(0.8);
                    let c = mix(COLORS[*slot as usize], [255, 60, 60], 0.35);
                    let p = pf(pts);
                    g.lit_polyline(c, 0.45 * ff, &p, 11.0);
                    g.lit_polyline(c, ff, &p, 6.0);
                    g.lit_polyline([255, 255, 255], ff, &p, 2.0);
                    for &(x, y) in &p[1..] {
                        g.light(c, x, y, 26.0, 6.0 * ff);
                    }
                }
                FxData::Zap { pts, .. } => {
                    if (fx.age * 30.0) as i32 % 4 == 3 {
                        continue; // flackert
                    }
                    let mut rnd = Rng::new(fx.id as u64 * 31 + (fx.age * 24.0) as u64);
                    let p = pf(pts);
                    for w in p.windows(2) {
                        let bolt = bolt_points(w[0].0, w[0].1, w[1].0, w[1].1, &mut rnd, 4);
                        g.lit_polyline([60, 120, 255], f, &bolt, 7.0);
                        g.lit_polyline([150, 210, 255], f, &bolt, 3.0);
                        g.lit_polyline([255, 255, 255], f, &bolt, 1.0);
                        for _ in 0..2 {
                            // Seitenäste
                            let (bx, by) = bolt[rnd.below(bolt.len())];
                            let b = rnd.uniform(0.0, TAU as f64) as f32;
                            let side = bolt_points(bx, by, bx + b.cos() * 22.0, by + b.sin() * 22.0, &mut rnd, 2);
                            g.lit_polyline([120, 180, 255], f, &side, 2.0);
                        }
                    }
                    for &(x, y) in &p {
                        g.light([110, 170, 255], x, y, 40.0, 7.0 * f);
                    }
                }
                FxData::Tele { x, y } => {
                    let (x, y) = (*x as f32, *y as f32);
                    g.light([170, 80, 255], x, y, 40.0, 7.0 * f);
                    for j in 0..2 {
                        let r = ((28.0 - 20.0 * k) * (1.0 - 0.4 * j as f32)).round().max(2.0);
                        g.lit_ring(col(shade([190, 110, 255], f)), x, y, r, 2.0);
                    }
                }
                FxData::Pick { x, y, item, .. } if k < 0.3 => {
                    g.light(box_color(*item), *x as f32, *y as f32, 40.0, 6.0 * (1.0 - k / 0.3))
                }
                FxData::Block { x, y } => {
                    g.lit_ring(col(shade([90, 170, 255], f)), *x as f32, *y as f32, 16.0 + 30.0 * ease_out(k), 3.0)
                }
                FxData::Nuke { x, y } => {
                    let (x, y) = (*x as f32, *y as f32);
                    let e = ease_out((k * 3.0).min(1.0));
                    g.light([255, 140, 50], x, y, 60.0 + 300.0 * e, 8.0 * f);
                    g.light([255, 250, 225], x, y, 40.0 + 120.0 * (k * 4.0).min(1.0), 8.0 * f * f);
                    let rr = NUKE_BLAST as f32 * 1.6 * ease_out((k * 2.2).min(1.0));
                    g.lit_ring(col(shade([255, 230, 180], f)), x, y, rr, (8.0 * f).round().max(1.0));
                }
                FxData::Rail { slot, p0, p1, holes } => {
                    let c = mix(COLORS[*slot as usize], [120, 220, 255], 0.6);
                    let p = [(p0.0 as f32, p0.1 as f32), (p1.0 as f32, p1.1 as f32)];
                    g.lit_polyline(c, 0.5 * f, &p, 14.0);
                    g.lit_polyline(c, f, &p, 6.0);
                    g.lit_polyline([255, 255, 255], f, &p, 2.0);
                    for &(x, y) in holes {
                        g.light(c, x as f32, y as f32, 30.0, 7.0 * f);
                    }
                }
                FxData::Prism { slot, beams } => {
                    for (j, b) in beams.iter().enumerate() {
                        if b.len() < 2 {
                            continue;
                        }
                        let c =
                            if j > 0 { PRISM_COLORS[j % 3] } else { mix(COLORS[*slot as usize], [255, 255, 255], 0.4) };
                        let p = pf(b);
                        g.lit_polyline(c, 0.45 * f, &p, 9.0);
                        g.lit_polyline(c, f, &p, 4.0);
                        g.lit_polyline([255, 255, 255], f, &p, 1.0);
                    }
                }
                FxData::Push { x, y, a, .. } => {
                    let (x, y, a) = (*x as f32, *y as f32, *a as f32 / 100.0);
                    let r = 20.0 + PUSH_RANGE as f32 * ease_out(k);
                    let cone = PUSH_CONE as f32;
                    let p: Vec<(f32, f32)> = (0..9)
                        .map(|j| {
                            let d = cone * (2.0 * j as f32 / 8.0 - 1.0);
                            (x + (a + d).cos() * r, y + (a + d).sin() * r)
                        })
                        .collect();
                    g.lit_polyline([150, 200, 255], f, &p, 4.0);
                }
                FxData::Frost { x, y } => {
                    let (x, y) = (*x as f32, *y as f32);
                    g.lit_ring(col(shade([170, 225, 255], f)), x, y, 14.0 + FROST_R as f32 * ease_out(k), 4.0);
                    g.light([120, 200, 255], x, y, FROST_R as f32, 6.0 * f);
                }
                FxData::Jump { x1, y1, .. } => {
                    g.lit_ring(col(shade([150, 190, 255], f)), *x1 as f32, *y1 as f32, 10.0 + 30.0 * ease_out(k), 2.0)
                }
                FxData::Disco { .. } => {
                    // bunte Scheinwerfer über das ganze Feld
                    let (ww, wh) = (self.world_w, self.world_w * VIEW / VW);
                    for (j, c) in FW_COLORS.iter().enumerate() {
                        let b = t * 1.6 + j as f32 * TAU / FW_COLORS.len() as f32;
                        g.light(
                            *c,
                            ww / 2.0 + b.cos() * ww * 0.3,
                            wh / 2.0 + (b * 1.3).sin() * wh * 0.3,
                            160.0,
                            5.0 * (f * 3.0).min(1.0),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// Schwebende Texte: aufgehobenes Power-up, Eigentor, Block
    pub(super) fn draw_fx_ui(&self, g: &mut Gfx, view: &View) {
        let pool = panzer_core::items::box_pool();
        for fx in &view.fx {
            let k = (fx.age / fx.life).min(1.0) as f32;
            let age = fx.age as f32;
            match &fx.data {
                FxData::Pick { x, y, item, mystery } => {
                    let (x, y) = (*x as f32, *y as f32);
                    if *mystery && age < 0.5 {
                        // Glücksrad
                        let shown = pool[(age * 22.0) as usize + fx.id as usize % pool.len()];
                        let shown = pool[pool.iter().position(|&p| p == shown).unwrap_or(0) % pool.len()];
                        g.sprite(&g.spr.boxes[&shown], x, y - 40.0, 0.0, 1.0, WHITE);
                        continue;
                    }
                    if *mystery && age < 0.9 {
                        let z = ease_back((age - 0.5) / 0.25);
                        g.sprite(&g.spr.boxes[item], x, y - 40.0, 0.0, (z * 1.3).max(0.1), WHITE);
                    }
                    let c = mix(box_color(*item), [255, 255, 255], 0.25);
                    let yy = y - 32.0 - 26.0 * k - if *mystery { 18.0 } else { 0.0 };
                    g.fancy(&format!("{}!", item.name()), x, yy, 24.0, c, 2.0, false, (2.5 * (1.0 - k)).min(1.0), 1.0);
                }
                FxData::Pop { x, y, text, .. } => {
                    let z = ease_back((age / 0.25).min(1.0));
                    let txt = panzer_core::text::pop_text(*text);
                    g.fancy(
                        txt,
                        *x as f32,
                        *y as f32 - 30.0 * k,
                        30.0,
                        [255, 90, 70],
                        3.0,
                        true,
                        (3.0 * (1.0 - k)).min(1.0),
                        z.max(0.05),
                    );
                }
                FxData::Block { x, y } => {
                    g.fancy(
                        "BLOCK!",
                        *x as f32,
                        *y as f32 - 30.0 - 22.0 * k,
                        22.0,
                        [120, 200, 255],
                        2.0,
                        true,
                        (3.0 * (1.0 - k)).min(1.0),
                        1.0,
                    );
                }
                _ => {}
            }
        }
    }

    /// Nebel: alles dunkel bis auf den Kreis um den eigenen Panzer, Wände schwach sichtbar
    pub(super) fn draw_fog(&mut self, g: &mut Gfx, arena: &Arena, view: &View) {
        let centers: Vec<(f32, f32)> = match self.me {
            None => view.tanks.iter().filter(|t| t.alive && !t.hidden).map(|t| (t.x as f32, t.y as f32)).collect(),
            Some(m) => match view.tank(m) {
                Some(t) if t.alive => vec![(t.x as f32, t.y as f32)],
                _ => return, // tot -> zuschauen ohne Nebel
            },
        };
        let (ww, wh) = (arena.ww as u32, arena.wh as u32);
        if self.fog.as_ref().is_none_or(|rt| rt.texture.width() as u32 != ww || rt.texture.height() as u32 != wh) {
            let rt = render_target(ww, wh);
            rt.texture.set_filter(FilterMode::Linear);
            self.fog = Some(rt);
        }
        let Some(rt) = self.fog.clone() else { return };
        push_camera_state();
        set_camera(&rt_camera(Rect::new(0.0, 0.0, ww as f32, wh as f32), &rt));
        let fogc = [9, 10, 15];
        clear_background(cola(fogc, 1.0));
        let wall = shade(arena.theme().wall, 0.45);
        for w in &arena.walls {
            draw_rectangle(w[0] as f32, w[1] as f32, w[2] as f32, w[3] as f32, col(wall));
        }
        // erst alle äußeren Ringe, dann die inneren (Überschreiben, nicht Mischen)
        gl_use_material(&g.replace);
        for (extra, alpha) in [(40.0, 215.0), (24.0, 150.0), (10.0, 70.0), (0.0, 0.0)] {
            for &(x, y) in &centers {
                draw_circle(x, y, FOG_R as f32 + extra, cola(fogc, alpha / 255.0));
            }
        }
        gl_use_default_material();
        pop_camera_state();
        draw_texture_ex(
            &rt.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams { dest_size: Some(vec2(ww as f32, wh as f32)), flip_y: true, ..Default::default() },
        );
    }

    /// Stromausfall: alles dunkel bis auf Scheinwerfer und alles, was glüht. Die Lichtkarte
    /// entsteht in Viertel-Auflösung und wird mit dem Bild multipliziert.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn darkness(
        &mut self,
        g: &mut Gfx,
        arena: &Arena,
        view: &View,
        now: f64,
        _lay: Layout,
        _src: Rect,
        _shake: Vec2,
    ) {
        let q = 4.0;
        let (lw, lh) = ((arena.ww as f32 / q) as u32 + 1, (arena.wh as f32 / q) as u32 + 1);
        if self.lightmap.as_ref().is_none_or(|rt| rt.texture.width() as u32 != lw || rt.texture.height() as u32 != lh) {
            let rt = render_target(lw, lh);
            rt.texture.set_filter(FilterMode::Linear);
            self.lightmap = Some(rt);
        }
        let Some(rt) = self.lightmap.clone() else { return };
        push_camera_state();
        // Kamera in Weltkoordinaten, nur die Auflösung ist geviertelt
        set_camera(&rt_camera(Rect::new(0.0, 0.0, lw as f32 * q, lh as f32 * q), &rt));
        clear_background(col([13, 15, 22]));
        gl_use_material(&g.add);
        for tk in &view.tanks {
            if tk.alive && !tk.hidden && self.tank_alpha(tk, now) >= 100.0 / 255.0 {
                let cone = &g.spr.cone;
                let s = cone.width() * q;
                draw_texture_ex(
                    cone,
                    tk.x as f32 - s / 2.0,
                    tk.y as f32 - s / 2.0,
                    WHITE,
                    DrawTextureParams { dest_size: Some(vec2(s, s)), rotation: tk.a as f32, ..Default::default() },
                );
            }
        }
        for &(c, x, y, r, level) in &g.light_log {
            let rr = r * 2.0;
            let k = ((level + 2.0).min(8.0)) / 8.0;
            draw_texture_ex(
                &g.spr.falloff,
                x - rr,
                y - rr,
                colk(c, k),
                DrawTextureParams { dest_size: Some(vec2(2.0 * rr, 2.0 * rr)), ..Default::default() },
            );
        }
        gl_use_default_material();
        pop_camera_state();
        gl_use_material(&g.mul);
        draw_texture_ex(
            &rt.texture,
            0.0,
            0.0,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(lw as f32 * q, lh as f32 * q)),
                flip_y: true,
                ..Default::default()
            },
        );
        gl_use_default_material();
    }
}
