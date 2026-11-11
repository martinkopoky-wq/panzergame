//! Ereignisse aus den Snapshots (Sound, Partikel), laufende Emitter, Umgebung, Partikel-Sortierung.

use std::f32::consts::{PI, TAU};

use panzer_core::consts::*;
use panzer_core::items::{Eff, Kind};
use panzer_core::physics::tank_scale;
use panzer_core::snap::{FxData, View};
use panzer_core::Arena;

use super::{Amb, Flying, Frame, Renderer, TURRET_AIR};
use crate::gfx::*;
use crate::particles::{o, Opt, FIRE_GRAD, P};
use crate::raster::{mix, shade};
use crate::sprites::{box_color, FW_COLORS, PRISM_COLORS};

/// Klang beim Abfeuern je Geschossart
fn shot_sound(k: Kind) -> Option<&'static str> {
    use Kind::*;
    Some(match k {
        B | Seek | Gold | Split | Wave | Double | Corner | Dist => "shot",
        Mg | Sb | Spb => "mg",
        Sg | Nv => "sg",
        Rocket | Rc | Mrk | Fw => "rocket",
        Frag | Nuke | Gl | Napalm | Ring => "frag",
        Bnc | Sticky => "bnc",
        Rang | Hook => "rang",
        Swap | Phase => "tele",
        Ice => "ice",
        Hole => "hole",
        Mine | Spider | Trap | Destruct => "mine",
        Strike | Arty => "strike",
        Fl | Burn => "flame",
        Sentry | Oil | Timebomb | Fence | Sprinkler | Decoy | Switch | Catcher | Cushion | Cracker | Rust | Shadow => {
            "drop"
        }
        Chain | Thorn | Plate => "clank",
        Hammer | Pendel => "rang",
        Twist => "tele",
        Afterburn => "tick",
        Cross | Verdict => "blip",
        Eater => "hole",
        Saw => "saw",
        Plasma => "plasma",
        Cannon => "cannon",
        Orbit => "orbit",
        _ => return None,
    })
}

/// Geschosse, die nicht aus dem Rohr kommen (kein Mündungsfeuer)
fn no_muzzle(k: Kind) -> bool {
    k.is_static() || matches!(k, Kind::Frg | Kind::Spk | Kind::Sb | Kind::Spb | Kind::Spider | Kind::Trap | Kind::Oil)
}

/// Funken hinter fliegenden Geschossen: (pro Sekunde, Partikel, Farbe oder bunt, Tempo, Leben)
fn sparkle(k: Kind) -> Option<(f32, P, Option<[u8; 3]>, f32, f32)> {
    Some(match k {
        Kind::Plasma => (30.0, P::Spark, Some([90, 255, 170]), 90.0, 0.2),
        Kind::Saw => (12.0, P::Spark, Some([255, 255, 255]), 150.0, 0.15),
        Kind::Gold => (30.0, P::Star, Some([255, 210, 80]), 15.0, 0.5),
        Kind::Phase => (25.0, P::Star, Some([120, 230, 255]), 0.0, 0.4),
        Kind::Fw => (40.0, P::Star, None, 20.0, 0.4),
        Kind::Napalm => (30.0, P::Spark, Some([255, 140, 50]), 60.0, 0.2),
        _ => return None,
    })
}

/// Geschossarten, an denen man Abpraller sieht und hört
fn bounce_fx(k: Kind) -> bool {
    use Kind::*;
    matches!(
        k,
        B | Mg | Sg | Bnc | Frag | Ice | Swap | Saw | Cannon | Gold | Nv | Sb | Seek | Mrk | Spk | Split | Wave | Spb
    )
}

pub fn light_r(r: f32) -> usize {
    let r = (r as usize).max(2);
    let step = if r < 24 {
        2
    } else if r < 80 {
        6
    } else {
        20
    };
    r - r % step + step
}

impl Renderer {
    fn chance(&mut self, p: f32) -> bool {
        self.parts.u(0.0, 1.0) < p
    }

    pub(super) fn events(&mut self, view: &View, f: &Frame, now: f64) {
        for p in &view.shots {
            if !self.seen_shots.insert(p.id) {
                continue;
            }
            self.sound(shot_sound(p.kind).unwrap_or("shot"), Some(p.x as f32), 1.0);
            if let Some(tk) = view.tank(p.owner) {
                if !no_muzzle(p.kind) && tk.alive && !tk.hidden {
                    self.muzzle(tk.x as f32, tk.y as f32, tk.a as f32, tank_scale(&tk.eff) as f32, p.kind);
                }
            }
        }
        for fx in &view.fx {
            if !self.seen_fx.insert(fx.id) {
                continue;
            }
            if fx.age > fx.life * 0.7 {
                continue; // z. B. nach dem Beitreten: Altes nicht nachholen
            }
            match &fx.data {
                FxData::Boom { x, y, size } => {
                    let (x, y, z) = (*x as f32, *y as f32, *size as f32 / 100.0);
                    self.explosion(x, y, z);
                    self.sound(if z >= 1.2 { "bigboom" } else { "boom" }, Some(x), 1.0);
                    self.shake(0.3 * z, Some((x, y)));
                }
                FxData::Die { x, y, slot, angle } => {
                    let (x, y) = (*x as f32, *y as f32);
                    self.focus = Some((x, y));
                    self.wreck_burst(x, y, *slot as usize, *angle as f32 / 100.0);
                    self.shake(0.3, Some((x, y)));
                    self.flash = self.flash.max(if Some(*slot as usize) == f.me { 0.22 } else { 0.07 });
                }
                FxData::Laser { slot, pts } => {
                    if let Some(first) = pts.first() {
                        self.sound("laser", Some(first.0 as f32), 1.0);
                    }
                    if pts.len() > 2 {
                        for &(x, y) in &pts[1..pts.len() - 1] {
                            self.parts.add(
                                P::Flash,
                                x as f32,
                                y as f32,
                                0.0,
                                0.0,
                                0.25,
                                22.0,
                                o(COLORS[*slot as usize]),
                            );
                        }
                    }
                    if let Some(&(ex, ey)) = pts.last() {
                        self.parts.burst(
                            P::Spark,
                            ex as f32,
                            ey as f32,
                            10.0,
                            300.0,
                            0.4,
                            2.0,
                            Opt::default().drag(3.0),
                        );
                    }
                }
                FxData::Zap { pts, .. } => {
                    if let Some(first) = pts.first() {
                        self.sound("zap", Some(first.0 as f32), 1.0);
                    }
                    for &(x, y) in pts.iter().skip(1) {
                        self.parts.burst(
                            P::Spark,
                            x as f32,
                            y as f32,
                            12.0,
                            320.0,
                            0.35,
                            2.0,
                            Opt::default().drag(3.0),
                        );
                        self.parts.add(P::Flash, x as f32, y as f32, 0.0, 0.0, 0.3, 40.0, o([120, 190, 255]));
                    }
                    self.flash = self.flash.max(0.1);
                }
                FxData::Tele { x, y } => {
                    self.sound("tele", Some(*x as f32), 1.0);
                    self.parts.burst(
                        P::Star,
                        *x as f32,
                        *y as f32,
                        16.0,
                        150.0,
                        0.6,
                        2.0,
                        o([190, 110, 255]).drag(4.0),
                    );
                }
                FxData::Pick { x, y, item, mystery } => {
                    self.sound(if *mystery { "mystery" } else { "pick" }, Some(*x as f32), 1.0);
                    let c = box_color(*item);
                    self.parts.burst(P::Star, *x as f32, *y as f32, 18.0, 170.0, 0.7, 2.0, o(c).drag(3.5));
                    self.parts.add(P::Ring, *x as f32, *y as f32, 0.0, 0.0, 0.4, 46.0, o(c));
                }
                FxData::Block { x, y } => {
                    self.sound("shield", Some(*x as f32), 1.0);
                    self.parts.burst(
                        P::Shard,
                        *x as f32,
                        *y as f32,
                        16.0,
                        260.0,
                        0.6,
                        3.0,
                        o([120, 200, 255]).drag(3.0),
                    );
                    self.parts.add(P::Flash, *x as f32, *y as f32, 0.0, 0.0, 0.25, 45.0, o([90, 170, 255]));
                }
                FxData::Freeze { x, y } => {
                    self.sound("freeze", Some(*x as f32), 1.0);
                    self.parts.burst(
                        P::Shard,
                        *x as f32,
                        *y as f32,
                        12.0,
                        200.0,
                        0.5,
                        3.0,
                        o([200, 235, 255]).drag(4.0),
                    );
                    self.parts.burst(P::Star, *x as f32, *y as f32, 12.0, 90.0, 0.9, 2.0, o([170, 225, 255]).drag(2.0));
                }
                FxData::Nuke { x, y } => {
                    self.focus = Some((*x as f32, *y as f32));
                    self.sound("nuke", Some(*x as f32), 1.0);
                    self.nuke_burst(*x as f32, *y as f32);
                }
                FxData::Ann { .. } => self.sound("ann", None, 1.0),
                FxData::Rail { p0, p1, holes, .. } => {
                    self.sound("rail", Some(p0.0 as f32), 1.0);
                    self.shake(0.3, Some((p0.0 as f32, p0.1 as f32)));
                    self.flash = self.flash.max(0.12);
                    for &(x, y) in holes.iter().chain(std::iter::once(p1)) {
                        let (x, y) = (x as f32, y as f32);
                        self.parts.burst(P::Spark, x, y, 16.0, 380.0, 0.45, 2.0, Opt::default().drag(3.0));
                        self.parts.burst(P::Dust, x, y, 6.0, 90.0, 0.8, 7.0, o([170, 170, 185]).drag(3.0));
                        self.parts.add(P::Flash, x, y, 0.0, 0.0, 0.3, 34.0, o([150, 220, 255]));
                    }
                }
                FxData::Prism { beams, .. } => {
                    if let Some(p) = beams.first().and_then(|b| b.first()) {
                        self.sound("laser", Some(p.0 as f32), 1.0);
                    }
                    for (j, b) in beams.iter().enumerate() {
                        if let Some(&(ex, ey)) = b.last() {
                            self.parts.burst(
                                P::Spark,
                                ex as f32,
                                ey as f32,
                                8.0,
                                260.0,
                                0.35,
                                2.0,
                                Opt::default().drag(3.0),
                            );
                            self.parts.add(
                                P::Flash,
                                ex as f32,
                                ey as f32,
                                0.0,
                                0.0,
                                0.25,
                                24.0,
                                o(PRISM_COLORS[j % 3]),
                            );
                        }
                    }
                }
                FxData::Push { x, y, a, .. } => {
                    self.sound("push", Some(*x as f32), 1.0);
                    self.parts.burst(
                        P::Dust,
                        *x as f32,
                        *y as f32,
                        22.0,
                        260.0,
                        0.6,
                        8.0,
                        o([190, 200, 220]).drag(3.0).fan(*a as f32 / 100.0, PUSH_CONE as f32),
                    );
                }
                FxData::Frost { x, y } => {
                    self.sound("freeze", Some(*x as f32), 1.0);
                    self.parts.burst(
                        P::Shard,
                        *x as f32,
                        *y as f32,
                        36.0,
                        340.0,
                        0.6,
                        3.0,
                        o([200, 235, 255]).drag(3.0),
                    );
                    self.parts.burst(
                        P::Star,
                        *x as f32,
                        *y as f32,
                        24.0,
                        160.0,
                        1.0,
                        2.0,
                        o([170, 225, 255]).drag(2.0),
                    );
                }
                FxData::Jump { x0, y0, x1, y1 } => {
                    self.sound("jump", Some(*x0 as f32), 1.0);
                    for (x, y) in [(*x0, *y0), (*x1, *y1)] {
                        self.parts.burst(
                            P::Dust,
                            x as f32,
                            y as f32,
                            12.0,
                            120.0,
                            0.7,
                            7.0,
                            o([180, 180, 190]).drag(3.0),
                        );
                    }
                    self.parts.burst(P::Fire, *x0 as f32, *y0 as f32, 8.0, 90.0, 0.4, 7.0, Opt::default().drag(3.0));
                }
                FxData::Disco { x, .. } => self.sound("disco", Some(*x as f32), 1.0),
                FxData::Mirror { x, y } => {
                    self.sound("mirror", Some(*x as f32), 1.0);
                    self.parts.burst(
                        P::Star,
                        *x as f32,
                        *y as f32,
                        10.0,
                        160.0,
                        0.4,
                        2.0,
                        o([230, 240, 255]).drag(4.0),
                    );
                }
                FxData::Build { x, y } => {
                    self.sound("build", Some(*x as f32), 1.0);
                    self.parts.burst(
                        P::Dust,
                        *x as f32,
                        *y as f32,
                        14.0,
                        110.0,
                        0.7,
                        7.0,
                        o([190, 170, 150]).drag(3.0),
                    );
                }
                FxData::Trap { x, y } => {
                    self.sound("trap", Some(*x as f32), 1.0);
                    self.parts.burst(P::Spark, *x as f32, *y as f32, 10.0, 220.0, 0.3, 2.0, Opt::default().drag(5.0));
                }
                FxData::Fw { x, y } => {
                    self.sound("fw", Some(*x as f32), 1.0);
                    for c in FW_COLORS {
                        self.parts.burst(
                            P::Star,
                            *x as f32,
                            *y as f32,
                            8.0,
                            220.0,
                            0.9,
                            2.0,
                            o(c).drag(2.5).grav(40.0),
                        );
                    }
                    self.parts.add(P::Flash, *x as f32, *y as f32, 0.0, 0.0, 0.3, 60.0, o([255, 240, 220]));
                }
                FxData::Pop { .. } => {}
            }
        }
        for tk in &view.tanks {
            if tk.taunt != 0 && self.taunts.get(&tk.slot) != Some(&tk.taunt) {
                self.sound("blip", Some(tk.x as f32), 1.0);
            }
            self.taunts.insert(tk.slot, tk.taunt);
        }
        let won = view.win.is_some();
        if won && !self.last_win {
            self.sound("win", None, 1.0);
            if let Some(slot) = view.win.as_ref().and_then(|w| w.slot) {
                if let Some(tk) = view.tank(slot) {
                    self.confetti(tk.x as f32, tk.y as f32, COLORS[slot]);
                }
            }
        }
        self.last_win = won;
        if view.ts < 0.9 && !self.slow_seen {
            self.slow_seen = true;
            self.sound("slowmo", None, 1.0);
        }
        for (i, &s) in view.scores.iter().enumerate() {
            if self.scores_seen[i].is_some_and(|old| s > old) {
                self.score_pop[i] = Some(now);
            }
            self.scores_seen[i] = Some(s);
        }
    }

    /// Mündungsfeuer, Funken und Qualm beim Schießen
    fn muzzle(&mut self, x: f32, y: f32, a: f32, k: f32, kind: Kind) {
        let (mx, my) = (x + a.cos() * MUZZLE as f32 * k, y + a.sin() * MUZZLE as f32 * k);
        if kind == Kind::Fl {
            if self.chance(0.35) {
                self.parts.add(
                    P::Smoke,
                    mx,
                    my,
                    a.cos() * 60.0,
                    a.sin() * 60.0,
                    0.8,
                    5.0,
                    o([50, 48, 50]).drag(2.5).grav(-12.0),
                );
            }
            return;
        }
        use Kind::*;
        let big = matches!(kind, Rocket | Nuke | Hole | Frag | Sg | Cannon | Rc | Plasma);
        if kind == Cannon {
            self.shake(0.25, Some((x, y)));
        }
        self.parts.add(P::Flash, mx, my, 0.0, 0.0, 0.09, if big { 30.0 } else { 20.0 }, o([255, 214, 150]));
        self.parts.burst(
            P::Spark,
            mx,
            my,
            if big { 5.0 } else { 3.0 },
            280.0,
            0.18,
            2.0,
            Opt::default().drag(6.0).fan(a, 0.45),
        );
        self.parts.add(
            P::Smoke,
            mx,
            my,
            a.cos() * 35.0,
            a.sin() * 35.0,
            0.7,
            if big { 6.0 } else { 4.0 },
            o([96, 98, 106]).drag(3.0).grav(-8.0),
        );
    }

    fn explosion(&mut self, x: f32, y: f32, z: f32) {
        let pr = &mut self.parts;
        pr.add(P::Flash, x, y, 0.0, 0.0, 0.15 + 0.06 * z, 58.0 * z, o([255, 236, 190]));
        pr.burst(P::Fire, x, y, 8.0 + 9.0 * z, 100.0 * z, 0.6, 12.0 * z, Opt::default().drag(3.6));
        pr.burst(P::Spark, x, y, 8.0 + 12.0 * z, 420.0, 0.45, 2.0, Opt::default().drag(2.4));
        pr.burst(P::Smoke, x, y, 3.0 + 5.0 * z, 46.0 * z, 1.7, 11.0 * z, o([58, 58, 64]).drag(1.8).grav(-16.0));
        pr.burst(P::Ember, x, y, 3.0 + 5.0 * z, 170.0, 1.1, 1.5, o([255, 170, 70]).drag(1.6).grav(-24.0));
        pr.add(P::Ring, x, y, 0.0, 0.0, 0.35 + 0.1 * z, 70.0 * z, o([255, 200, 140]));
        self.scorch_at(x, y, 16.0 + 12.0 * z);
    }

    /// Panzer zerlegt: Trümmer in Flugrichtung, Funken, der Turm fliegt davon
    fn wreck_burst(&mut self, x: f32, y: f32, slot: usize, a: f32) {
        let c = COLORS[slot.min(MAX_PLAYERS - 1)];
        for _ in 0..12 {
            let b = a + self.parts.u(-1.4, 1.4);
            let v = self.parts.u(90.0, 310.0);
            let cc = mix(c, [30, 26, 24], self.parts.u(0.15, 0.75));
            let life = self.parts.u(1.0, 1.8);
            let size = self.parts.u(3.0, 7.0);
            let spin = self.parts.u(-14.0, 14.0);
            self.parts.add(P::Debris, x, y, b.cos() * v, b.sin() * v, life, size, o(cc).drag(3.2).spin(spin));
        }
        self.parts.burst(P::Spark, x, y, 24.0, 460.0, 0.55, 2.0, Opt::default().drag(2.4).fan(a, 1.7));
        self.parts.burst(P::Fire, x, y, 10.0, 80.0, 1.0, 15.0, Opt::default().drag(2.2));
        let b = a + self.parts.u(-0.7, 0.7);
        let v = self.parts.u(110.0, 170.0);
        let spin = if self.chance(0.5) { -1.0 } else { 1.0 } * self.parts.u(7.0, 14.0);
        let a0 = self.parts.u(0.0, 6.3);
        self.flying.push(Flying { x, y, vx: b.cos() * v, vy: b.sin() * v, age: 0.0, a: a0, spin, slot });
    }

    fn nuke_burst(&mut self, x: f32, y: f32) {
        self.flash = 1.0;
        self.trauma = 1.0;
        let pr = &mut self.parts;
        pr.add(P::Flash, x, y, 0.0, 0.0, 0.6, 260.0, o([255, 250, 230]));
        pr.burst(P::Fire, x, y, 60.0, 330.0, 1.4, 34.0, Opt::default().drag(2.6));
        pr.burst(P::Spark, x, y, 70.0, 750.0, 0.9, 3.0, Opt::default().drag(1.8));
        pr.burst(P::Smoke, x, y, 34.0, 230.0, 3.6, 30.0, o([96, 88, 84]).drag(1.4).grav(-14.0));
        for j in 0..24 {
            // Ring aus Rauch, der sich ausbreitet
            let b = j as f32 * PI / 12.0;
            pr.add(
                P::Smoke,
                x,
                y,
                b.cos() * 260.0,
                b.sin() * 260.0,
                3.0,
                22.0,
                o([120, 110, 104]).drag(2.2).grav(-6.0),
            );
        }
        pr.burst(P::Ember, x, y, 50.0, 420.0, 2.4, 2.5, o([255, 160, 60]).drag(1.2).grav(-30.0));
        for i in 0..3 {
            pr.add(
                P::Ring,
                x,
                y,
                0.0,
                0.0,
                0.8 + 0.3 * i as f32,
                NUKE_BLAST as f32 * (1.3 + 0.35 * i as f32),
                o([255, 220, 170]),
            );
        }
        self.scorch_at(x, y, 110.0);
    }

    fn confetti(&mut self, x: f32, y: f32, c: [u8; 3]) {
        let cols = [c, mix(c, [255, 255, 255], 0.5), ACCENT_C, [255, 255, 255]];
        for _ in 0..90 {
            let a = self.parts.u(0.0, TAU);
            let v = self.parts.u(80.0, 330.0);
            let life = self.parts.u(1.6, 2.6);
            let size = self.parts.u(4.0, 7.0);
            let cc = cols[self.parts.rng.below(cols.len())];
            let spin = self.parts.u(4.0, 12.0);
            self.parts.add(
                P::Confetti,
                x,
                y,
                a.cos() * v,
                a.sin() * v - 120.0,
                life,
                size,
                o(cc).drag(2.2).grav(55.0).spin(spin),
            );
        }
    }

    fn scorch_at(&mut self, x: f32, y: f32, r: f32) {
        let r = light_r(r);
        let cv = self.scorch.entry(r).or_insert_with(|| crate::sprites::scorch_canvas(r));
        if let Some(m) = self.maze.as_mut() {
            m.stamp(cv, x, y);
        }
    }

    fn bounce(&mut self, x: f32, y: f32, kind: Kind, owner: usize, track: [u8; 3]) {
        if let Some(m) = self.maze.as_mut() {
            m.dot(x, y, 2.0, shade(track, 0.8), 1.0); // Einschlagspur
        }
        let c = if kind == Kind::Bnc { [255, 120, 200] } else { mix(COLORS[owner], [255, 230, 160], 0.6) };
        self.parts.burst(P::Spark, x, y, 4.0, 190.0, 0.22, 2.0, Opt::default().drag(6.0));
        self.parts.add(P::Flash, x, y, 0.0, 0.0, 0.1, 14.0, o(c));
        self.sound(if kind == Kind::Bnc { "bnc" } else { "tink" }, Some(x), 1.0);
    }

    /// Merkt sich die letzten Positionen jedes Geschosses (Leuchtschweif, Abpraller)
    pub(super) fn track_shots(&mut self, view: &View, now: f64) {
        let track = self.maze_track_color();
        let mut bounces = Vec::new();
        for p in &view.shots {
            let (x, y) = (p.x as f32, p.y as f32);
            let h = self.hist.entry(p.id).or_default();
            let Some(&(lx, ly, _)) = h.last() else {
                h.push((x, y, now));
                continue;
            };
            let (dx, dy) = (x - lx, y - ly);
            if dx * dx + dy * dy < 0.04 {
                continue;
            }
            if h.len() >= 2 && bounce_fx(p.kind) {
                let (ox, oy) = (lx - h[h.len() - 2].0, ly - h[h.len() - 2].1);
                if (dx * ox < 0.0 && ox.abs() > 0.3) || (dy * oy < 0.0 && oy.abs() > 0.3) {
                    bounces.push((lx, ly, p.kind, p.owner));
                }
            }
            h.push((x, y, now));
            let trail_time = if p.kind == Kind::Wave { 0.35 } else { 0.13 };
            while h.len() > 3 && now - h[0].2 > trail_time {
                h.remove(0);
            }
        }
        let live: std::collections::HashSet<u32> = view.shots.iter().map(|s| s.id).collect();
        self.hist.retain(|k, _| live.contains(k));
        for (x, y, k, o) in bounces {
            self.bounce(x, y, k, o, track);
        }
    }

    fn maze_track_color(&self) -> [u8; 3] {
        [25, 28, 37]
    }

    /// Kettenspuren direkt auf die Labyrinth-Fläche (bleiben bis zur nächsten Runde)
    pub(super) fn stamp_tracks(&mut self, view: &View, arena: &Arena) {
        let c = arena.theme().track;
        for tk in &view.tanks {
            if !tk.alive || tk.hidden || tk.eff.has(Eff::Ghost) || tk.eff.has(Eff::Invis) {
                continue;
            }
            let (x, y, a) = (tk.x as f32, tk.y as f32, tk.a as f32);
            let last = self.tracks.get(&tk.slot).copied();
            if let Some(l) = last {
                if (x - l.0).hypot(y - l.1) < 5.0 {
                    continue;
                }
            }
            self.tracks.insert(tk.slot, (x, y, a));
            let Some((lx, ly, la)) = last else { continue };
            if (x - lx).hypot(y - ly) > 40.0 {
                continue; // Teleport
            }
            let k = tank_scale(&tk.eff) as f32;
            if let Some(m) = self.maze.as_mut() {
                for side in [-1.0f32, 1.0] {
                    let p0 = (lx - la.sin() * 12.0 * k * side, ly + la.cos() * 12.0 * k * side);
                    let p1 = (x - a.sin() * 12.0 * k * side, y + a.cos() * 12.0 * k * side);
                    m.line(p0.0, p0.1, p1.0, p1.1, (6.0 * k).round().max(2.0), c, 1.0);
                }
            }
        }
    }

    /// Partikel, die laufend entstehen: Wracks brennen, Staub, Flammen, Raketenrauch ...
    pub(super) fn emit(&mut self, view: &View, arena: &Arena, vdt: f32, now: f64) {
        let th = arena.theme();
        for tk in &view.tanks {
            let (x, y, a) = (tk.x as f32, tk.y as f32, tk.a as f32);
            let prev = self.prev.get(&tk.slot).cloned();
            if !tk.alive {
                let da = tk.dead_age as f32;
                if da < 3.5 && self.chance(28.0 * vdt) {
                    let (px, py) = (x + self.parts.u(-9.0, 9.0), y + self.parts.u(-9.0, 9.0));
                    let (vx, vy) = (self.parts.u(-10.0, 10.0), self.parts.u(-40.0, -12.0));
                    let (life, size) = (self.parts.u(0.35, 0.8), self.parts.u(6.0, 11.0));
                    self.parts.add(P::Fire, px, py, vx, vy, life, size, Opt::default().drag(1.0).grav(-20.0));
                }
                if da < 9.0 && self.chance(if da < 3.5 { 12.0 } else { 5.0 } * vdt) {
                    let (px, py) = (x + self.parts.u(-6.0, 6.0), y + self.parts.u(-6.0, 6.0));
                    let (vx, vy) = (self.parts.u(-8.0, 8.0), self.parts.u(-34.0, -16.0));
                    let (life, size) = (self.parts.u(1.4, 2.4), self.parts.u(6.0, 10.0));
                    self.parts.add(P::Smoke, px, py, vx, vy, life, size, o([46, 46, 52]).drag(0.6).grav(-6.0));
                }
                self.prev.insert(tk.slot, tk.clone());
                continue;
            }
            if tk.hidden {
                self.prev.remove(&tk.slot);
                continue;
            }
            let eff = &tk.eff;
            let k = tank_scale(eff) as f32;
            let tr = TANK_R as f32 * k;
            let hidden = eff.has(Eff::Invis) && self.me.is_some() && Some(tk.slot) != self.me;
            if let Some(p) = &prev {
                if p.alive && !hidden && !eff.has(Eff::Ghost) {
                    let moved = (x - p.x as f32).hypot(y - p.y as f32);
                    if 0.3 < moved && moved < 20.0 && self.chance(0.45) {
                        let side = if self.chance(0.5) { -1.0 } else { 1.0 };
                        let bx = x - a.cos() * 14.0 * k - a.sin() * 11.0 * k * side;
                        let by = y - a.sin() * 14.0 * k + a.cos() * 11.0 * k * side;
                        let (vx, vy) = (self.parts.u(-14.0, 14.0), self.parts.u(-14.0, 14.0));
                        let life = self.parts.u(0.5, 0.9);
                        self.parts.add(P::Dust, bx, by, vx, vy, life, 3.5 * k, o(th.dust).drag(2.0));
                    }
                }
            }
            if hidden {
                self.prev.insert(tk.slot, tk.clone());
                continue;
            }
            if eff.has(Eff::Potato) && self.chance(22.0 * vdt) {
                let (px, py) = (x + tr + 10.0, y - tr - 6.0);
                let vx = self.parts.u(-8.0, 8.0);
                self.parts.add(P::Fire, px, py - 4.0, vx, -40.0, 0.4, 5.0, Opt::default().drag(1.0).grav(-30.0));
                if self.chance(0.4) {
                    self.parts.add(P::Smoke, px, py - 6.0, 0.0, -30.0, 0.9, 4.0, o([70, 70, 74]).drag(1.0).grav(-10.0));
                }
            }
            if eff.has(Eff::Turbo) && self.chance(50.0 * vdt) {
                let (bx, by) = (x - a.cos() * (tr + 6.0), y - a.sin() * (tr + 6.0));
                self.parts.add(
                    P::Fire,
                    bx,
                    by,
                    -a.cos() * 90.0,
                    -a.sin() * 90.0,
                    0.25,
                    5.0 * k,
                    Opt::default().drag(4.0),
                );
            }
            for (e, rate, c, vy, life) in
                [(Eff::Ghost, 14.0, [210, 220, 255], -20.0, 0.8), (Eff::Frozen, 12.0, [190, 230, 255], -8.0, 0.9)]
            {
                if eff.has(e) && self.chance(rate * vdt) {
                    let (px, py) = (x + self.parts.u(-tr, tr), y + self.parts.u(-tr, tr));
                    self.parts.add(P::Star, px, py, 0.0, vy, life, 2.0, o(c).drag(1.0));
                }
            }
            if tk.shield > 0.0 && self.chance(8.0 * vdt) {
                let b = self.parts.u(0.0, TAU);
                self.parts.add(
                    P::Star,
                    x + b.cos() * (tr + 8.0),
                    y + b.sin() * (tr + 8.0),
                    0.0,
                    0.0,
                    0.5,
                    2.0,
                    o([110, 190, 255]),
                );
            }
            if eff.has(Eff::Star) && self.chance(30.0 * vdt) {
                let c = FW_COLORS[(now * 10.0) as usize % FW_COLORS.len()];
                let (px, py) = (x + self.parts.u(-tr, tr), y + self.parts.u(-tr, tr));
                self.parts.add(P::Star, px, py, 0.0, 0.0, 0.5, 2.0, o(c));
            }
            if eff.has(Eff::Slow) && self.chance(10.0 * vdt) {
                let px = x + self.parts.u(-tr, tr);
                self.parts.add(P::Star, px, y - tr, 0.0, -18.0, 0.8, 2.0, o([170, 100, 230]).drag(1.0));
            }
            if eff.has(Eff::Ram) && self.chance(25.0 * vdt) {
                let (fx, fy) = (x + a.cos() * (tr + 4.0), y + a.sin() * (tr + 4.0));
                let (vx, vy) = (self.parts.u(-60.0, 60.0), self.parts.u(-60.0, 60.0));
                self.parts.add(P::Spark, fx, fy, vx, vy, 0.2, 2.0, Opt::default().drag(4.0));
            }
            if let Some(p) = prev.filter(|p| p.alive) {
                if p.eff.has(Eff::Frozen) && !eff.has(Eff::Frozen) {
                    // Eis zerspringt
                    self.parts.burst(P::Shard, x, y, 14.0, 220.0, 0.5, 3.0, o([200, 235, 255]).drag(4.0));
                    self.sound("freeze", Some(x), 0.6);
                }
                for e in [Eff::Mini, Eff::Ghost, Eff::Invis] {
                    if p.eff.has(e) != eff.has(e) {
                        // Effekt an oder aus: Puff
                        self.parts.burst(P::Dust, x, y, 10.0, 90.0, 0.6, 6.0, o([200, 200, 220]).drag(3.0));
                    }
                }
                if eff.has(Eff::Potato) {
                    // tickt in den letzten Sekunden
                    let left = eff.get(Eff::Potato);
                    let before = if p.eff.has(Eff::Potato) { p.eff.get(Eff::Potato) } else { 99.0 };
                    if left < 4.0 && left.ceil() != before.ceil() {
                        self.sound("tick", Some(x), 1.0);
                    }
                }
            }
            if eff.has(Eff::Turbo) {
                let trail = self.after.entry(tk.slot).or_default();
                if trail.last().is_none_or(|l| now - l.3 > 0.035) {
                    trail.push((x, y, a, now));
                    if trail.len() > 3 {
                        trail.remove(0);
                    }
                }
            } else {
                self.after.remove(&tk.slot);
            }
            self.prev.insert(tk.slot, tk.clone());
        }
        for p in &view.shots {
            let (x, y) = (p.x as f32, p.y as f32);
            if let Some((rate, part, c, v, life)) = sparkle(p.kind) {
                if self.chance(rate * vdt) {
                    let (vx, vy) = (self.parts.u(-v, v), self.parts.u(-v, v));
                    let c = c.unwrap_or(FW_COLORS[self.parts.rng.below(FW_COLORS.len())]);
                    self.parts.add(part, x, y, vx, vy, life, 2.0, o(c).drag(4.0));
                }
                continue;
            }
            match p.kind {
                Kind::Oilf | Kind::Burn => {
                    let (r, rate) = if p.kind == Kind::Burn { (NAPALM_R as f32, 160.0) } else { (OIL_R as f32, 60.0) };
                    if self.chance(rate * vdt) {
                        let (b, d) = (self.parts.u(0.0, TAU), self.parts.u(0.0, r * 0.9));
                        let (vx, vy) = (self.parts.u(-10.0, 10.0), self.parts.u(-40.0, -10.0));
                        self.parts.add(
                            P::Fire,
                            x + b.cos() * d,
                            y + b.sin() * d,
                            vx,
                            vy,
                            0.5,
                            10.0,
                            Opt::default().drag(1.0).grav(-30.0),
                        );
                    }
                    if self.chance(8.0 * vdt) {
                        self.parts.add(P::Smoke, x, y, 0.0, -25.0, 1.4, 10.0, o([40, 38, 42]).drag(1.0).grav(-12.0));
                    }
                }
                Kind::Orbit if p.age > ORBIT_DELAY && self.chance(50.0 * vdt) => {
                    let b = self.parts.u(0.0, TAU);
                    self.parts.add(
                        P::Spark,
                        x,
                        y,
                        b.cos() * 260.0,
                        b.sin() * 260.0,
                        0.25,
                        2.0,
                        Opt::default().drag(4.0),
                    );
                }
                Kind::Meteor => {
                    let k = 1.0 - (p.life - p.age) as f32 / 0.7;
                    if k > 0.0 && self.chance(70.0 * vdt) {
                        let (mx, my) = (x + 260.0 * (1.0 - k), y - 380.0 * (1.0 - k));
                        self.parts.add(P::Fire, mx, my, 40.0, -60.0, 0.35, 9.0, Opt::default().drag(2.0));
                        self.parts.add(P::Smoke, mx, my, 30.0, -40.0, 0.9, 7.0, o([70, 64, 60]).drag(1.5));
                    }
                }
                Kind::Rocket | Kind::Rc | Kind::Mrk => {
                    let a = p.ang as f32;
                    let (tx, ty) = (x - a.cos() * 13.0, y - a.sin() * 13.0);
                    if self.chance(if p.kind != Kind::Mrk { 70.0 } else { 25.0 } * vdt) {
                        let (vx, vy) = (self.parts.u(-10.0, 10.0), self.parts.u(-10.0, 10.0));
                        self.parts.add(
                            P::Smoke,
                            tx,
                            ty,
                            vx,
                            vy,
                            SMOKE_LIFE as f32 * 1.4,
                            4.0,
                            o([120, 120, 126]).drag(1.5),
                        );
                    }
                    if self.chance(40.0 * vdt) {
                        self.parts.add(
                            P::Fire,
                            tx,
                            ty,
                            -a.cos() * 40.0,
                            -a.sin() * 40.0,
                            0.18,
                            5.0,
                            Opt::default().drag(3.0),
                        );
                    }
                }
                Kind::Fl => {
                    if self.chance(30.0 * vdt) {
                        // jede Flamme lodert weiter
                        let (vx, vy) = (self.parts.u(-25.0, 25.0), self.parts.u(-35.0, 5.0));
                        self.parts.add(P::Fire, x, y, vx, vy, 0.3, 10.0, Opt::default().drag(2.0).grav(-40.0));
                    }
                    if self.chance(7.0 * vdt) {
                        let (vx, vy) = (self.parts.u(-30.0, 30.0), self.parts.u(-50.0, -10.0));
                        self.parts.add(P::Ember, x, y, vx, vy, 0.7, 1.5, o([255, 150, 50]).drag(1.5).grav(-20.0));
                        if self.chance(0.5) {
                            self.parts.add(P::Smoke, x, y, 0.0, -20.0, 1.0, 6.0, o([40, 38, 40]).drag(1.2).grav(-14.0));
                        }
                    }
                }
                Kind::Ice if self.chance(35.0 * vdt) => {
                    let (vx, vy) = (self.parts.u(-20.0, 20.0), self.parts.u(-20.0, 20.0));
                    self.parts.add(P::Star, x, y, vx, vy, 0.5, 1.6, o([180, 230, 255]).drag(2.0));
                }
                Kind::Swap | Kind::Bnc if self.chance(18.0 * vdt) => {
                    let c = if p.kind == Kind::Swap { [200, 130, 255] } else { [255, 140, 210] };
                    let (vx, vy) = (self.parts.u(-15.0, 15.0), self.parts.u(-15.0, 15.0));
                    self.parts.add(P::Star, x, y, vx, vy, 0.45, 1.6, o(c).drag(2.0));
                }
                Kind::Nuke if self.chance(6.0 * vdt) => {
                    let (vx, vy) = (self.parts.u(-80.0, 80.0), self.parts.u(-80.0, 80.0));
                    self.parts.add(P::Spark, x, y, vx, vy, 0.25, 2.0, Opt::default().drag(4.0));
                }
                _ => {}
            }
        }
        self.ambient(arena, vdt);
    }

    /// Umgebung je nach Look: Staub, Sand, Glut, Schnee, Glühwürmchen, Neon-Pixel
    fn ambient(&mut self, arena: &Arena, vdt: f32) {
        let kind = arena.theme().amb;
        if self.amb_id != Some(arena.id) {
            self.amb.clear();
            self.amb_id = Some(arena.id);
        }
        let (x0, y0, x1, y1) = arena.bounds();
        let (x0, y0, x1, y1) = (x0 as f32, y0 as f32, x1 as f32, y1 as f32);
        let base = match kind {
            Ambient::Dust => 30,
            Ambient::Sand => 55,
            Ambient::Ember => 40,
            Ambient::Snow => 70,
            Ambient::Firefly => 22,
            Ambient::Neon => 28,
        };
        let want = base * ((arena.cols * arena.rows) / 48).max(1);
        while self.amb.len() < want {
            let (x, y) = (self.parts.u(x0, x1), self.parts.u(y0, y1));
            let (vx, vy) = match kind {
                Ambient::Sand => (self.parts.u(50.0, 110.0), self.parts.u(-8.0, 8.0)),
                Ambient::Ember => (self.parts.u(-10.0, 10.0), self.parts.u(-40.0, -15.0)),
                Ambient::Snow => (self.parts.u(5.0, 25.0), self.parts.u(20.0, 50.0)),
                Ambient::Neon => (0.0, self.parts.u(-25.0, -8.0)),
                _ => (self.parts.u(-8.0, 8.0), self.parts.u(-8.0, 8.0)),
            };
            let life = self.parts.u(3.0, 8.0);
            let ph = self.parts.u(0.0, 6.3);
            self.amb.push(Amb { x, y, vx, vy, age: 0.0, life, ph });
        }
        self.amb.retain_mut(|p| {
            p.age += vdt;
            p.x += p.vx * vdt;
            p.y += p.vy * vdt;
            if kind == Ambient::Firefly {
                // schwirren herum
                p.vx += (p.age * 1.7 + p.ph).sin() * 30.0 * vdt;
                p.vy += (p.age * 1.3 + p.ph).cos() * 30.0 * vdt;
            }
            p.age < p.life && x0 - 20.0 < p.x && p.x < x1 + 20.0 && y0 - 20.0 < p.y && p.y < y1 + 20.0
        });
    }

    pub(super) fn draw_ambient(&mut self, g: &mut Gfx, arena: &Arena, low: bool) {
        let th = arena.theme();
        let kind = th.amb;
        for p in &self.amb {
            let mut fade = (p.age / 0.8).min((p.life - p.age) / 0.8).min(1.0);
            match kind {
                Ambient::Ember | Ambient::Firefly | Ambient::Neon => {
                    if low {
                        continue;
                    }
                    if kind == Ambient::Firefly {
                        fade *= (p.age * 2.2 + p.ph).sin().max(0.0);
                    }
                    let c = match kind {
                        Ambient::Ember => [255, 120, 40],
                        Ambient::Firefly => [190, 255, 90],
                        _ => {
                            if (p.ph * 10.0) as i32 % 2 == 1 {
                                th.accent
                            } else {
                                th.hi
                            }
                        }
                    };
                    g.light(c, p.x, p.y, if kind != Ambient::Neon { 5.0 } else { 4.0 }, 7.0 * fade);
                }
                _ => {
                    if low != (kind != Ambient::Snow) {
                        continue;
                    }
                    let (c, a, r) =
                        if kind == Ambient::Snow { ([235, 245, 255], 150.0, 2.0) } else { (th.dust, 70.0, 1.0) };
                    let alpha = a * fade / 255.0;
                    if alpha > 0.03 {
                        g.puff(p.x, p.y, r, c, alpha);
                    }
                }
            }
        }
    }

    pub(super) fn update_flying(&mut self, dt: f32, arena: &Arena) {
        let (x0, y0, x1, y1) = arena.bounds();
        let (x0, y0, x1, y1) = (x0 as f32 + 10.0, y0 as f32 + 10.0, x1 as f32 - 10.0, y1 as f32 - 10.0);
        let mut landed = Vec::new();
        self.flying.retain_mut(|f| {
            f.age += dt;
            f.x += f.vx * dt;
            f.y += f.vy * dt;
            f.a += f.spin * dt;
            if !(x0 < f.x && f.x < x1) {
                // prallt an der Außenwand ab
                f.vx = -f.vx;
                f.x = f.x.clamp(x0, x1);
            }
            if !(y0 < f.y && f.y < y1) {
                f.vy = -f.vy;
                f.y = f.y.clamp(y0, y1);
            }
            if f.age < TURRET_AIR {
                return true;
            }
            landed.push((f.x, f.y, f.a, f.slot));
            false
        });
        for (x, y, a, slot) in landed {
            let (px, py) = panzer_core::geom::push_out(x as f64, y as f64, 9.0, arena.near(x as f64, y as f64));
            let (px, py) = (px as f32, py as f32);
            let cv = crate::raster::rotate_canvas(&self.turret_cv[slot.min(MAX_PLAYERS - 1)], a);
            if let Some(m) = self.maze.as_mut() {
                m.stamp(&cv, px, py);
            }
            self.parts.burst(P::Dust, px, py, 8.0, 70.0, 0.7, 6.0, o(arena.theme().dust).drag(3.0));
            self.parts.burst(P::Spark, px, py, 5.0, 160.0, 0.25, 2.0, Opt::default().drag(5.0));
            self.sound("clank", Some(px), 1.0);
        }
    }

    pub(super) fn draw_flying(&self, g: &Gfx) {
        for f in &self.flying {
            let k = f.age / TURRET_AIR;
            let h = 4.0 * k * (1.0 - k); // Höhe 0..1..0
            g.shadow(f.x + 6.0 + 14.0 * h, f.y + 6.0 + 22.0 * h, 9.0, 1.0);
            g.sprite(
                &g.spr.turrets[f.slot.min(MAX_PLAYERS - 1)],
                f.x,
                f.y - 30.0 * h,
                f.a,
                1.0 + 0.6 * h,
                macroquad::prelude::WHITE,
            );
        }
    }

    /// Partikel sortieren: unter den Panzern, darüber, ins Licht
    #[allow(clippy::type_complexity)]
    pub(super) fn collect(
        &mut self,
        g: &mut Gfx,
    ) -> (
        Vec<(f32, f32, f32, [u8; 3], f32)>,
        Vec<([u8; 3], [(f32, f32); 4])>,
        Vec<(f32, f32, f32, [u8; 3], f32)>,
        Vec<([u8; 3], [(f32, f32); 4])>,
    ) {
        let (mut low, mut polys_low, mut high, mut polys_high) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for p in &self.parts.items {
            let k = p.age / p.life;
            let (x, y) = (p.x, p.y);
            match p.kind {
                P::Fire => g.light(FIRE_GRAD[((k * 7.0) as usize).min(6)], x, y, p.size * (0.55 + 0.9 * k), 7.0),
                P::Spark => {
                    let c = FIRE_GRAD[((k * 6.0) as usize).min(5)];
                    g.lit_line(col(c), x, y, x - p.vx * 0.03, y - p.vy * 0.03, 2.0);
                }
                P::Smoke => {
                    let r = (p.size * (0.6 + 1.1 * k)).max(2.0);
                    let a = 150.0 * (1.0 - k) * (k * 8.0 + 0.25).min(1.0) / 255.0;
                    if a > 0.02 {
                        high.push((x, y, r, p.color, a));
                    }
                }
                P::Ember | P::Star => {
                    let tw = if p.kind == P::Ember { 1.0 } else { 0.55 + 0.45 * (p.age * 18.0 + p.angle).sin() };
                    g.light(p.color, x, y, p.size * 2.2 + 2.0, 8.0 * (1.0 - k) * tw);
                }
                P::Flash => g.light(p.color, x, y, p.size * (0.8 + 0.4 * k), 8.0 * (1.0 - k).powf(1.5)),
                P::Dust => {
                    let r = (p.size * (0.6 + k)).max(1.0);
                    let a = 100.0 * (1.0 - k) / 255.0;
                    if a > 0.02 {
                        low.push((x, y, r, p.color, a));
                    }
                }
                P::Debris => polys_low.push((p.color, rot_rect(x, y, p.size, p.size * 0.65, p.angle))),
                P::Shard => {
                    let c = shade(p.color, 1.0 - k);
                    g.lit_line(col(c), x, y, x - p.vx * 0.035, y - p.vy * 0.035, 2.0);
                }
                P::Confetti => {
                    let w = p.size * (0.25 + 0.75 * (p.age * 7.0 + p.spin).cos().abs());
                    polys_high.push((p.color, rot_rect(x, y, p.size, w * 0.6, p.angle)));
                }
                P::Ring => {
                    let c = shade(p.color, (1.0 - k).powf(1.2));
                    g.lit_ring(col(c), x, y, p.size * ease_out(k), (4.0 * (1.0 - k)).round().max(1.0));
                }
            }
        }
        (low, polys_low, high, polys_high)
    }
}
