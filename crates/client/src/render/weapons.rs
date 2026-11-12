//! Lesbare Formen für die Spezialwaffen. Maße und Zeiten kommen aus der Simulation.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use macroquad::prelude::*;
use panzer_core::consts::*;
use panzer_core::items::{Item, Kind};
use panzer_core::snap::{ShotView, TankView, View};

use super::Renderer;
use crate::gfx::*;
use crate::raster::mix;

/// Der Außenrand bleibt an der echten Wirkungsgrenze; der äußere Bogen zählt die Zeit.
pub(super) fn field(x: f32, y: f32, radius: f32, c: [u8; 3], age: f32, life: f32) {
    draw_circle(x, y, radius, cola(c, 0.07));
    draw_circle_lines(x, y, radius, 1.3, cola(c, 0.7));
    let left = (1.0 - age / life.max(0.001)).clamp(0.0, 1.0);
    arc(x, y, radius + 3.0, -FRAC_PI_2, -FRAC_PI_2 + TAU * left, 2.0, col(c));
}

pub(super) fn sector(x: f32, y: f32, a: f32, radius: f32, half: f32, c: [u8; 3]) {
    for j in 0..20 {
        let a0 = a - half + 2.0 * half * j as f32 / 20.0;
        let a1 = a - half + 2.0 * half * (j + 1) as f32 / 20.0;
        draw_triangle(
            vec2(x, y),
            vec2(x + a0.cos() * radius, y + a0.sin() * radius),
            vec2(x + a1.cos() * radius, y + a1.sin() * radius),
            cola(c, 0.08),
        );
    }
    arc(x, y, radius, a - half, a + half, 1.5, col(c));
    for b in [a - half, a + half] {
        draw_line(x, y, x + b.cos() * radius, y + b.sin() * radius, 1.0, cola(c, 0.5));
    }
}

/// Wurde die Sofortwaffe `w` zwischen zwei Bildern benutzt? Bei Einzelladungen steht im
/// Munitionsfeld die Zahl freier normaler Kugeln, nicht die Waffe; `dropped` = liegt als Kiste am Boden.
fn instant_used(w: Item, before_ammo: i32, weapon: Option<Item>, ammo: i32, dropped: bool) -> bool {
    if weapon == Some(w) {
        w.has_ammo() && ammo < before_ammo
    } else {
        weapon.is_none() && (!w.has_ammo() || before_ammo == 1) && !dropped
    }
}

fn sector_fill(x: f32, y: f32, a: f32, radius: f32, half: f32, c: Color) {
    for j in 0..12 {
        let a0 = a - half + 2.0 * half * j as f32 / 12.0;
        let a1 = a - half + 2.0 * half * (j + 1) as f32 / 12.0;
        draw_triangle(
            vec2(x, y),
            vec2(x + a0.cos() * radius, y + a0.sin() * radius),
            vec2(x + a1.cos() * radius, y + a1.sin() * radius),
            c,
        );
    }
}

fn arrow(x: f32, y: f32, a: f32, size: f32, c: [u8; 3]) {
    let forward = vec2(a.cos(), a.sin());
    let side = vec2(-a.sin(), a.cos());
    let p = vec2(x, y);
    draw_triangle(
        p + forward * size,
        p - forward * size * 0.6 + side * size * 0.65,
        p - forward * size * 0.6 - side * size * 0.65,
        col(c),
    );
}

fn crosshair(x: f32, y: f32, r: f32, c: [u8; 3]) {
    for j in 0..4 {
        let a = j as f32 * FRAC_PI_2;
        draw_line(
            x + a.cos() * (r - 5.0),
            y + a.sin() * (r - 5.0),
            x + a.cos() * (r + 5.0),
            y + a.sin() * (r + 5.0),
            2.0,
            col(c),
        );
    }
    draw_circle(x, y, 2.0, col(c));
}

impl Renderer {
    pub(super) fn track_weapon_releases(&mut self, view: &View, now: f64) {
        self.weapon_echoes.retain(|(_, at)| now - at < 0.24);
        for previous in &self.previous_weapon_shots {
            // Nur abgelaufene Vorwarnungen nachzeichnen, kein vorzeitig entferntes Objekt.
            if previous.life - previous.age <= 0.12 && !view.shots.iter().any(|p| p.id == previous.id) {
                self.weapon_echoes.push((previous.clone(), now));
            }
        }
        let mut keep = std::mem::take(&mut self.previous_weapon_shots);
        keep.clear();
        keep.extend(
            view.shots
                .iter()
                .filter(|p| matches!(p.kind, Kind::Afterburn | Kind::Hammer | Kind::Twist | Kind::Ring | Kind::Cross))
                .cloned(),
        );
        self.previous_weapon_shots = keep;
    }

    pub(super) fn draw_weapon_releases(&self, g: &mut Gfx, now: f64) {
        for (p, at) in &self.weapon_echoes {
            let fade = (1.0 - (now - at) as f32 / 0.24).clamp(0.0, 1.0);
            let (x, y, a) = (p.x as f32, p.y as f32, p.ang as f32);
            let c = [255, 171, 85];
            match p.kind {
                Kind::Afterburn => {
                    let end = (x + a.cos() * AFTERBURN_LEN as f32, y + a.sin() * AFTERBURN_LEN as f32);
                    g.lit_polyline(c, fade, &[(x, y), end], AFTERBURN_HALF_W as f32 * 2.0);
                    draw_line(x, y, end.0, end.1, 3.0, Color::new(1.0, 0.96, 0.82, fade));
                }
                Kind::Ring => {
                    let r = (RING_IN + RING_OUT) as f32 / 2.0;
                    g.lit_ring(col(crate::raster::shade(c, fade)), x, y, r, (RING_OUT - RING_IN) as f32);
                }
                Kind::Hammer => {
                    sector(x, y, a, HAMMER_RANGE as f32, HAMMER_HALF as f32, crate::raster::shade(c, fade));
                    g.light(c, x + a.cos() * HAMMER_RANGE as f32, y + a.sin() * HAMMER_RANGE as f32, 25.0, fade * 6.0);
                }
                Kind::Twist => {
                    g.lit_ring(col(crate::raster::shade([201, 155, 255], fade)), x, y, TWIST_RANGE as f32, 3.0)
                }
                Kind::Cross => {
                    crosshair(x, y, CROSS_R as f32, crate::raster::shade(c, fade));
                    g.light(c, x, y, CROSS_R as f32, fade * 6.0);
                }
                _ => {}
            }
        }
    }

    /// Sofortwaffen erzeugen kein Geschoss. Man erkennt den Einsatz daran, dass die Munition sinkt
    /// oder die letzte Ladung weg ist, ohne dass sie als Kiste am Boden liegt (das wäre Entwaffnung).
    pub(super) fn detect_action(&mut self, before: &TankView, tk: &TankView, view: &View, now: f64) {
        let Some(w) = before.weapon.filter(|w| matches!(w, Item::Shear | Item::Disarm | Item::Pause | Item::Parry))
        else {
            return;
        };
        let dropped = view.boxes.iter().any(|b| b.item == w && (b.x - tk.x).hypot(b.y - tk.y) < 14.0);
        if !instant_used(w, before.ammo, tk.weapon, tk.ammo, dropped) {
            return;
        }
        let (x, y, a) = (tk.x as f32, tk.y as f32, tk.a as f32);
        self.actions.retain(|act| now - act.4 < 1.5);
        self.actions.push((w, x, y, a, now));
        let sound = match w {
            Item::Shear => "snip",
            Item::Disarm => "whoosh",
            Item::Pause => "pause",
            _ => "clang",
        };
        self.sound(sound, Some(x), 1.0);
    }

    /// Wirkung der Sofortwaffen sichtbar machen, in ihren echten Reichweiten
    pub(super) fn draw_actions(&self, g: &mut Gfx, now: f64) {
        for &(w, x, y, a, at) in &self.actions {
            let dt = (now - at) as f32;
            match w {
                Item::Shear => {
                    // Zwei Stahlklingen stehen kurz offen über dem Kegel und schnappen dann zu.
                    // Nach dem Zuschnappen schnell weg, sonst sieht es wie ein Laserstrahl aus.
                    let (shut, out) = (0.16, 0.07);
                    if dt >= shut + out {
                        continue;
                    }
                    let (range, half) = (SHEAR_RANGE as f32, SHEAR_ARC as f32 / 2.0);
                    let close = (dt / shut).min(1.0).powi(2);
                    let fade = if dt < shut { 1.0 } else { 1.0 - (dt - shut) / out };
                    let c = [170, 222, 255];
                    sector_fill(x, y, a, range, half, cola(c, 0.08 * fade));
                    for side in [-1.0, 1.0] {
                        let b = a + side * half * (1.0 - close);
                        let (cb, sb) = (b.cos(), b.sin());
                        let (nx, ny) = (-sb * side, cb * side);
                        // Klinge: breit am Gelenk, spitz am Ende; Rücken nach außen, Schneide innen
                        let root = vec2(x + cb * 16.0, y + sb * 16.0);
                        let tip = vec2(x + cb * range, y + sb * range);
                        let back = root - vec2(nx, ny) * 6.0 + vec2(cb, sb) * 10.0;
                        draw_triangle(root, tip, back, Color::new(0.74, 0.78, 0.86, 0.8 * fade));
                        g.lit_line(colk(c, 0.9 * fade), root.x, root.y, tip.x, tip.y, 2.0);
                    }
                    draw_circle(x + a.cos() * 16.0, y + a.sin() * 16.0, 3.5, Color::new(0.35, 0.37, 0.42, fade));
                    if close > 0.85 {
                        let m = range * 0.55;
                        g.light(c, x + a.cos() * m, y + a.sin() * m, 36.0, 6.0 * fade);
                    }
                }
                Item::Disarm => {
                    // drei goldene Wellen laufen durch den Fächer
                    let k = dt / 0.4;
                    if k >= 1.0 {
                        continue;
                    }
                    let (range, half) = (DISARM_RANGE as f32, DISARM_HALF as f32);
                    let c = [255, 206, 110];
                    sector_fill(x, y, a, range, half, cola(c, 0.09 * (1.0 - k)));
                    for j in 0..3 {
                        let r = range * (k * 1.25 - j as f32 * 0.15);
                        if r <= 14.0 || r > range {
                            continue;
                        }
                        let f = (1.0 - r / range).max(0.25) * (1.0 - k);
                        let n = 10;
                        for i in 0..n {
                            let a0 = a - half + 2.0 * half * i as f32 / n as f32;
                            let a1 = a - half + 2.0 * half * (i + 1) as f32 / n as f32;
                            g.lit_line(
                                colk(c, 1.2 * f),
                                x + a0.cos() * r,
                                y + a0.sin() * r,
                                x + a1.cos() * r,
                                y + a1.sin() * r,
                                3.0,
                            );
                        }
                    }
                }
                Item::Pause => {
                    // Uhr-Ring breitet sich aus und steht, solange angehaltene Kugeln warten
                    let total = PAUSE_TIME as f32;
                    if dt >= total {
                        continue;
                    }
                    let grow = ease_out(dt / 0.18);
                    let fade = (1.0 - dt / total).powf(0.6);
                    let r = PAUSE_RANGE as f32 * grow;
                    let c = [184, 178, 241];
                    draw_circle(x, y, r, cola(c, 0.07 * fade));
                    g.lit_ring(colk(c, 1.1 * fade), x, y, r, 2.0);
                    for j in 0..12 {
                        let b = j as f32 * TAU / 12.0;
                        let inner = if j % 3 == 0 { r - 12.0 } else { r - 7.0 };
                        draw_line(
                            x + b.cos() * inner,
                            y + b.sin() * inner,
                            x + b.cos() * r,
                            y + b.sin() * r,
                            2.0,
                            cola(c, 0.8 * fade),
                        );
                    }
                    // Zeiger läuft in der Haltezeit einmal herum
                    let hand = -FRAC_PI_2 + TAU * (dt / total);
                    let (hx, hy) = (x + hand.cos() * (r - 14.0).max(0.0), y + hand.sin() * (r - 14.0).max(0.0));
                    draw_line(x, y, hx, hy, 2.0, cola([235, 232, 255], fade));
                    arc(x, y, r + 4.0, -FRAC_PI_2, hand, 2.0, cola(c, fade));
                }
                Item::Parry => {
                    // Klinge zieht als Sichel durch den Abwehrwinkel
                    let k = dt / 0.24;
                    if k >= 1.0 {
                        continue;
                    }
                    let half = PARRY_HALF as f32;
                    let head = a - half + 2.0 * half * ease_out((k / 0.5).min(1.0));
                    let fade = 1.0 - k;
                    let r = PARRY_REACH as f32 * 0.8;
                    let c = [255, 236, 190];
                    let n = 14;
                    for i in 0..n {
                        let f0 = i as f32 / n as f32;
                        let f1 = (i + 1) as f32 / n as f32;
                        let (b0, b1) = (a - half + (head - a + half) * f0, a - half + (head - a + half) * f1);
                        g.lit_line(
                            colk(c, 1.4 * f1 * fade),
                            x + b0.cos() * r,
                            y + b0.sin() * r,
                            x + b1.cos() * r,
                            y + b1.sin() * r,
                            1.0 + 4.0 * f1,
                        );
                    }
                    g.light(c, x + head.cos() * r, y + head.sin() * r, 22.0, 7.0 * fade);
                }
                _ => {}
            }
        }
    }

    pub(super) fn draw_weapon_shot(&self, g: &mut Gfx, p: &ShotView, view: &View, now: f64) {
        let (x, y, a) = (p.x as f32, p.y as f32, p.ang as f32);
        let (age, life) = (p.age as f32, p.life as f32);
        let k = (age / life.max(0.001)).clamp(0.0, 1.0);
        let t = now as f32;
        let team = COLORS[p.owner];
        let orange = [255, 172, 74];
        let cyan = [112, 221, 255];
        let purple = [201, 155, 255];
        match p.kind {
            Kind::Double => {
                self.ball(g, p.id, x, y, DOUBLE_R as f32, orange, true);
                draw_circle_lines(x, y, 8.0, 1.5, col(orange));
                draw_circle(x - 2.5, y, 1.1, WHITE);
                draw_circle(x + 2.5, y, 1.1, WHITE);
            }
            Kind::Corner => {
                self.trail(g, p.id, cyan, 6.0, 3.0);
                let a = self.heading(p.id).unwrap_or(a);
                let pts = rot_rect(x, y, 11.0, 7.0, a);
                poly(&pts, col([15, 35, 46]));
                poly_lines(&pts, 1.5, col(cyan), true);
                arrow(x, y, a, 5.0, cyan);
            }
            Kind::Chain => {
                if let Some(owner) = view.tank(p.owner) {
                    let start = vec2(owner.x as f32, owner.y as f32);
                    let end = vec2(x, y);
                    let d = end - start;
                    let n = (d.length() / 5.0).ceil().max(1.0) as usize;
                    for j in 0..=n {
                        let at = start + d * j as f32 / n as f32;
                        draw_circle_lines(at.x, at.y, 3.5, 1.4, col([195, 203, 218]));
                    }
                }
                let r = 19.0 - 5.0 * k;
                for side in [-1.0, 1.0] {
                    arc(x, y, r, side * FRAC_PI_2 - 0.8, side * FRAC_PI_2 + 0.8, 4.0, col(orange));
                }
                arc(x, y, r + 4.0, -FRAC_PI_2, -FRAC_PI_2 + TAU * k, 2.0, col(orange));
            }
            Kind::Hammer => {
                let r = HAMMER_RANGE as f32;
                let half = HAMMER_HALF as f32;
                sector(x, y, a, r, half, orange);
                let swing = a - half + 2.0 * half * k * k;
                let end = vec2(x + swing.cos() * (r - 8.0), y + swing.sin() * (r - 8.0));
                draw_line(x, y, end.x, end.y, 5.0, col([34, 28, 21]));
                draw_line(x, y, end.x, end.y, 2.5, col([186, 148, 93]));
                let head = rot_rect(end.x, end.y, 12.0, 22.0, swing);
                poly(&head, col([203, 210, 224]));
                poly_lines(&head, 2.0, col(orange), true);
            }
            Kind::Decoy => {
                field(x, y, DECOY_SENSE as f32, orange, age, life);
                let armed = age >= DECOY_ARM as f32;
                let body = rot_rect(x, y, 16.0, 13.0, 0.0);
                poly(&body, col([45, 38, 25]));
                poly_lines(&body, 2.0, col(orange), true);
                for j in 0..3 {
                    draw_line(
                        x - 6.0 + j as f32 * 5.0,
                        y - 4.0,
                        x - 2.0 + j as f32 * 5.0,
                        y + 4.0,
                        2.0,
                        cola(orange, 0.8),
                    );
                }
                draw_circle(x, y - 9.0, 2.0, col(if armed { [255, 85, 60] } else { [120, 120, 120] }));
            }
            Kind::Shadow => {
                let ghost = Color::new(0.55, 0.4, 0.8, 0.55);
                g.sprite(&g.spr.tanks[p.owner], x, y, a, 1.0, ghost);
                field(x, y, TANK_R as f32 + 5.0, purple, age, life);
                for da in [-SHADOW_SPREAD as f32, 0.0, SHADOW_SPREAD as f32] {
                    dotted(
                        &[
                            (x + (a + da).cos() * 20.0, y + (a + da).sin() * 20.0),
                            (x + (a + da).cos() * 55.0, y + (a + da).sin() * 55.0),
                        ],
                        cola(purple, 0.7),
                    );
                }
            }
            Kind::Twist => {
                field(x, y, TWIST_RANGE as f32, purple, age, life);
                for j in 0..4 {
                    let b = t * 2.0 + j as f32 * FRAC_PI_2;
                    arc(x, y, 19.0, b, b + 1.0, 2.0, col(purple));
                    arrow(x + (b + 1.0).cos() * 19.0, y + (b + 1.0).sin() * 19.0, b + 1.0 + FRAC_PI_2, 4.0, purple);
                }
            }
            Kind::Switch | Kind::Catcher => {
                let switch = p.kind == Kind::Switch;
                let r = if switch { SWITCH_R } else { CATCHER_R } as f32;
                let c = if switch { cyan } else { purple };
                field(x, y, r, c, age, life);
                draw_circle(x, y, 10.0, col([24, 27, 38]));
                draw_circle_lines(x, y, 10.0, 2.0, col(team));
                if switch {
                    draw_line(x - 6.0, y + 4.0, x + 4.0, y + 4.0, 2.0, col(c));
                    draw_line(x + 4.0, y + 4.0, x + 4.0, y - 5.0, 2.0, col(c));
                    arrow(x + 4.0, y - 5.0, -FRAC_PI_2, 3.5, c);
                } else {
                    for side in [-1.0, 1.0] {
                        arc(x, y, 7.0, side * FRAC_PI_2 - 0.8, side * FRAC_PI_2 + 0.8, 2.0, col(c));
                    }
                    arrow(x, y, PI, 4.0, c);
                }
            }
            Kind::Afterburn => {
                let (dx, dy) = (a.cos(), a.sin());
                let (ex, ey) = (x + dx * AFTERBURN_LEN as f32, y + dy * AFTERBURN_LEN as f32);
                let width = AFTERBURN_HALF_W as f32 * 2.0;
                draw_line(x, y, ex, ey, width, cola(orange, 0.10 + 0.10 * k));
                for side in [-1.0, 1.0] {
                    let (sx, sy) = (-dy * width * 0.5 * side, dx * width * 0.5 * side);
                    draw_line(x + sx, y + sy, ex + sx, ey + sy, 1.0, cola(orange, 0.65));
                }
                dotted(&[(x, y), (ex, ey)], col(orange));
                let (fx, fy) = (x + (ex - x) * k, y + (ey - y) * k);
                g.light([255, 110, 45], fx, fy, 22.0, 6.0);
                draw_circle(fx, fy, 3.5, WHITE);
            }
            Kind::Thorn => {
                let armed = age >= THORN_FUSE as f32;
                let c = if armed { orange } else { [151, 158, 174] };
                sector(x, y, a, THORN_REACH as f32, FRAC_PI_2, c);
                let base = rot_rect(x, y, 9.0, 24.0, a);
                poly(&base, col([38, 42, 50]));
                poly_lines(&base, 1.5, col(team), true);
                for offset in [-7.0, 0.0, 7.0] {
                    arrow(x - a.sin() * offset, y + a.cos() * offset, a, if armed { 8.0 } else { 5.0 }, c);
                }
            }
            Kind::Plate => {
                field(x, y, PLATE_R as f32, orange, age, life);
                let pts = rot_rect(x, y, PLATE_R as f32 * 1.35, PLATE_R as f32 * 1.35, PI / 4.0);
                poly(&pts, col([55, 52, 43]));
                poly_lines(&pts, 2.0, col(orange), true);
                // Drei mechanische Nieten, kein erfundener Zähler: Betritte sind nicht im Snapshot.
                for j in 0..3 {
                    draw_circle(x - 6.0 + j as f32 * 6.0, y, 1.7, col(orange));
                }
            }
            Kind::Pincer => {
                self.trail(g, p.id, orange, 7.0, 3.0);
                arc(x, y, PINCER_R as f32, a + 0.7, a + TAU - 0.7, 3.0, col(orange));
                arrow(x, y, a, 7.0, orange);
            }
            Kind::Cross => {
                let locked = age >= CROSS_FOLLOW as f32;
                let c = if locked { [255, 90, 80] } else { purple };
                field(x, y, CROSS_R as f32, c, age, life);
                crosshair(x, y, CROSS_R as f32, c);
                g.text(if locked { "WEG!" } else { "ZIEL" }, x, y - CROSS_R as f32 - 14.0, 11.0, col(c), 0.5);
            }
            Kind::Verdict => {
                // Die Stillstandszeit wird serverseitig geführt. Keinen falschen Zähler aus age ableiten.
                let r = TANK_R as f32 + 8.0;
                field(x, y, r, [255, 112, 105], age, life);
                for j in 0..4 {
                    let a = PI / 4.0 + j as f32 * FRAC_PI_2;
                    arrow(x + a.cos() * (r + 6.0), y + a.sin() * (r + 6.0), a + PI, 4.0, [255, 112, 105]);
                }
                g.text("BEWEGEN!", x, y - r - 11.0, 11.0, col([255, 171, 160]), 0.5);
            }
            Kind::Pendel => {
                let armed = age >= PENDEL_WARN as f32;
                let c = if armed { orange } else { [156, 167, 190] };
                let r = PENDEL_LEN as f32;
                draw_circle_lines(x, y, r + PENDEL_R as f32, 1.0, cola(c, 0.35));
                for j in 1..=4 {
                    let b = a - j as f32 * 0.15;
                    draw_line(
                        x,
                        y,
                        x + b.cos() * r,
                        y + b.sin() * r,
                        PENDEL_R as f32 * 2.0,
                        cola(c, 0.10 * (1.0 - j as f32 / 5.0)),
                    );
                }
                let (ex, ey) = (x + a.cos() * r, y + a.sin() * r);
                draw_line(x, y, ex, ey, 5.0, col([38, 42, 50]));
                draw_line(x, y, ex, ey, 2.5, col([186, 196, 214]));
                draw_circle(ex, ey, PENDEL_R as f32, col(c));
                draw_circle_lines(ex, ey, PENDEL_R as f32, 2.0, col([37, 39, 46]));
                draw_circle(x, y, 7.0, col(team));
                draw_circle(x, y, 3.0, col([215, 222, 236]));
            }
            Kind::Rust => {
                let c = [205, 133, 71];
                field(x, y, RUST_R as f32, c, age, life);
                let fade = (age / 0.25).min((life - age).max(0.0) / 0.3).min(1.0);
                for j in 0..9 {
                    let b = j as f32 * 2.399 + t * 0.22;
                    let d = RUST_R as f32 * (0.15 + 0.18 * (j % 4) as f32);
                    g.puff(x + b.cos() * d, y + b.sin() * d, 11.0 + 2.0 * (j % 3) as f32, c, 0.3 * fade);
                }
                // Durchgestrichener Lauf: hier kann niemand schießen.
                draw_line(x - 7.0, y + 4.0, x + 6.0, y - 4.0, 4.0, col([244, 204, 154]));
                draw_line(x - 9.0, y - 8.0, x + 9.0, y + 8.0, 2.0, col([255, 100, 75]));
            }
            Kind::Cushion => {
                let c = [101, 225, 192];
                field(x, y, CUSHION_R as f32, c, age, life);
                let armed = age >= CUSHION_ARM as f32;
                let pts = rot_rect(x, y, 27.0, 21.0, 0.0);
                poly(&pts, col([29, 80, 72]));
                poly_lines(&pts, 2.5, col(c), true);
                for j in 0..4 {
                    let b = j as f32 * FRAC_PI_2;
                    let d = if armed { 19.0 + 2.0 * (t * 5.0).sin() } else { 14.0 };
                    arrow(x + b.cos() * d, y + b.sin() * d, b, 4.0, c);
                }
            }
            Kind::Cracker => {
                let c = mix(orange, [255, 80, 55], k);
                field(x, y, 16.0, c, age, life);
                let pts = rot_rect(x, y, 12.0, 18.0, -0.2);
                poly(&pts, col([179, 49, 34]));
                poly_lines(&pts, 1.5, col(orange), true);
                draw_line(x, y - 9.0, x + 4.0, y - 14.0, 1.5, col(orange));
                g.light(c, x + 4.0, y - 14.0, 10.0, 3.0 + 3.0 * k);
            }
            Kind::Ring => {
                // Ein Ring aus Vierecken, keine Scheibe über dem sicheren Zentrum.
                let (inner, outer) = (RING_IN as f32, RING_OUT as f32);
                for j in 0..64 {
                    let (a0, a1) = (j as f32 * TAU / 64.0, (j + 1) as f32 * TAU / 64.0);
                    let pts = [
                        (x + a0.cos() * inner, y + a0.sin() * inner),
                        (x + a0.cos() * outer, y + a0.sin() * outer),
                        (x + a1.cos() * outer, y + a1.sin() * outer),
                        (x + a1.cos() * inner, y + a1.sin() * inner),
                    ];
                    poly(&pts, cola(orange, 0.12 + 0.08 * k));
                }
                draw_circle_lines(x, y, inner, 2.0, col(orange));
                draw_circle_lines(x, y, outer, 2.0, col(orange));
                arc(x, y, outer + 4.0, -FRAC_PI_2, -FRAC_PI_2 + TAU * (1.0 - k), 2.5, col(orange));
                g.text("SICHER", x, y, 10.0, col([184, 228, 198]), 0.5);
            }
            Kind::Dist => {
                let armed = age * DIST_SPEED as f32 >= DIST_ARM as f32;
                let c = if armed { orange } else { [139, 159, 181] };
                self.ball(g, p.id, x, y, DIST_R as f32, c, true);
                if armed {
                    crosshair(x, y, 7.0, c);
                } else {
                    draw_circle_lines(x, y, 7.0, 1.0, cola(c, 0.5));
                }
            }
            Kind::Eater => {
                self.trail(g, p.id, purple, 9.0, 3.0);
                g.light(purple, x, y, 26.0, 4.0);
                let mouth = 0.25 + 0.18 * (t * 12.0).sin();
                let a = self.heading(p.id).unwrap_or(0.0);
                arc(x, y, EATER_R as f32, a + mouth, a + TAU - mouth, 5.0, col(purple));
                draw_circle(x, y, 3.0, col([18, 14, 28]));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_charge_use_is_seen_although_ammo_field_counts_bullets() {
        // Zündpause hat eine Ladung, im Feld stehen 5 freie Kugeln, danach ist die Waffe weg
        assert!(instant_used(Item::Pause, 5, None, 5, false));
        // Entwaffnet: Waffe weg, liegt aber als Kiste am Boden
        assert!(!instant_used(Item::Pause, 5, None, 5, true));
        // noch in der Hand, das Feld ändert sich nur durch normale Kugeln
        assert!(!instant_used(Item::Pause, 5, Some(Item::Pause), 4, false));
    }

    #[test]
    fn multi_charge_use_counts_down() {
        assert!(instant_used(Item::Shear, 2, Some(Item::Shear), 1, false));
        assert!(instant_used(Item::Shear, 1, None, 5, false));
        assert!(!instant_used(Item::Shear, 2, Some(Item::Shear), 2, false));
        // Wundertüte ersetzt die Waffe: kein Einsatz
        assert!(!instant_used(Item::Shear, 2, Some(Item::Gold), 1, false));
    }
}
