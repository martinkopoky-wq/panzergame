//! Spielzustand -> quantisierter Snapshot für einen bestimmten Zuschauer.
//!
//! Anti-Cheat: Was der Zuschauer im echten Spiel nicht sehen kann, wird gar nicht erst
//! verschickt (getarnte Panzer ohne Radar, alles außerhalb des Nebels). Ein manipulierter
//! Client kann dann auch nichts davon anzeigen.

use std::f64::consts::TAU;

use super::Game;
use crate::consts::*;
use crate::geom::hypot;
use crate::items::{Eff, Kind, ALL_EFFS};
use crate::snap::*;

/// so weit über den Nebelradius hinaus wird noch verschickt (Rand + Interpolation)
const FOG_SEND: f64 = FOG_R + 75.0;

#[inline]
fn qpos(v: f64) -> u16 {
    (v * 4.0).round().clamp(0.0, 65535.0) as u16
}

#[inline]
fn qang(a: f64) -> u16 {
    ((crate::geom::pmod(a, TAU) / TAU) * 65536.0).round() as u32 as u16
}

#[inline]
fn ms(t: f64) -> u32 {
    (t * 1000.0).round().max(0.0) as u32
}

impl Game {
    /// Snapshot für den Spieler observer (None = alle sehen alles, z. B. lokal).
    /// cull = Verstecktes weglassen (nur im Netzspiel sinnvoll).
    pub fn net_snap(&self, observer: Option<usize>, cull: bool) -> NetSnap {
        let me = observer.and_then(|o| self.tank(o));
        let radar = me.is_some_and(|t| t.alive && t.has(Eff::Radar));
        let fog_center = match (cull, me) {
            (true, Some(t)) if t.alive && self.mutator == Some(Mutator::Nebel) => Some((t.x, t.y)),
            _ => None,
        };
        let gdeci = self.gtime * 10.0;
        let mut tanks = Vec::with_capacity(self.tanks.len());
        for (ti, t) in self.tanks.iter().enumerate() {
            let mut hidden = false;
            if cull && Some(t.slot) != observer && t.alive {
                if t.has(Eff::Invis) && !radar {
                    hidden = true;
                }
                if let Some((fx, fy)) = fog_center {
                    if hypot(t.x - fx, t.y - fy) > FOG_SEND {
                        hidden = true;
                    }
                }
            }
            let (ammo, reload) = self.reload_state(ti);
            let mut mask = 0u32;
            let mut exp = Vec::new();
            for (i, &e) in ALL_EFFS.iter().enumerate() {
                if e.hidden() || !t.eff.has(e) {
                    continue;
                }
                mask |= 1 << i;
                exp.push((gdeci + t.eff.get(e) * 10.0).round().max(1.0) as u32);
            }
            let mut flags = 0;
            if t.alive {
                flags |= T_ALIVE;
            }
            if hidden {
                flags |= T_HIDDEN;
            }
            tanks.push(NetTank {
                slot: t.slot as u8,
                flags,
                x: if hidden { 0 } else { qpos(t.x) },
                y: if hidden { 0 } else { qpos(t.y) },
                a: if hidden { 0 } else { qang(t.a) },
                died: if t.alive { 0 } else { ms(self.time - t.dead_age) },
                weapon: t.weapon.map_or(255, |w| w as u8),
                shield: if t.shield > 0.0 { (gdeci + t.shield * 10.0).round().max(1.0) as u32 } else { 0 },
                ammo: ammo.clamp(0, 255) as u8,
                reload: (reload * 10.0).round().clamp(0.0, 255.0) as u8,
                eff_mask: mask,
                eff_exp: exp,
                charge: (t.charge * 100.0).round().clamp(0.0, 255.0) as u8,
                taunt: if t.taunt_time > 0.0 { t.taunt } else { 0 },
            });
        }
        let mut shots = Vec::with_capacity(self.shots.len());
        for s in &self.shots {
            if let Some((fx, fy)) = fog_center {
                if hypot(s.x - fx, s.y - fy) > FOG_SEND {
                    continue;
                }
            }
            let ang = match s.kind {
                k if k.angled() => s.heading(),
                Kind::Spider => s.heading(),
                Kind::Sentry
                | Kind::Orbit
                | Kind::Fence
                | Kind::Sprinkler
                | Kind::Hammer
                | Kind::Shadow
                | Kind::Afterburn
                | Kind::Thorn
                | Kind::Pendel => s.ang,
                Kind::Pincer => s.heading(),
                _ => 0.0,
            };
            shots.push(NetShot {
                id: s.id,
                kind: s.kind as u8,
                owner: s.owner as u8,
                x: qpos(s.x),
                y: qpos(s.y),
                ang: qang(ang),
                born: ms(self.gtime - s.age),
                life: if s.life > 1e6 { u32::MAX } else { ms(s.life) },
            });
        }
        shots.sort_by_key(|s| s.id);
        let mut fx: Vec<NetFx> = self
            .fx
            .iter()
            .map(|f| NetFx {
                id: f.id,
                born: ms(self.gtime - f.age),
                life: ms(f.life).min(u16::MAX as u32) as u16,
                data: f.data.clone(),
            })
            .collect();
        fx.sort_by_key(|f| f.id);
        let mut bots = 0u8;
        for &b in &self.bots {
            bots |= 1 << b;
        }
        let mut scores = [0u16; MAX_PLAYERS];
        for (i, s) in self.scores.iter().enumerate() {
            scores[i] = (*s).min(u16::MAX as u32) as u16;
        }
        NetSnap {
            time: ms(self.time),
            gtime: ms(self.gtime),
            ack: 0,
            maze_id: self.maze_id,
            maze_rev: self.maze.rev,
            ts: (self.ts * 255.0).round().clamp(0.0, 255.0) as u8,
            et: self.end_timer.map_or(u16::MAX, |e| ms(e.max(0.0)).min(65534) as u16),
            win: self
                .win
                .as_ref()
                .map(|w| NetWin { slot: w.slot.map_or(255, |s| s as u8), streak: w.streak.min(255) as u8 }),
            scores,
            bots,
            queued: false,
            tanks,
            shots,
            boxes: self
                .boxes
                .iter()
                .map(|&(x, y, item)| NetBox {
                    x: x.round().clamp(0.0, 65535.0) as u16,
                    y: y.round().clamp(0.0, 65535.0) as u16,
                    item: item as u8,
                })
                .collect(),
            fx,
            feed: self
                .feed
                .iter()
                .map(|f| NetFeed {
                    by: f.by.map_or(255, |b| b as u8),
                    victim: f.victim as u8,
                    how: f.how as u8,
                    variant: f.variant,
                    born: ms(self.time - f.age),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{read_snap, write_snap, Reader, Writer};
    use crate::game::Shot;
    use crate::rng::Rng;

    #[test]
    fn directional_weapon_visuals_survive_full_and_delta_snapshots() {
        let kinds = [Kind::Hammer, Kind::Shadow, Kind::Afterburn, Kind::Thorn, Kind::Pendel, Kind::Pincer];
        let mut game = Game::with_rng(&[0, 1], Rng::new(1));
        let mut previous = None;
        for angle in [0.0, TAU / 4.0, TAU / 2.0, TAU - 0.1] {
            game.shots.clear();
            for (id, kind) in kinds.into_iter().enumerate() {
                let mut shot = Shot::new(id as u32 + 1, kind, 0, 200.0, 200.0, angle, 1.0, 4.0, 2.0, 0.0);
                shot.ang = angle;
                game.shots.push(shot);
            }
            let snapshot = game.net_snap(None, false);
            for base in [None, previous.as_ref()] {
                let mut writer = Writer::new();
                write_snap(&mut writer, &snapshot, base);
                let decoded = read_snap(&mut Reader::new(&writer.buf), base).expect("snapshot");
                for shot in decoded.view().shots {
                    let error = crate::geom::angle_diff(shot.ang, angle).abs();
                    assert!(error < TAU / 65536.0, "{:?}: expected {angle}, got {}", shot.kind, shot.ang);
                }
            }
            previous = Some(snapshot);
        }
    }
}
