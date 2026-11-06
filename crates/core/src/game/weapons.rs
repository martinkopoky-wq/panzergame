//! Abfeuern, Sofort-Waffen, Explosionen, Power-ups aufheben.

use std::f64::consts::{PI, TAU};

use super::{Game, TempWall};
use crate::consts::*;
use crate::geom::{angle_diff, circle_hits_wall, hypot, seg_dist};
use crate::items::{box_pool, box_weapons, buffs, Eff, Item, Kind};
use crate::physics::{ahead, bullet_params, first_wall, rail_path, strike_target, tank_radius};
use crate::snap::{q16, qp, Ann, FxData};

/// Ein Geschoss aus dem Rohr: (Art, Tempo, Radius, Lebensdauer, Schonzeit für den Schützen)
fn single_shot(w: Item) -> Option<(Kind, f64, f64, f64, f64)> {
    Some(match w {
        Item::Frag => (Kind::Frag, FRAG_SPEED, FRAG_R, FRAG_FUSE, 0.3),
        Item::Rocket => (Kind::Rocket, ROCKET_SPEED, ROCKET_R, ROCKET_LIFE, ROCKET_GRACE),
        Item::Bouncy => (Kind::Bnc, BOUNCY_SPEED, BOUNCY_R, BOUNCY_LIFE, 0.3),
        Item::Swap => (Kind::Swap, SWAP_SPEED, SWAP_R, SWAP_LIFE, 0.4),
        Item::Rang => (Kind::Rang, RANG_SPEED, RANG_R, RANG_LIFE, 0.5),
        Item::Ice => (Kind::Ice, ICE_SPEED, ICE_R, ICE_LIFE, OWNER_GRACE),
        Item::Hole => (Kind::Hole, HOLE_SPEED, HOLE_R, HOLE_TRAVEL + HOLE_TIME, 1e9),
        Item::Nuke => (Kind::Nuke, NUKE_SPEED, NUKE_R, NUKE_FUSE, 1e9),
        Item::Rc => (Kind::Rc, RC_SPEED, 5.0, RC_LIFE, 0.6),
        Item::Sticky => (Kind::Sticky, STICKY_SPEED, 5.0, 4.0, OWNER_GRACE),
        Item::Gl => (Kind::Gl, GL_SPEED, 5.0, GL_DIST / GL_SPEED, 1e9),
        Item::Saw => (Kind::Saw, SAW_SPEED, SAW_R, SAW_LIFE, 0.4),
        Item::Plasma => (Kind::Plasma, PLASMA_SPEED, PLASMA_R, PLASMA_LIFE, 0.4),
        Item::Cannon => (Kind::Cannon, CANNON_SPEED, CANNON_R, 7.0, 0.4),
        Item::Gold => (Kind::Gold, GOLD_SPEED, 5.0, GOLD_LIFE, 0.4),
        Item::Fw => (Kind::Fw, FW_SPEED, 4.0, FW_FUSE, 0.3),
        Item::Seek => (Kind::Seek, SEEK_SPEED, 4.0, SEEK_LIFE, 0.3),
        Item::Phase => (Kind::Phase, PHASE_SPEED, 4.0, PHASE_LIFE, OWNER_GRACE),
        Item::Split => (Kind::Split, SPLIT_SPEED, 4.0, SPLIT_LIFE, OWNER_GRACE),
        Item::Wave => (Kind::Wave, WAVE_SPEED, 4.0, WAVE_LIFE, OWNER_GRACE),
        Item::Napalm => (Kind::Napalm, NAPALM_SPEED, 5.0, NAPALM_FUSE, 0.4),
        Item::Hook => (Kind::Hook, HOOK_SPEED, 5.0, HOOK_LIFE, 1e9),
        Item::Dist => (Kind::Dist, DIST_SPEED, DIST_R, DIST_LIFE, 0.3),
        Item::Eater => (Kind::Eater, EATER_SPEED, EATER_R, EATER_LIFE, OWNER_GRACE),
        Item::Double => (Kind::Double, DOUBLE_SPEED, DOUBLE_R, DOUBLE_LIFE, 0.3),
        Item::Corner => (Kind::Corner, CORNER_SPEED, CORNER_R, CORNER_LIFE, 0.3),
        _ => return None,
    })
}

/// Mehrere auf einmal: (Art, Anzahl, Fächer +- rad oder None = rundherum, Tempo, Radius, Leben, Schonzeit)
#[allow(clippy::type_complexity)]
fn fan_shot(w: Item) -> Option<(Kind, usize, Option<f64>, f64, f64, f64, f64)> {
    Some(match w {
        Item::Shotgun => (Kind::Sg, SG_PELLETS, Some(SG_SPREAD), BULLET_SPEED * 1.22, 3.0, SG_LIFE, OWNER_GRACE),
        Item::Swarm => (Kind::Mrk, SWARM_N, Some(0.5), SWARM_SPEED, 3.0, SWARM_LIFE, 0.5),
        Item::Nova => (Kind::Nv, NOVA_N, None, BULLET_SPEED * 1.1, 3.0, 4.0, OWNER_GRACE),
        _ => return None,
    })
}

/// Hinter den Panzer gelegt: (Art, Radius, Lebensdauer)
fn laid(w: Item) -> Option<(Kind, f64, f64)> {
    Some(match w {
        Item::Mine => (Kind::Mine, MINE_R, 1e9),
        Item::Trap => (Kind::Trap, MINE_R, 1e9),
        Item::Oil => (Kind::Oil, OIL_R, OIL_LIFE),
        Item::Spider => (Kind::Spider, 6.0, SPIDER_LIFE),
        Item::Sentry => (Kind::Sentry, SENTRY_R, SENTRY_LIFE),
        Item::Timebomb => (Kind::Timebomb, 8.0, TIMEBOMB_FUSE),
        Item::Fence => (Kind::Fence, 6.0, FENCE_ARM + FENCE_TIME),
        Item::Sprinkler => (Kind::Sprinkler, SENTRY_R, SPRINKLER_ARM + SPRINKLER_TIME),
        _ => return None,
    })
}

impl Game {
    pub(super) fn trigger(&mut self, ti: usize) {
        let slot = self.tanks[ti].slot;
        // 2. Druck: Eckenschuss biegt rechts ab (einmal)
        if let Some(si) = self.shots.iter().position(|s| {
            s.owner == slot && !s.dead && s.kind == Kind::Corner && s.cool == 0.0
        }) {
            let s = &mut self.shots[si];
            let a = s.heading() + PI / 2.0;
            s.vx = a.cos() * s.speed;
            s.vy = a.sin() * s.speed;
            s.cool = 1.0;
            return;
        }
        // 2. Druck: Splitterbombe zünden bzw. zur Tauschkugel springen
        if let Some(si) =
            self.shots.iter().position(|s| s.owner == slot && !s.dead && matches!(s.kind, Kind::Frag | Kind::Swap))
        {
            if self.shots[si].kind == Kind::Frag {
                self.explode(si);
            } else {
                self.teleport(si);
            }
            return;
        }
        let w = self.tanks[ti].weapon;
        if self.tanks[ti].cooldown > 0.0 || w.is_some_and(|w| w.is_hold()) || self.in_wall(ti) {
            return;
        }
        let a = self.tanks[ti].a;
        let Some(w) = w else {
            let bp = bullet_params(self.mutator);
            let rapid = self.tanks[ti].has(Eff::Rapid);
            let mine = self.shots.iter().filter(|s| s.owner == slot && s.kind == Kind::B).count();
            if mine >= bp.most * if rapid { 2 } else { 1 } {
                return;
            }
            let offs: &[f64] = if self.tanks[ti].has(Eff::Twin) { &[-0.07, 0.07] } else { &[0.0] };
            for &off in offs {
                self.shoot(Kind::B, ti, a + off, bp.speed, bp.r, bp.life, OWNER_GRACE);
            }
            self.tanks[ti].cooldown = FIRE_COOLDOWN * if rapid { 0.5 } else { 1.0 };
            return;
        };
        if let Some((kind, speed, r, life, grace)) = single_shot(w) {
            self.shoot(kind, ti, a, speed, r, life, grace);
            if w == Item::Rc {
                self.tanks[ti].eff.set(Eff::Steer, RC_LIFE);
            }
        } else if let Some((kind, n, spread, speed, r, life, grace)) = fan_shot(w) {
            for k in 0..n {
                let off = match spread {
                    None => TAU * k as f64 / n as f64,
                    Some(s) => s * (2.0 * k as f64 / (n - 1) as f64 - 1.0),
                };
                let aa = a + off + self.rng.uniform(-0.04, 0.04);
                let v = speed * self.rng.uniform(0.9, 1.1);
                let lk = self.rng.uniform(0.85, 1.1);
                self.shoot(kind, ti, aa, v, r, life * lk, grace);
            }
        } else if let Some((kind, r, life)) = laid(w) {
            let (x, y) = self.behind(ti, r.min(14.0));
            let si = self.place(kind, slot, x, y, r, life);
            if kind == Kind::Sentry {
                self.shots[si].cool = 0.8;
                self.shots[si].ang = a;
            } else if kind == Kind::Spider {
                self.shots[si].grace = 1e9; // nur Gegner lösen sie aus
            } else if kind == Kind::Fence {
                self.shots[si].ang = a + PI / 2.0; // quer zur Fahrtrichtung
            } else if kind == Kind::Sprinkler {
                self.shots[si].cool = SPRINKLER_ARM;
                self.shots[si].ang = a;
            }
        } else {
            match w {
                Item::Laser => self.laser(ti),
                Item::Zap => self.zap(ti),
                Item::Push => self.push(ti),
                Item::Frost => self.frost(ti),
                Item::Prism => self.prism(ti),
                Item::Jump => self.jump(ti),
                Item::Wall => {
                    if !self.build_wall(ti) {
                        return; // kein Platz: Ladung bleibt
                    }
                }
                Item::Ram => self.tanks[ti].eff.set(Eff::Ram, RAM_TIME),
                Item::Shear => self.shear(ti),
                Item::Chain => self.chain(ti),
                Item::Pause => self.pause(ti),
                Item::Hammer => {
                    let t = &self.tanks[ti];
                    let (x, y, a) = (t.x, t.y, t.a);
                    let si = self.place(Kind::Hammer, slot, x, y, 0.0, HAMMER_WINDUP);
                    self.shots[si].ang = a;
                }
                Item::Decoy => {
                    let t = &self.tanks[ti];
                    let (x, y) = ahead(&self.arena, t.x, t.y, t.a, DECOY_THROW, 12.0);
                    self.place(Kind::Decoy, slot, x, y, 0.0, DECOY_LIFE);
                }
                Item::Switch => {
                    let t = &self.tanks[ti];
                    let (x, y) = ahead(&self.arena, t.x, t.y, t.a, SWITCH_DIST, 12.0);
                    self.place(Kind::Switch, slot, x, y, SWITCH_R, SWITCH_LIFE);
                }
                Item::Afterburn => {
                    let t = &self.tanks[ti];
                    let (x, y, a) = (t.x, t.y, t.a);
                    let si = self.place(Kind::Afterburn, slot, x, y, 0.0, AFTERBURN_FUSE);
                    self.shots[si].ang = a;
                }
                Item::Thorn => {
                    let (x, y) = self.behind(ti, 14.0);
                    let si = self.place(Kind::Thorn, slot, x, y, 0.0, THORN_LIFE);
                    self.shots[si].ang = a;
                }
                Item::Plate => {
                    let (x, y) = self.behind(ti, PLATE_R.min(14.0));
                    self.place(Kind::Plate, slot, x, y, PLATE_R, PLATE_LIFE);
                }
                Item::Pincer => self.pincer(ti),
                Item::Verdict => self.verdict(ti),
                Item::Catcher => {
                    let (x, y) = self.behind(ti, 14.0);
                    self.place(Kind::Catcher, slot, x, y, CATCHER_R, CATCHER_LIFE);
                }
                Item::Parry => self.parry(ti),
                Item::Rust => {
                    let t = &self.tanks[ti];
                    let (x, y) = ahead(&self.arena, t.x, t.y, t.a, RUST_THROW, 12.0);
                    self.place(Kind::Rust, slot, x, y, RUST_R, RUST_LIFE);
                }
                Item::Cushion => {
                    let t = &self.tanks[ti];
                    let (x, y) = ahead(&self.arena, t.x, t.y, t.a, CUSHION_THROW, 12.0);
                    self.place(Kind::Cushion, slot, x, y, CUSHION_R, CUSHION_LIFE);
                }
                Item::Cracker => {
                    let (x, y) = self.behind(ti, 6.0);
                    self.place(Kind::Cracker, slot, x, y, 0.0, CRACKER_FUSE);
                }
                Item::Cross => self.cross(ti),
                Item::Pendel => {
                    let t = &self.tanks[ti];
                    let (x, y) = ahead(&self.arena, t.x, t.y, t.a, 40.0, 12.0);
                    self.place(Kind::Pendel, slot, x, y, 0.0, PENDEL_WARN + PENDEL_LIFE);
                }
                Item::Shadow => {
                    let t = &self.tanks[ti];
                    let (x, y, a) = (t.x, t.y, t.a);
                    let si = self.place(Kind::Shadow, slot, x, y, 0.0, SHADOW_FUSE);
                    self.shots[si].ang = a;
                }
                Item::Twist => {
                    let t = &self.tanks[ti];
                    let (x, y) = (t.x, t.y);
                    self.place(Kind::Twist, slot, x, y, 0.0, TWIST_FUSE);
                }
                Item::Disarm => self.disarm(ti),
                Item::Ring => {
                    let t = &self.tanks[ti];
                    let (x, y) = ahead(&self.arena, t.x, t.y, t.a, RING_THROW, 12.0);
                    self.place(Kind::Ring, slot, x, y, 0.0, RING_FUSE);
                }
                Item::Strike => {
                    let (t, arena) = (&self.tanks[ti], &self.arena);
                    let (x, y) = strike_target(arena, t.x, t.y, t.a);
                    self.place(Kind::Strike, slot, x, y, 0.0, STRIKE_DELAY);
                }
                Item::Arty => {
                    for (j, &d) in ARTY_DIST.iter().enumerate() {
                        let t = &self.tanks[ti];
                        let (x, y) = ahead(&self.arena, t.x, t.y, t.a, d, 12.0);
                        self.place(Kind::Arty, slot, x, y, 0.0, 1.3 + 0.35 * j as f64);
                    }
                }
                Item::Orbit => {
                    let t = &self.tanks[ti];
                    let (x, y) = ahead(&self.arena, t.x, t.y, t.a, ORBIT_DIST, 12.0);
                    let si = self.place(Kind::Orbit, slot, x, y, ORBIT_R, ORBIT_DELAY + ORBIT_TIME);
                    self.shots[si].ang = a;
                }
                Item::Destruct => {
                    let (x, y) = (self.tanks[ti].x, self.tanks[ti].y);
                    let si = self.place(Kind::Destruct, slot, x, y, 0.0, DESTRUCT_FUSE);
                    self.shots[si].stuck = Some(slot as i8); // fährt mit dem Panzer mit
                }
                _ => {}
            }
        }
        self.use_ammo(ti);
        let slow = laid(w).is_some() || matches!(w, Item::Wall | Item::Jump | Item::Push);
        self.tanks[ti].cooldown = if slow { 0.35 } else { FIRE_COOLDOWN };
    }

    /// Geschossschere: schneidet eigene Gegnerkugeln im Kegel vor dem Panzer ab (bis SHEAR_MAX).
    pub(super) fn shear(&mut self, ti: usize) {
        let (sx, sy, a, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].a, self.tanks[ti].slot);
        let mut left = SHEAR_MAX;
        for si in 0..self.shots.len() {
            if left == 0 {
                break;
            }
            let s = &self.shots[si];
            if s.dead || s.owner == slot || !matches!(s.kind, Kind::B | Kind::Mg | Kind::Sg | Kind::Bnc | Kind::Nv | Kind::Mrk) {
                continue;
            }
            if hypot(s.x - sx, s.y - sy) > SHEAR_RANGE || angle_diff((s.y - sy).atan2(s.x - sx), a).abs() > SHEAR_ARC / 2.0 {
                continue;
            }
            self.shots[si].dead = true;
            left -= 1;
        }
    }

    /// Entwaffnungsfächer: Gegner im Kegel verlieren ihre Waffe, sie liegt als Kiste am Boden.
    pub(super) fn disarm(&mut self, ti: usize) {
        let (sx, sy, a) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].a);
        for vi in 0..self.tanks.len() {
            let v = &self.tanks[vi];
            if vi == ti || !v.alive || v.weapon.is_none() {
                continue;
            }
            if hypot(v.x - sx, v.y - sy) > DISARM_RANGE || angle_diff((v.y - sy).atan2(v.x - sx), a).abs() > DISARM_HALF {
                continue;
            }
            let (x, y) = (v.x, v.y);
            if let Some(w) = self.tanks[vi].weapon.take() {
                self.boxes.push((x, y, w));
                self.tanks[vi].ammo = 0;
                self.tanks[vi].nopick = DISARM_PICK_WAIT;
            }
        }
    }

    /// Kettenzange: packt den ersten Gegner im Frontwinkel. Beide stehen HOLD still, dann trifft die Zange.
    pub(super) fn chain(&mut self, ti: usize) {
        let (sx, sy, a, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].a, self.tanks[ti].slot);
        let victim = (0..self.tanks.len()).find(|&vi| {
            let v = &self.tanks[vi];
            vi != ti
                && v.alive
                && hypot(v.x - sx, v.y - sy) <= CHAIN_RANGE + v.radius()
                && angle_diff((v.y - sy).atan2(v.x - sx), a).abs() <= CHAIN_HALF
        });
        let Some(vi) = victim else { return };
        let (vx, vy, vslot) = (self.tanks[vi].x, self.tanks[vi].y, self.tanks[vi].slot);
        let si = self.place(Kind::Chain, slot, vx, vy, 0.0, CHAIN_HOLD);
        self.shots[si].stuck = Some(vslot as i8);
        self.freeze(ti, CHAIN_HOLD);
        self.freeze(vi, CHAIN_HOLD);
    }

    /// Zündpause: alle normalen Kugeln im Umkreis bleiben PAUSE_TIME lang stehen.
    pub(super) fn pause(&mut self, ti: usize) {
        let (x, y) = (self.tanks[ti].x, self.tanks[ti].y);
        for s in self.shots.iter_mut() {
            if !s.dead && matches!(s.kind, Kind::B | Kind::Mg | Kind::Sg | Kind::Bnc) && hypot(s.x - x, s.y - y) <= PAUSE_RANGE {
                s.cool = PAUSE_TIME;
            }
        }
    }

    /// Zangenmarkierung: zwei Zangen starten seitlich vor dem Ziel und laufen zur Mitte.
    pub(super) fn pincer(&mut self, ti: usize) {
        let (t, slot) = (&self.tanks[ti], self.tanks[ti].slot);
        let (cx, cy) = ahead(&self.arena, t.x, t.y, t.a, PINCER_DIST, 12.0);
        let (lx, ly) = (-t.a.sin(), t.a.cos()); // quer zur Blickrichtung
        for side in [1.0, -1.0] {
            let (px, py) = (cx + lx * PINCER_LAT * side, cy + ly * PINCER_LAT * side);
            let si = self.place(Kind::Pincer, slot, px, py, PINCER_R, PINCER_LIFE);
            // Richtung zur Mitte, Tempo, zurückgelegte Strecke (in cool)
            let (dx, dy) = (cx - px, cy - py);
            let d = dx.hypot(dy).max(1e-9);
            let s = &mut self.shots[si];
            s.vx = dx / d;
            s.vy = dy / d;
            s.speed = PINCER_SPEED;
        }
    }

    /// Fadenkreuzfluch: sperrt einen sichtbaren Gegner ein, friert nach CROSS_FOLLOW ein.
    pub(super) fn cross(&mut self, ti: usize) {
        let (x, y, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].slot);
        let target = self.nearest(x, y, Some(slot), CROSS_RANGE, true);
        let (tx, ty, tslot) = match target {
            Some(vi) => (self.tanks[vi].x, self.tanks[vi].y, Some(self.tanks[vi].slot as i8)),
            None => {
                let t = &self.tanks[ti];
                let (ax, ay) = ahead(&self.arena, t.x, t.y, t.a, CROSS_RANGE, 12.0);
                (ax, ay, None)
            }
        };
        let si = self.place(Kind::Cross, slot, tx, ty, CROSS_R, CROSS_FOLLOW + CROSS_FREEZE);
        self.shots[si].stuck = tslot;
    }

    /// Stillstandsurteil: markiert den nächsten sichtbaren Gegner.
    pub(super) fn verdict(&mut self, ti: usize) {
        let (x, y, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].slot);
        let Some(vi) = self.nearest(x, y, Some(slot), VERDICT_RANGE, true) else { return };
        let (tx, ty, vslot) = (self.tanks[vi].x, self.tanks[vi].y, self.tanks[vi].slot);
        let si = self.place(Kind::Verdict, slot, tx, ty, 0.0, VERDICT_LIFE);
        let s = &mut self.shots[si];
        s.stuck = Some(vslot as i8);
        s.vx = tx; // Ruhepunkt
        s.vy = ty;
    }

    /// Konterklinge: wehrt eine Kugel im Frontwinkel ab. Nur dann folgt der Stoß.
    pub(super) fn parry(&mut self, ti: usize) {
        let (x, y, a, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].a, self.tanks[ti].slot);
        let front = |px: f64, py: f64| {
            angle_diff((py - y).atan2(px - x), a).abs() <= PARRY_HALF
        };
        let bullet = self.shots.iter().position(|s| {
            !s.dead
                && s.owner != slot
                && matches!(s.kind, Kind::B | Kind::Mg | Kind::Sg | Kind::Bnc)
                && hypot(s.x - x, s.y - y) <= PARRY_REACH
                && front(s.x, s.y)
        });
        let Some(bi) = bullet else { return };
        self.shots[bi].dead = true;
        let victim = (0..self.tanks.len()).find(|&vi| {
            let v = &self.tanks[vi];
            vi != ti && v.alive && hypot(v.x - x, v.y - y) <= LUNGE_REACH + v.radius() && front(v.x, v.y)
        });
        if let Some(vi) = victim {
            let ang = (self.tanks[vi].y - y).atan2(self.tanks[vi].x - x);
            self.hit(vi, Some(slot), Kind::Parry, false, Some(ang), 0);
        }
    }

    /// Platz hinter dem Panzer (Minen, Öl, Fallen), aber nicht durch eine Wand.
    fn behind(&self, ti: usize, r: f64) -> (f64, f64) {
        let t = &self.tanks[ti];
        let back = t.a + PI;
        let d = (t.radius() + r + 3.0).min(first_wall(&self.arena, t.x, t.y, back, 40.0) - r - 1.0);
        (t.x + back.cos() * d.max(0.0), t.y + back.sin() * d.max(0.0))
    }

    /// Wandbauer: ein Wandstück quer hinter den Panzer. False, wenn kein Platz ist.
    fn build_wall(&mut self, ti: usize) -> bool {
        let t = &self.tanks[ti];
        let back = t.a + PI;
        let d = t.radius() + 16.0;
        let (cx, cy) = (t.x + back.cos() * d, t.y + back.sin() * d);
        let (w, h) = if t.a.cos().abs() > t.a.sin().abs() {
            (WALL, BUILD_LEN) // fährt waagrecht -> Wand senkrecht
        } else {
            (BUILD_LEN, WALL)
        };
        let a = &self.arena;
        let x = (cx - w / 2.0).max(a.ox).min(a.ox + a.cols as f64 * CELL - w);
        let y = (cy - h / 2.0).max(a.oy).min(a.oy + a.rows as f64 * CELL - h);
        let rect = [x.round(), y.round(), w, h];
        for o in &self.tanks {
            if o.alive && circle_hits_wall(o.x, o.y, o.radius(), &rect) {
                return false;
            }
        }
        for s in self.shots.iter_mut() {
            if !s.kind.is_static() && circle_hits_wall(s.x, s.y, s.r, &rect) {
                s.dead = true;
            }
        }
        self.temp.push(TempWall { rect: [rect[0] as i32, rect[1] as i32, w as i32, h as i32], left: BUILD_TIME });
        self.rebuild_walls();
        self.fx(FxData::Build { x: q16(x + w / 2.0), y: q16(y + h / 2.0) }, 0.4);
        true
    }

    /// Wände neu aus dem Raster plus den gebauten Wänden. Höhere rev -> der Server
    /// verschickt das Maze, Client und Renderer tauschen die Arena mitten in der Runde.
    pub(super) fn rebuild_walls(&mut self) {
        self.maze.extra = self.temp.iter().map(|w| w.rect).collect();
        self.maze.rev = self.maze.rev.wrapping_add(1);
        self.arena = crate::arena::Arena::new(&self.maze);
    }

    pub(super) fn rail(&mut self, ti: usize) {
        let (tx, ty, ta, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].a, self.tanks[ti].slot);
        let ((ex, ey), holes) = rail_path(&self.arena, tx, ty, ta);
        let (x0, y0) = (tx + ta.cos() * MUZZLE, ty + ta.sin() * MUZZLE);
        for oi in 0..self.tanks.len() {
            let o = &self.tanks[oi];
            if o.alive && oi != ti && seg_dist(o.x, o.y, x0, y0, ex, ey) < o.radius() + 3.0 {
                self.hit(oi, Some(slot), Kind::Rail, false, Some(ta), 0);
            }
        }
        for si in 0..self.shots.len() {
            // zerstört auch Geschütztürme in der Schusslinie
            let s = &self.shots[si];
            if s.kind == Kind::Sentry && seg_dist(s.x, s.y, x0, y0, ex, ey) < SENTRY_R {
                self.sentry_boom(si);
            }
        }
        let holes = holes.iter().map(|&(x, y)| (x as i16, y as i16)).collect();
        self.fx(FxData::Rail { slot: slot as u8, p0: qp(x0, y0), p1: qp(ex, ey), holes }, RAIL_SHOW);
    }

    fn targets(&self) -> (Vec<usize>, Vec<(f64, f64, f64)>) {
        let idx: Vec<usize> = (0..self.tanks.len()).filter(|&i| self.tanks[i].alive).collect();
        let circles = idx.iter().map(|&i| (self.tanks[i].x, self.tanks[i].y, self.tanks[i].radius())).collect();
        (idx, circles)
    }

    /// Prisma: ein Strahl, der sich an der ersten Wand in drei aufspaltet.
    fn prism(&mut self, ti: usize) {
        let (tx, ty, ta, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].a, self.tanks[ti].slot);
        let (targets, circles) = self.targets();
        let (dx, dy) = (ta.cos(), ta.sin());
        let (d, axis) = self.arena.first_wall_dir(tx, ty, dx, dy, LASER_LEN);
        let (pts, hit) = self.arena.raycast(tx, ty, ta, d, &circles);
        let mut hits: Vec<(usize, f64, u32)> = Vec::new();
        let mut beams = vec![pts.clone()];
        if let Some(h) = hit {
            hits.push((targets[h], ta, 0));
        } else if let Some(axis) = axis {
            let rx = if axis.has_x() { -dx } else { dx };
            let ry = if axis.has_y() { -dy } else { dy };
            let base = ry.atan2(rx);
            let last = pts[pts.len() - 1];
            let (hx, hy) = (last.0 + rx * 0.5, last.1 + ry * 0.5);
            for off in [-PRISM_SPLIT, 0.0, PRISM_SPLIT] {
                let (p2, h2) = self.arena.raycast(hx, hy, base + off, PRISM_LEN, &circles);
                if let Some(h2) = h2 {
                    let (x0, y0) = p2[p2.len() - 2];
                    let (x1, y1) = p2[p2.len() - 1];
                    hits.push((targets[h2], (y1 - y0).atan2(x1 - x0), p2.len() as u32));
                }
                beams.push(p2);
            }
        }
        for (oi, a, b) in hits {
            self.hit(oi, Some(slot), Kind::Prism, false, Some(a), b);
        }
        let beams = beams.iter().map(|b| b.iter().map(|&(x, y)| qp(x, y)).collect()).collect();
        self.fx(FxData::Prism { slot: slot as u8, beams }, LASER_SHOW);
    }

    /// Druckwelle: schiebt Panzer im Kegel weg und lenkt Geschosse um (gehören dann einem).
    fn push(&mut self, ti: usize) {
        let (tx, ty, ta, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].a, self.tanks[ti].slot);
        for oi in 0..self.tanks.len() {
            let o = &self.tanks[oi];
            if !o.alive || oi == ti {
                continue;
            }
            let (dx, dy) = (o.x - tx, o.y - ty);
            let d = hypot(dx, dy);
            if d > PUSH_RANGE
                || d < 1.0
                || angle_diff(dy.atan2(dx), ta).abs() > PUSH_CONE
                || !self.arena.clear(tx, ty, o.x, o.y)
            {
                continue;
            }
            let (ux, uy) = (dx / d, dy / d);
            let n = ((PUSH_DIST * (1.0 - 0.4 * d / PUSH_RANGE)) / 8.0).floor() as i32;
            for _ in 0..n {
                let (ox, oy) = (self.tanks[oi].x, self.tanks[oi].y);
                let (nx, ny) = self.settle(oi, ox + ux * 8.0, oy + uy * 8.0);
                if hypot(nx - ox, ny - oy) < 2.0 {
                    break;
                }
                self.tanks[oi].x = nx;
                self.tanks[oi].y = ny;
            }
        }
        for s in self.shots.iter_mut() {
            if s.dead || s.kind.is_static() || s.speed <= 0.0 {
                continue;
            }
            let (dx, dy) = (s.x - tx, s.y - ty);
            let d = hypot(dx, dy);
            if d > PUSH_RANGE || d < 1.0 {
                continue;
            }
            let a = dy.atan2(dx);
            if angle_diff(a, ta).abs() > PUSH_CONE {
                continue;
            }
            s.vx = a.cos() * s.speed;
            s.vy = a.sin() * s.speed;
            s.owner = slot;
            s.grace = s.age + 0.3;
        }
        self.fx(FxData::Push { slot: slot as u8, x: q16(tx), y: q16(ty), a: (ta * 100.0).round() as i16 }, 0.45);
    }

    fn frost(&mut self, ti: usize) {
        let (tx, ty) = (self.tanks[ti].x, self.tanks[ti].y);
        for oi in 0..self.tanks.len() {
            let o = &self.tanks[oi];
            if !o.alive || oi == ti {
                continue;
            }
            if hypot(o.x - tx, o.y - ty) < FROST_R + o.radius() {
                self.freeze(oi, FREEZE_TIME);
            }
        }
        self.fx(FxData::Frost { x: q16(tx), y: q16(ty) }, 0.6);
    }

    /// Einfrieren, außer ein Schild oder der Superstern schützt.
    pub(super) fn freeze(&mut self, ti: usize, length: f64) {
        let (x, y) = qp(self.tanks[ti].x, self.tanks[ti].y);
        if self.tanks[ti].shield > 0.0 {
            self.tanks[ti].shield = 0.0;
            self.fx(FxData::Block { x, y }, 0.6);
        } else if !self.tanks[ti].has(Eff::Star) {
            self.tanks[ti].eff.set(Eff::Frozen, length);
            self.fx(FxData::Freeze { x, y }, 0.5);
        }
    }

    /// Sprungdüse: ein Satz nach vorne, auch über eine Wand.
    fn jump(&mut self, ti: usize) {
        let t = &self.tanks[ti];
        let (x0, y0) = (t.x, t.y);
        let (x, y) = ahead(&self.arena, t.x, t.y, t.a, JUMP_DIST, t.radius());
        let (nx, ny) = self.settle(ti, x, y);
        self.tanks[ti].x = nx;
        self.tanks[ti].y = ny;
        self.fx(FxData::Jump { x0: q16(x0), y0: q16(y0), x1: q16(nx), y1: q16(ny) }, 0.5);
    }

    pub(super) fn explode(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (bx, by, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        for k in 0..FRAG_PIECES {
            let a = TAU * k as f64 / FRAG_PIECES as f64 + self.rng.uniform(-0.1, 0.1);
            let speed = self.rng.uniform(200.0, 300.0);
            let life = self.rng.uniform(0.5, 0.9);
            let id = self.new_id();
            self.shots.push(super::Shot::new(id, Kind::Frg, owner, bx, by, a, speed, 2.0, life, 0.0));
        }
        let (x, y) = qp(bx, by);
        self.fx(FxData::Boom { x, y, size: 100 }, 0.5);
    }

    /// Explosion: trifft alle Panzer im Radius, Wände schützen nicht. size 0 = ohne Feuerball.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn blast(&mut self, x: f64, y: f64, radius: f64, by: Option<usize>, how: Kind, size: f64, pierce: bool) {
        if size > 0.0 {
            let (qx, qy) = qp(x, y);
            self.fx(FxData::Boom { x: qx, y: qy, size: (size * 100.0).round() as u8 }, 0.6);
        }
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if t.alive && hypot(t.x - x, t.y - y) < radius + t.radius() {
                let a = (t.y - y).atan2(t.x - x);
                self.hit(ti, by, how, pierce, Some(a), 0);
            }
        }
        for si in 0..self.shots.len() {
            // Öl in der Nähe fängt Feuer
            let o = &self.shots[si];
            if o.kind == Kind::Oil && hypot(o.x - x, o.y - y) < radius + OIL_R {
                self.ignite(si);
            }
        }
    }

    /// Atombombe: riesige Explosion, auch durch Wände und Schilde. Reißt Minen, Bomben
    /// und die Wände in der Nähe mit (die Außenwände bleiben).
    pub(super) fn nuke(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (x, y, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        self.blow_walls(x, y, NUKE_BLAST * 0.7);
        let (qx, qy) = qp(x, y);
        self.fx(FxData::Nuke { x: qx, y: qy }, 2.5);
        self.slowmo(1.2);
        let n = self.shots.len();
        for oi in 0..n {
            let o = &self.shots[oi];
            if o.dead || hypot(o.x - x, o.y - y) > NUKE_BLAST {
                continue;
            }
            match o.kind {
                Kind::Mine => self.mine_boom(oi),
                Kind::Frag => self.explode(oi),
                _ => {}
            }
        }
        self.blast(x, y, NUKE_BLAST, Some(owner), Kind::Nuke, 0.0, true);
    }

    /// Innere Wandstücke im Umkreis entfernen. Gleiche Maze-id, höhere rev -> der Server
    /// verschickt es neu, Client und Renderer tauschen die Arena mitten in der Runde.
    pub(super) fn blow_walls(&mut self, x: f64, y: f64, radius: f64) {
        let m = &mut self.maze;
        let (ox, oy) = (m.ox as f64, m.oy as f64);
        let mut gone = 0;
        for r in 1..m.rows {
            for c in 0..m.cols {
                if m.h[r][c] && hypot(ox + (c as f64 + 0.5) * CELL - x, oy + r as f64 * CELL - y) < radius {
                    m.h[r][c] = false;
                    gone += 1;
                }
            }
        }
        for r in 0..m.rows {
            for c in 1..m.cols {
                if m.v[r][c] && hypot(ox + c as f64 * CELL - x, oy + (r as f64 + 0.5) * CELL - y) < radius {
                    m.v[r][c] = false;
                    gone += 1;
                }
            }
        }
        let before = self.temp.len();
        self.temp.retain(|w| {
            let [wx, wy, ww, wh] = w.rect;
            hypot(wx as f64 + ww as f64 / 2.0 - x, wy as f64 + wh as f64 / 2.0 - y) >= radius
        });
        gone += before - self.temp.len();
        if gone > 0 {
            self.rebuild_walls();
        }
    }

    pub(super) fn teleport(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (sx, sy, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        let Some(ti) = self.tank_index(owner) else { return };
        if !self.tanks[ti].alive || self.tanks[ti].has(Eff::Frozen) {
            return;
        }
        let (x, y) = qp(self.tanks[ti].x, self.tanks[ti].y);
        self.fx(FxData::Tele { x, y }, 0.5);
        let (nx, ny) = self.settle(ti, sx, sy);
        self.tanks[ti].x = nx;
        self.tanks[ti].y = ny;
        let (x, y) = qp(nx, ny);
        self.fx(FxData::Tele { x, y }, 0.5);
    }

    /// Tauschkugel trifft Panzer ti -> Schütze und ti tauschen die Plätze.
    pub(super) fn swap(&mut self, si: usize, ti: usize) {
        let Some(oi) = self.tank_index(self.shots[si].owner) else { return };
        if !self.tanks[oi].alive || oi == ti {
            return;
        }
        let (ox, oy) = (self.tanks[oi].x, self.tanks[oi].y);
        let (tx, ty) = (self.tanks[ti].x, self.tanks[ti].y);
        let (qx, qy) = qp(ox, oy);
        self.fx(FxData::Tele { x: qx, y: qy }, 0.5);
        let (qx, qy) = qp(tx, ty);
        self.fx(FxData::Tele { x: qx, y: qy }, 0.5);
        let p1 = self.settle(oi, tx, ty);
        let p2 = self.settle(ti, ox, oy);
        self.tanks[oi].x = p1.0;
        self.tanks[oi].y = p1.1;
        self.tanks[ti].x = p2.0;
        self.tanks[ti].y = p2.1;
    }

    fn laser(&mut self, ti: usize) {
        let (tx, ty, ta, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].a, self.tanks[ti].slot);
        let (targets, circles) = self.targets();
        let (pts, hit) = self.arena.raycast(tx, ty, ta, LASER_LEN, &circles);
        if let Some(h) = hit {
            let (x0, y0) = pts[pts.len() - 2];
            let (x1, y1) = pts[pts.len() - 1];
            self.hit(targets[h], Some(slot), Kind::Laser, false, Some((y1 - y0).atan2(x1 - x0)), pts.len() as u32 - 2);
        }
        let pts = pts.iter().map(|&(x, y)| qp(x, y)).collect();
        self.fx(FxData::Laser { slot: slot as u8, pts }, LASER_SHOW);
    }

    /// Blitz: trifft den nächsten Panzer vor einem (freie Sicht) und springt weiter.
    fn zap(&mut self, ti: usize) {
        let (tx, ty, ta, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].a, self.tanks[ti].slot);
        let (mut x, mut y, mut rng) = (tx, ty, ZAP_RANGE);
        let mut pts = vec![(tx + ta.cos() * MUZZLE, ty + ta.sin() * MUZZLE)];
        let mut hit: Vec<usize> = Vec::new();
        for _ in 0..ZAP_JUMPS {
            let mut best: Option<(f64, usize)> = None;
            for (oi, o) in self.tanks.iter().enumerate() {
                if !o.alive || oi == ti || hit.contains(&oi) {
                    continue;
                }
                let d = hypot(o.x - x, o.y - y);
                if d > rng || best.is_some_and(|b| d >= b.0) {
                    continue;
                }
                if hit.is_empty() {
                    // der erste Sprung geht nur nach vorne
                    if angle_diff((o.y - y).atan2(o.x - x), ta).abs() > ZAP_CONE {
                        continue;
                    }
                }
                if self.arena.clear(x, y, o.x, o.y) {
                    best = Some((d, oi));
                }
            }
            let Some((_, oi)) = best else { break };
            hit.push(oi);
            let o = &self.tanks[oi];
            pts.push((o.x, o.y));
            x = o.x;
            y = o.y;
            rng = ZAP_CHAIN;
        }
        if hit.is_empty() {
            // ins Leere: schlägt in die nächste Wand ein
            let d = first_wall(&self.arena, tx, ty, ta, ZAP_RANGE * 0.6);
            pts.push((tx + ta.cos() * d, ty + ta.sin() * d));
        }
        let mut prev = pts[0];
        for oi in hit {
            let (ox, oy) = (self.tanks[oi].x, self.tanks[oi].y);
            self.hit(oi, Some(slot), Kind::Zap, false, Some((oy - prev.1).atan2(ox - prev.0)), 0);
            prev = (ox, oy);
        }
        let pts = pts.iter().map(|&(x, y)| qp(x, y)).collect();
        self.fx(FxData::Zap { slot: slot as u8, pts }, ZAP_SHOW);
    }

    pub(super) fn boom_shot(&mut self, si: usize, size: f64) {
        self.shots[si].dead = true;
        let s = &self.shots[si];
        let (x, y, owner, kind) = (s.x, s.y, s.owner, s.kind);
        let radius = kind.blast().unwrap_or(30.0);
        self.blast(x, y, radius, Some(owner), kind, size, false);
    }

    /// Feuerwerk: zerplatzt in bunte Funken, die jeden treffen.
    pub(super) fn burst(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (sx, sy, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        for k in 0..FW_SPARKS {
            let a = TAU * k as f64 / FW_SPARKS as f64 + self.rng.uniform(-0.1, 0.1);
            let v = self.rng.uniform(170.0, 260.0);
            let life = self.rng.uniform(0.5, 0.8);
            self.spawn(Kind::Spk, owner, sx, sy, a, v, 2.0, life, 0.0, 0.0);
        }
        self.fx(FxData::Fw { x: q16(sx), y: q16(sy) }, 0.6);
    }

    /// Napalm: das Geschoss platzt und hinterlässt eine brennende Fläche.
    pub(super) fn napalm(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (sx, sy, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        self.place(Kind::Burn, owner, sx, sy, NAPALM_R, NAPALM_BURN);
        let (x, y) = qp(sx, sy);
        self.fx(FxData::Boom { x, y, size: 70 }, 0.5);
    }

    /// Querschläger: teilt sich beim Abprallen in zwei, die Kugeln fliegen auseinander.
    pub(super) fn split(&mut self, si: usize) {
        if self.shots[si].keys >= SPLIT_GENS {
            return;
        }
        self.shots[si].keys += 1;
        let s = &self.shots[si];
        let (x, y, owner, a, speed, r, life) = (s.x, s.y, s.owner, s.heading(), s.speed, s.r, s.life);
        let (age, generation, bounces) = (s.age, s.keys, s.bounces);
        let copy = self.spawn(Kind::Split, owner, x, y, a + SPLIT_ANGLE, speed, r, life, 0.0, 0.0);
        let c = &mut self.shots[copy];
        c.age = age;
        c.keys = generation;
        c.bounces = bounces;
        let s = &mut self.shots[si];
        s.vx = (a - SPLIT_ANGLE).cos() * speed;
        s.vy = (a - SPLIT_ANGLE).sin() * speed;
    }

    pub(super) fn ignite(&mut self, si: usize) {
        let s = &mut self.shots[si];
        if s.kind == Kind::Oil {
            s.kind = Kind::Oilf;
            s.age = 0.0;
            s.life = OIL_BURN;
        }
    }

    pub(super) fn mine_boom(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (x, y, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        self.blast(x, y, MINE_BLAST, Some(owner), Kind::Mine, 0.7, false);
    }

    pub(super) fn sentry_boom(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (x, y) = qp(self.shots[si].x, self.shots[si].y);
        self.fx(FxData::Boom { x, y, size: 60 }, 0.5);
    }

    /// Teleporter: in die Zelle, die am weitesten von allen anderen weg ist.
    fn tp(&mut self, ti: usize) {
        let foes: Vec<(f64, f64)> =
            self.tanks.iter().enumerate().filter(|(i, o)| o.alive && *i != ti).map(|(_, o)| (o.x, o.y)).collect();
        let mut best: Option<(f64, f64, f64)> = None;
        for cell in self.arena.cells() {
            let (x, y) = self.arena.center(cell);
            let mut d = foes.iter().map(|&(fx, fy)| hypot(fx - x, fy - y)).fold(f64::INFINITY, f64::min);
            if d == f64::INFINITY {
                d = 0.0;
            }
            d += self.rng.uniform(0.0, 60.0);
            if best.is_none_or(|b| d > b.0) {
                best = Some((d, x, y));
            }
        }
        let Some((_, bx, by)) = best else { return };
        let (qx, qy) = qp(self.tanks[ti].x, self.tanks[ti].y);
        self.fx(FxData::Tele { x: qx, y: qy }, 0.5);
        let (nx, ny) = self.settle(ti, bx, by);
        self.tanks[ti].x = nx;
        self.tanks[ti].y = ny;
        let (qx, qy) = qp(nx, ny);
        self.fx(FxData::Tele { x: qx, y: qy }, 0.5);
    }

    /// Discokugel: alle Panzer tauschen reihum die Plätze.
    fn disco(&mut self, ti: usize) {
        let alive: Vec<usize> = (0..self.tanks.len()).filter(|&i| self.tanks[i].alive).collect();
        if alive.len() < 2 {
            return;
        }
        let pos: Vec<(f64, f64)> = alive.iter().map(|&i| (self.tanks[i].x, self.tanks[i].y)).collect();
        let k = 1 + self.rng.below(alive.len() - 1);
        for (j, &oi) in alive.iter().enumerate() {
            let (x, y) = pos[(j + k) % pos.len()];
            let (qx, qy) = qp(self.tanks[oi].x, self.tanks[oi].y);
            self.fx(FxData::Tele { x: qx, y: qy }, 0.5);
            let (nx, ny) = self.settle(oi, x, y);
            self.tanks[oi].x = nx;
            self.tanks[oi].y = ny;
        }
        let (qx, qy) = qp(self.tanks[ti].x, self.tanks[ti].y);
        self.fx(FxData::Disco { x: qx, y: qy }, 2.0);
        self.announce(Ann::Disco, self.tanks[ti].slot);
    }

    fn meteors(&mut self, ti: usize) {
        let cells = self.arena.cells();
        let pick = self.rng.sample(&cells, METEOR_N.min(cells.len()));
        let slot = self.tanks[ti].slot;
        for cell in pick {
            let (x, y) = self.arena.center(cell);
            let x = x + self.rng.uniform(-25.0, 25.0);
            let y = y + self.rng.uniform(-25.0, 25.0);
            let life = self.rng.uniform(1.0, 3.6);
            self.place(Kind::Meteor, slot, x, y, 0.0, life);
        }
        self.announce(Ann::Meteors, slot);
    }

    /// Power-up an Panzer ti. False, wenn er es nicht nehmen kann (hat schon eine Waffe).
    pub(super) fn give(&mut self, ti: usize, mut kind: Item) -> bool {
        let mut mystery = false;
        if kind == Item::Mystery {
            let roll = self.rng.random();
            kind = if roll < POTATO_CHANCE {
                Item::Potato
            } else if roll < POTATO_CHANCE + NUKE_CHANCE {
                Item::Nuke
            } else if roll < POTATO_CHANCE + NUKE_CHANCE + 0.08 {
                Item::Meteor
            } else if roll < POTATO_CHANCE + NUKE_CHANCE + 0.16 {
                Item::Gold
            } else {
                let mut pool = box_weapons();
                pool.extend(buffs());
                self.rng.choice(&pool)
            };
            mystery = true;
        }
        if kind.is_weapon() {
            if self.tanks[ti].weapon.is_some() && !mystery {
                return false;
            }
            self.tanks[ti].weapon = Some(kind);
            self.tanks[ti].ammo = kind.ammo();
        } else {
            match kind {
                Item::Shield => self.tanks[ti].shield = SHIELD_TIME,
                Item::Confuse | Item::Slow => {
                    // trifft alle anderen
                    let e = kind.effect().unwrap_or(Eff::Slow);
                    for (oi, o) in self.tanks.iter_mut().enumerate() {
                        if o.alive && oi != ti && !o.has(Eff::Star) {
                            o.eff.set(e, kind.amount());
                        }
                    }
                }
                Item::Potato => {
                    self.tanks[ti].eff.set(Eff::Potato, POTATO_TIME);
                    self.tanks[ti].eff.set(Eff::NoPass, 0.5);
                }
                Item::Tp => self.tp(ti),
                Item::Disco => self.disco(ti),
                Item::Meteor => self.meteors(ti),
                _ => {
                    if let Some(e) = kind.effect() {
                        let t = &mut self.tanks[ti];
                        match e {
                            Eff::Giant => {
                                t.eff.remove(Eff::Mini);
                            }
                            Eff::Mini => {
                                t.eff.remove(Eff::Giant);
                            }
                            _ => {}
                        }
                        t.eff.set(e, kind.amount());
                        if e == Eff::Giant {
                            self.settle_in_place(ti); // größer: aus der Wand schieben
                        }
                    }
                }
            }
        }
        let (x, y) = qp(self.tanks[ti].x, self.tanks[ti].y);
        self.fx(FxData::Pick { x, y, item: kind, mystery }, 1.3);
        true
    }

    pub(super) fn pass_potato(&mut self, i1: usize, i2: usize) {
        for (a, b) in [(i1, i2), (i2, i1)] {
            if self.tanks[a].has(Eff::Potato) && !self.tanks[b].has(Eff::Potato) && !self.tanks[a].has(Eff::NoPass) {
                let left = self.tanks[a].eff.remove(Eff::Potato);
                self.tanks[b].eff.set(Eff::Potato, left.max(POTATO_MIN));
                self.tanks[b].eff.set(Eff::NoPass, 0.8); // nicht sofort zurückgeben
                let (x, y) = qp(self.tanks[b].x, self.tanks[b].y);
                self.fx(FxData::Pick { x, y, item: Item::Potato, mystery: false }, 1.3);
                return;
            }
        }
    }

    pub(super) fn spawn_box(&mut self) {
        let mut cells = self.arena.cells();
        self.rng.shuffle(&mut cells);
        let pool = box_pool();
        for cell in cells {
            let (x, y) = self.arena.center(cell);
            if self.tanks.iter().any(|t| t.alive && hypot(t.x - x, t.y - y) < 110.0) {
                continue;
            }
            if self.boxes.iter().any(|b| hypot(b.0 - x, b.1 - y) < 1.0) {
                continue;
            }
            let item = self.rng.choice(&pool);
            self.boxes.push((x, y, item));
            return;
        }
    }

    /// Nächster Gegner für Sentry, Spinne, Suchkugel
    pub(super) fn tank_r(&self, ti: usize) -> f64 {
        tank_radius(&self.tanks[ti].eff)
    }
}
