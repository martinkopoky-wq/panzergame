//! Computergegner. Liefert jeden Tick (Tasten, Schuss-Zähler) wie ein Spieler an der
//! Tastatur. Läuft nur beim Server bzw. lokal, die Clients sehen nur einen weiteren Panzer.

use std::collections::HashMap;
use std::f64::consts::TAU;

use crate::arena::{Arena, Cell};
use crate::consts::*;
use crate::game::{Game, Tank};
use crate::geom::{angle_diff, hypot, seg_dist};
use crate::items::{Eff, Item, Kind};
use crate::physics::{fence_ends, first_wall, rail_path, strike_target, tank_radius};
use crate::rng::Rng;

/// so oft planen Bots neu (Ziel, Weg, Zielwinkel)
const BOT_PLAN: f64 = 0.3;
/// so viele Winkel probiert ein Bot für einen Bandenschuss
const BOT_RAYS: usize = 18;
/// so lange darf eine Waffe ungenutzt bleiben, dann wird sie auf gut Glück eingesetzt
const HOARD_TIME: f64 = 9.0;
/// "Hehe!", "Zu langsam!", "Komm her!"
const BOT_TAUNTS: [u8; 3] = [1, 4, 5];

/// Waffen, mit denen ein Bot auch über die Bande zielt
fn bank_shot(w: Option<Item>) -> bool {
    use Item::*;
    match w {
        None => true,
        Some(w) => {
            matches!(
                w,
                Frag | Laser | Bouncy | Swap | Rang | Ice | Hole | Saw | Cannon | Gold | Sticky | Fw | Seek | Split
            )
        }
    }
}

/// legen Bots ab und zu hinter sich
fn is_drop(w: Item) -> bool {
    matches!(w, Item::Mine | Item::Trap | Item::Oil | Item::Spider | Item::Wall | Item::Fence)
}

/// Waffe einsetzen, sobald ein Gegner so nah ist
fn bot_near(w: Item) -> Option<f64> {
    Some(match w {
        Item::Trap | Item::Oil | Item::Spider => 220.0,
        Item::Sentry => 420.0,
        Item::Frost => FROST_R,
        Item::Ram | Item::Nova => 260.0,
        Item::Timebomb => 170.0,
        Item::Sprinkler => 300.0,
        Item::Destruct => DESTRUCT_BLAST,
        _ => return None,
    })
}

/// Waffe einsetzen, wenn ein Gegner in Blickrichtung liegt: (ab, bis, Winkel), Wände egal
fn bot_ahead(w: Item) -> Option<(f64, f64, f64)> {
    Some(match w {
        Item::Phase => (120.0, 700.0, 0.05),
        Item::Gl => (140.0, GL_DIST + 30.0, 0.2),
        Item::Arty => (90.0, 300.0, 0.3),
        Item::Orbit => (ORBIT_DIST - 90.0, ORBIT_DIST + 90.0, 0.35),
        _ => return None,
    })
}

/// Geschoss, das in den nächsten 0,6 s nah vorbeikommt
#[derive(Clone, Copy)]
struct Danger {
    when: f64,
    mx: f64,
    my: f64,
    vx: f64,
    vy: f64,
}

pub struct Bot {
    pub slot: usize,
    pub skill: f64,
    pub fires: u16,
    round: u32,
    tick: u64,
    plan_in: f64,
    path: Vec<Cell>,
    goal: Option<(f64, f64)>,
    /// Slot des Ziels. Kein Index in game.tanks: Der verschiebt sich, sobald jemand geht.
    target: Option<usize>,
    aim: Option<f64>,
    /// so lange passt der Schuss schon (Reaktionszeit)
    lock: f64,
    /// Ergebnis der letzten Trefferprüfung
    ok: bool,
    /// hat beschlossen, etwas abzulegen: gilt, bis es wirklich passiert ist
    drop_now: bool,
    /// Pause zwischen zwei Schüssen
    cool: f64,
    /// Geschoss-id -> bemerkt? (Bots übersehen manchmal etwas)
    noticed: HashMap<u32, bool>,
    stuck: f64,
    escape: f64,
    last: Option<(f64, f64)>,
    /// so lange liegt die Waffe schon ungenutzt herum
    held: f64,
    turn: u8,
    taunt: u8,
    taunt_t: f64,
    kills: usize,
    gg: bool,
    rng: Rng,
    /// wiederverwendete Arbeitslisten: foes pro Denk-Tick, dist/prev pro Plan (0,3 s)
    foes: Vec<usize>,
    dist: Vec<u32>,
    prev: Vec<Option<usize>>,
}

impl Bot {
    pub fn new(slot: usize, skill: Option<f64>) -> Bot {
        let mut rng = Rng::from_entropy();
        rng.next_u64();
        let skill = skill.unwrap_or_else(|| rng.uniform(0.55, 0.9));
        let mut b = Bot {
            slot,
            skill,
            fires: 0,
            round: u32::MAX,
            tick: slot as u64, // versetzt, damit nicht alle Bots im selben Tick rechnen
            plan_in: 0.0,
            path: Vec::new(),
            goal: None,
            target: None,
            aim: None,
            lock: 0.0,
            ok: false,
            drop_now: false,
            cool: 0.0,
            noticed: HashMap::new(),
            stuck: 0.0,
            escape: 0.0,
            last: None,
            held: 0.0,
            turn: LEFT,
            taunt: 0,
            taunt_t: 0.0,
            kills: 0,
            gg: false,
            rng: Rng::new(slot as u64 * 7919 + 17),
            foes: Vec::new(),
            dist: Vec::new(),
            prev: Vec::new(),
        };
        b.rng = Rng::from_entropy();
        for _ in 0..=slot {
            b.rng.next_u64();
        }
        b.reset();
        b
    }

    fn reset(&mut self) {
        self.plan_in = self.rng.uniform(0.0, BOT_PLAN);
        self.path.clear();
        self.goal = None;
        self.target = None;
        self.aim = None;
        self.lock = 0.0;
        self.ok = false;
        self.drop_now = false;
        self.cool = 0.0;
        self.noticed.clear();
        self.stuck = 0.0;
        self.escape = 0.0;
        self.last = None;
        self.held = 0.0;
        self.turn = if self.rng.random() < 0.5 { LEFT } else { RIGHT };
        self.taunt = 0;
        self.taunt_t = 0.0;
        self.kills = 0;
        self.gg = false;
    }

    /// (Tasten, Schuss-Zähler) für diesen Tick
    pub fn think(&mut self, game: &Game) -> (u8, u16) {
        if game.maze_id != self.round {
            self.round = game.maze_id;
            self.reset();
        }
        let Some(ti) = game.tank_index(self.slot) else { return (0, self.fires) };
        let t = &game.tanks[ti];
        if !t.alive {
            return (0, self.fires);
        }
        self.tick += 1;
        let radar = t.has(Eff::Radar); // mit Radar sieht er auch getarnte Panzer
        let mut foes = std::mem::take(&mut self.foes);
        foes.clear();
        foes.extend((0..game.tanks.len()).filter(|&i| {
            let o = &game.tanks[i];
            o.alive && i != ti && (radar || !o.has(Eff::Invis))
        }));
        self.plan_in -= TICK;
        if self.plan_in <= 0.0 {
            self.plan_in = BOT_PLAN;
            self.plan(game, ti, &foes);
        }
        if let Some(rc) = game.shots.iter().find(|s| s.owner == self.slot && s.kind == Kind::Rc) {
            if t.has(Eff::Steer) {
                // Fernlenkrakete zum Gegner steuern
                let keys = self.steer_rc(game, rc, &foes);
                self.foes = foes;
                return (keys, self.fires);
            }
        }
        let mut keys = self.drive(game, ti);
        let (press, hold) = self.fire(game, ti, &foes);
        self.foes = foes;
        if hold {
            keys |= SHOOT;
        }
        if press {
            self.fires = self.fires.wrapping_add(1);
        }
        (keys | (self.taunt_bits(game) << TAUNT_SHIFT), self.fires)
    }

    // --- Planen ---

    fn plan(&mut self, game: &Game, ti: usize, foes: &[usize]) {
        let arena = &game.arena;
        let t = &game.tanks[ti];
        // Puffer ausleihen (plan hält self sonst gemischt belegt) und am Ende zurückgeben
        let mut dist = std::mem::take(&mut self.dist);
        let mut prev = std::mem::take(&mut self.prev);
        arena.bfs_into(arena.cell(t.x, t.y), &mut dist, &mut prev);
        let steps = |x: f64, y: f64| -> u32 {
            let d = dist[arena.cell_index(arena.cell(x, y))];
            if d == u32::MAX {
                99
            } else {
                d
            }
        };
        let nearest = foes.iter().copied().min_by(|&a, &b| {
            let (oa, ob) = (&game.tanks[a], &game.tanks[b]);
            (steps(oa.x, oa.y), hypot(oa.x - t.x, oa.y - t.y))
                .partial_cmp(&(steps(ob.x, ob.y), hypot(ob.x - t.x, ob.y - t.y)))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.target = nearest.map(|i| game.tanks[i].slot);
        let target = nearest.map(|i| &game.tanks[i]);
        let far = target.map_or(99, |o| steps(o.x, o.y));
        let threat = self.threat(game, t);
        let goal = if let Some(th) = threat {
            Some(self.flee(arena, &dist, th))
        } else if t.has(Eff::Potato) && target.is_some() {
            // Kartoffel loswerden: hin und anstupsen
            target.map(|o| (o.x, o.y))
        } else {
            let bx = self.find_box(game, t, &dist);
            match (bx, target) {
                (Some((d, p)), _) if d <= far + 1 => Some(p),
                (_, Some(o)) => {
                    if far > 1 || !arena.clear(t.x, t.y, o.x, o.y) {
                        Some((o.x, o.y))
                    } else {
                        None
                    }
                }
                _ => {
                    if self.goal.is_none() || self.rng.random() < 0.08 {
                        // niemand zu sehen: irgendwo hin
                        let reach: Vec<usize> = (0..dist.len()).filter(|&i| dist[i] != u32::MAX).collect();
                        if reach.is_empty() {
                            None
                        } else {
                            let i = self.rng.choice(&reach);
                            Some(arena.center((i / arena.cols, i % arena.cols)))
                        }
                    } else {
                        self.goal
                    }
                }
            }
        };
        self.goal = goal;
        self.path.clear();
        if let Some(g) = goal {
            let mut cell = Some(arena.cell_index(arena.cell(g.0, g.1)));
            while let Some(ci) = cell {
                if dist[ci] == u32::MAX {
                    break;
                }
                self.path.push((ci / arena.cols, ci % arena.cols));
                cell = prev[ci];
            }
            self.path.reverse();
        }
        self.dist = dist;
        self.prev = prev;
        self.aim = None;
        if let Some(ti2) = nearest {
            if threat.is_none() {
                self.aim = self.find_aim(game, ti, ti2);
            }
        }
    }

    /// Tasten, mit denen die Fernlenkrakete auf dem Weg durchs Labyrinth zum nächsten
    /// Gegner bleibt.
    fn steer_rc(&self, game: &Game, rc: &crate::game::Shot, foes: &[usize]) -> u8 {
        let Some(&o) = foes.iter().min_by(|&&a, &&b| {
            let (ta, tb) = (&game.tanks[a], &game.tanks[b]);
            hypot(ta.x - rc.x, ta.y - rc.y).total_cmp(&hypot(tb.x - rc.x, tb.y - rc.y))
        }) else {
            return 0;
        };
        let o = &game.tanks[o];
        let arena = &game.arena;
        let path = arena.path(arena.cell(rc.x, rc.y), arena.cell(o.x, o.y));
        let (tx, ty) = if path.len() <= 2 { (o.x, o.y) } else { arena.center(path[1]) };
        let diff = angle_diff((ty - rc.y).atan2(tx - rc.x), rc.heading());
        if diff.abs() < 0.06 {
            0
        } else if diff > 0.0 {
            RIGHT
        } else {
            LEFT
        }
    }

    /// Gefahr in der Nähe (x, y, Radius), vor der man besser wegfährt.
    fn threat(&self, game: &Game, t: &Tank) -> Option<(f64, f64, f64)> {
        for s in &game.shots {
            let r = match s.kind {
                Kind::Nuke => NUKE_BLAST + 50.0,
                Kind::Strike => STRIKE_BLAST + 30.0,
                Kind::Arty | Kind::Meteor => s.kind.blast().unwrap_or(40.0) + 30.0,
                Kind::Orbit if s.age > ORBIT_DELAY - 0.8 => ORBIT_R + 70.0,
                Kind::Oilf => OIL_R + 25.0,
                Kind::Burn => NAPALM_R + 25.0,
                Kind::Timebomb => TIMEBOMB_BLAST + 30.0,
                Kind::Destruct if s.owner != self.slot => DESTRUCT_BLAST + 30.0,
                Kind::Fence => {
                    // die Schranke ist ein Strich: Gefahr ist die Stelle, die dem Bot am nächsten liegt
                    let ((x0, y0), (x1, y1)) = fence_ends(&game.arena, s.x, s.y, s.ang);
                    let (dx, dy) = (x1 - x0, y1 - y0);
                    let k = (((t.x - x0) * dx + (t.y - y0) * dy) / (dx * dx + dy * dy).max(1.0)).clamp(0.0, 1.0);
                    let (px, py) = (x0 + dx * k, y0 + dy * k);
                    if hypot(px - t.x, py - t.y) < tank_radius(&t.eff) + 30.0 {
                        return Some((px, py, 70.0));
                    }
                    continue;
                }
                Kind::Hole => HOLE_RANGE * 0.9,
                Kind::Sticky if s.stuck.is_some() && s.stuck != Some(self.slot as i8) => STICKY_BLAST + 30.0,
                Kind::Frag | Kind::Spider if s.owner != self.slot => 100.0,
                _ => continue,
            };
            if hypot(s.x - t.x, s.y - t.y) < r {
                return Some((s.x, s.y, r));
            }
        }
        if !t.has(Eff::Potato) {
            for o in &game.tanks {
                if o.alive && o.has(Eff::Potato) && hypot(o.x - t.x, o.y - t.y) < 170.0 {
                    return Some((o.x, o.y, 170.0));
                }
            }
        }
        None
    }

    /// Zelle in der Nähe, die möglichst weit von der Gefahr weg liegt.
    fn flee(&self, arena: &Arena, dist: &[u32], threat: (f64, f64, f64)) -> (f64, f64) {
        let (x, y, _) = threat;
        let mut best: Option<(f64, Cell)> = None;
        for (i, &d) in dist.iter().enumerate() {
            if d > 6 {
                continue;
            }
            let c = (i / arena.cols, i % arena.cols);
            let (cx, cy) = arena.center(c);
            let score = hypot(cx - x, cy - y) - d as f64 * 10.0;
            if best.is_none_or(|b| score > b.0) {
                best = Some((score, c));
            }
        }
        best.map_or((x, y), |b| arena.center(b.1))
    }

    /// Nächste Kiste, die der Bot nehmen kann: (Schritte, (x, y))
    fn find_box(&self, game: &Game, t: &Tank, dist: &[u32]) -> Option<(u32, (f64, f64))> {
        let mut best: Option<(u32, (f64, f64))> = None;
        for &(x, y, kind) in &game.boxes {
            if t.weapon.is_some() && kind.is_box_weapon() {
                continue; // mit Waffe bleiben Waffenkisten liegen
            }
            let d = dist[game.arena.cell_index(game.arena.cell(x, y))];
            if d != u32::MAX && d <= 6 && best.is_none_or(|b| d < b.0) {
                best = Some((d, (x, y)));
            }
        }
        best
    }

    /// Winkel, mit dem ein Schuss target trifft: direkt oder über die Bande.
    fn find_aim(&mut self, game: &Game, ti: usize, target: usize) -> Option<f64> {
        let arena = &game.arena;
        let (t, o) = (&game.tanks[ti], &game.tanks[target]);
        if arena.clear(t.x, t.y, o.x, o.y) {
            return Some((o.y - t.y).atan2(o.x - t.x));
        }
        if hypot(o.x - t.x, o.y - t.y) > 460.0 || !bank_shot(t.weapon) {
            return None;
        }
        let circles = [
            (o.x, o.y, tank_radius(&o.eff)),
            (t.x, t.y, tank_radius(&t.eff) + 2.0), // nicht sich selbst treffen
        ];
        let mut best: Option<(f64, f64)> = None;
        let base = self.rng.uniform(0.0, TAU);
        for k in 0..BOT_RAYS {
            let a = base + k as f64 * TAU / BOT_RAYS as f64;
            let (pts, hit) = arena.raycast(t.x, t.y, a, 560.0, &circles);
            if hit == Some(0) && pts.len() <= 4 {
                let d = angle_diff(a, t.a).abs();
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, a));
                }
            }
        }
        best.map(|b| b.1)
    }

    // --- Fahren ---

    fn drive(&mut self, game: &Game, ti: usize) -> u8 {
        let t = &game.tanks[ti];
        if t.has(Eff::Frozen) {
            return 0;
        }
        if let Some(danger) = self.danger(game, t) {
            return dodge(t, danger);
        }
        if self.escape > 0.0 {
            // festgefahren: kurz zurücksetzen und drehen
            self.escape -= TICK;
            return DOWN | self.turn;
        }
        let mut keys = 0;
        let ready = t.weapon.is_some() || game.reload_state(ti).0 > 0;
        match self.aim {
            Some(aim) if ready => {
                let diff = angle_diff(aim, t.a);
                if diff.abs() > 0.05 {
                    keys |= if diff > 0.0 { RIGHT } else { LEFT };
                }
                let close = self.target.and_then(|s| game.tank(s)).map(|o| hypot(o.x - t.x, o.y - t.y));
                if diff.abs() < 0.5 && self.goal.is_some() && close.is_some_and(|c| c > 150.0) {
                    keys |= UP;
                }
            }
            _ => {
                if let Some(wp) = self.waypoint(&game.arena, t) {
                    let diff = angle_diff((wp.1 - t.y).atan2(wp.0 - t.x), t.a);
                    if diff.abs() > 0.08 {
                        keys |= if diff > 0.0 { RIGHT } else { LEFT };
                    }
                    if diff.abs() < 0.8 {
                        keys |= UP;
                    }
                }
            }
        }
        if keys & UP != 0 {
            // steckt er fest?
            if self.last.is_some_and(|l| hypot(t.x - l.0, t.y - l.1) < 0.6) {
                self.stuck += TICK;
                if self.stuck > 0.35 {
                    self.stuck = 0.0;
                    self.escape = 0.35;
                    self.turn = if self.rng.random() < 0.5 { LEFT } else { RIGHT };
                }
            } else {
                self.stuck = 0.0;
            }
        }
        self.last = Some((t.x, t.y));
        keys
    }

    /// Nächster Punkt auf dem Weg: Mitte der nächsten Zelle bzw. das Ziel selbst.
    fn waypoint(&self, arena: &Arena, t: &Tank) -> Option<(f64, f64)> {
        if self.path.is_empty() {
            return self.goal;
        }
        let cur = arena.cell(t.x, t.y);
        let Some(i) = self.path.iter().position(|&c| c == cur) else {
            return Some(arena.center(self.path[0]));
        };
        if i + 1 >= self.path.len() {
            return self.goal;
        }
        Some(arena.center(self.path[i + 1]))
    }

    fn danger(&mut self, game: &Game, t: &Tank) -> Option<Danger> {
        let r = tank_radius(&t.eff);
        let mut worst: Option<Danger> = None;
        for s in &game.shots {
            let k = s.kind;
            if k.is_static() || k == Kind::Swap {
                continue;
            }
            if s.owner == self.slot && (s.age < s.grace || k == Kind::Rang) {
                continue;
            }
            let vv = s.vx * s.vx + s.vy * s.vy;
            if vv < 100.0 {
                continue;
            }
            let (rx, ry) = (t.x - s.x, t.y - s.y);
            let when = (rx * s.vx + ry * s.vy) / vv;
            if !(0.0 < when && when < 0.6) {
                continue;
            }
            let (mx, my) = (rx - s.vx * when, ry - s.vy * when); // vom Vorbeiflug-Punkt zum Bot
            if mx * mx + my * my > (r + s.r + 8.0).powi(2) {
                continue;
            }
            let skill = self.skill;
            let rng = &mut self.rng;
            let seen = *self.noticed.entry(s.id).or_insert_with(|| rng.random() < skill);
            if seen && worst.is_none_or(|w| when < w.when) {
                worst = Some(Danger { when, mx, my, vx: s.vx, vy: s.vy });
            }
        }
        worst
    }

    // --- Schießen ---

    /// (einmal drücken, Feuer halten)
    fn fire(&mut self, game: &Game, ti: usize, foes: &[usize]) -> (bool, bool) {
        self.cool = (self.cool - TICK).max(0.0);
        let t = &game.tanks[ti];
        if t.has(Eff::Frozen) || game.in_wall(ti) {
            return (false, false);
        }
        let own: Vec<&crate::game::Shot> = game.shots.iter().filter(|s| s.owner == self.slot).collect();
        if let Some(frag) = own.iter().find(|s| s.kind == Kind::Frag) {
            // Splitterbombe zünden: Gegner nah dran, man selbst weit genug weg
            let near =
                foes.iter().map(|&o| hypot(game.tanks[o].x - frag.x, game.tanks[o].y - frag.y)).fold(999.0, f64::min);
            return (near < 75.0 && hypot(t.x - frag.x, t.y - frag.y) > 120.0, false);
        }
        if foes.is_empty() || own.iter().any(|s| s.kind == Kind::Swap) {
            return (false, false); // ein zweiter Druck würde zur Tauschkugel springen
        }
        let w = t.weapon;
        self.held = if w.is_some() { self.held + TICK } else { 0.0 };
        match w {
            Some(Item::Mg) | Some(Item::Flame) => {
                // der Flammenwerfer reicht nicht weit: lieber verschwenden als ewig horten
                let reach = if w == Some(Item::Flame) && self.held < HOARD_TIME { 130.0 } else { 420.0 };
                return (false, facing(game, t, foes, reach, 0.22));
            }
            Some(Item::Rail) => {
                // laden, sobald jemand ungefähr in der Linie steht, dann loslassen
                if self.tick % 3 == 0 {
                    self.ok = rail_lined(game, t, foes);
                }
                if t.charge >= RAIL_CHARGE {
                    return (false, !self.ok);
                }
                return (false, self.ok || t.charge > 0.0 || self.held > 9.0);
            }
            _ => {}
        }
        if self.cool > 0.0 {
            return (false, false);
        }
        if self.tick % 3 == 0 {
            // die teuren Prüfungen nur jeden 3. Tick
            self.ok = self.should_fire(game, ti, foes, w);
        }
        self.lock = if self.ok { self.lock + TICK } else { 0.0 };
        if self.lock < 0.22 - 0.16 * self.skill {
            return (false, false); // Reaktionszeit
        }
        self.lock = 0.0;
        self.cool = 0.2;
        self.ok = false;
        self.drop_now = false;
        (true, false)
    }

    /// Zufällig etwas ablegen. Die Entscheidung bleibt bestehen: Eine Prüfung gilt nur
    /// 3 Ticks, die Reaktionszeit dauert länger, sonst käme es nie dazu.
    fn drop_roll(&mut self, chance: f64) -> bool {
        self.drop_now = self.drop_now || self.rng.random() < chance;
        self.drop_now
    }

    fn should_fire(&mut self, game: &Game, ti: usize, foes: &[usize], w: Option<Item>) -> bool {
        let arena = &game.arena;
        let t = &game.tanks[ti];
        let Some(w) = w else {
            return game.reload_state(ti).0 > 0 && lined_up(game, ti, 620.0);
        };
        // Waffe nicht ewig horten: Wer eine trägt, kann keine andere aufheben. Bots halten
        // im Duell rund 150 px Abstand, Nahkampfwaffen würden sonst nie ausgelöst.
        if self.held > HOARD_TIME && facing(game, t, foes, 600.0, 0.6) {
            return true;
        }
        match w {
            Item::Laser => {
                let alive: Vec<usize> = (0..game.tanks.len()).filter(|&i| game.tanks[i].alive).collect();
                let circles: Vec<_> =
                    alive.iter().map(|&i| (game.tanks[i].x, game.tanks[i].y, game.tanks[i].radius())).collect();
                let hit = arena.raycast(t.x, t.y, t.a, LASER_LEN, &circles).1;
                hit.is_some_and(|h| foes.contains(&alive[h]))
            }
            Item::Rocket | Item::Rc | Item::Swarm => {
                self.target.is_some() && first_wall(arena, t.x, t.y, t.a, 90.0) > 70.0
            }
            Item::Zap => facing(game, t, foes, ZAP_RANGE - 20.0, ZAP_CONE * 0.8),
            Item::Nuke => foes.iter().any(|&o| hypot(game.tanks[o].x - t.x, game.tanks[o].y - t.y) < 320.0),
            Item::Strike => {
                let (x, y) = strike_target(arena, t.x, t.y, t.a);
                hypot(x - t.x, y - t.y) > STRIKE_BLAST + 25.0
                    && foes.iter().any(|&o| hypot(game.tanks[o].x - x, game.tanks[o].y - y) < STRIKE_BLAST * 0.8)
            }
            Item::Mine => self.drop_roll(0.03),
            Item::Hole => facing(game, t, foes, 330.0, 0.3) && !facing(game, t, foes, 120.0, std::f64::consts::PI),
            Item::Shotgun => lined_up(game, ti, 240.0),
            // nicht in die eigene Feuerfläche schießen
            Item::Napalm => lined_up(game, ti, 420.0) && !facing(game, t, foes, 110.0, std::f64::consts::PI),
            _ if bot_near(w).is_some() => {
                let near = bot_near(w).unwrap_or(0.0);
                foes.iter().any(|&o| hypot(game.tanks[o].x - t.x, game.tanks[o].y - t.y) < near)
            }
            _ if bot_ahead(w).is_some() => {
                let (n, f, c) = bot_ahead(w).unwrap_or((0.0, 0.0, 0.0));
                aligned(game, t, foes, n, f, c)
            }
            _ if is_drop(w) => {
                self.drop_roll(0.01) || (w == Item::Wall && self.danger(game, t).is_some())
                // Deckung
            }
            Item::Push => {
                if self.danger(game, t).is_some_and(|d| d.when < 0.35) {
                    return true; // anfliegendes abwehren
                }
                facing(game, t, foes, 140.0, PUSH_CONE)
            }
            Item::Jump => self.threat(game, t).is_some() || self.escape > 0.0,
            Item::Seek => facing(game, t, foes, 600.0, 0.9),
            Item::Prism => lined_up(game, ti, 600.0) || facing(game, t, foes, 450.0, 0.4),
            Item::Fw => lined_up(game, ti, 300.0),
            _ => {
                lined_up(game, ti, 560.0) // Splitterbombe, Flummi, Sägeblatt ...
            }
        }
    }

    // --- Sprüche ---

    /// Ab und zu ein Spruch: nach einem Abschuss oder nach dem Rundensieg.
    fn taunt_bits(&mut self, game: &Game) -> u8 {
        let kills = game.kills[self.slot].len();
        if kills > self.kills && self.rng.random() < 0.5 {
            self.taunt = self.rng.choice(&BOT_TAUNTS);
            self.taunt_t = 0.3;
        }
        self.kills = kills;
        if game.win.as_ref().is_some_and(|w| w.slot == Some(self.slot)) && !self.gg {
            self.gg = true;
            self.taunt = 3; // "GG"
            self.taunt_t = 0.3;
        }
        self.taunt_t -= TICK;
        if self.taunt_t > 0.0 {
            self.taunt
        } else {
            0
        }
    }
}

/// Quer zur Flugbahn ausweichen (vorwärts oder rückwärts, je nachdem was näher liegt).
fn dodge(t: &Tank, d: Danger) -> u8 {
    let (mut mx, mut my) = (d.mx, d.my);
    let mut dd = hypot(mx, my);
    if dd < 1.0 {
        // kommt genau drauf zu: irgendeine Seite
        mx = -d.vy;
        my = d.vx;
        dd = hypot(d.vx, d.vy);
    }
    let (ex, ey) = (mx / dd, my / dd);
    let fwd = t.a.cos() * ex + t.a.sin() * ey;
    let mut keys = if fwd > 0.0 { UP } else { DOWN };
    if fwd.abs() < 0.7 {
        // steht quer -> beim Fahren eindrehen
        let side = t.a.cos() * ey - t.a.sin() * ex;
        keys |= if (side > 0.0) == (fwd > 0.0) { RIGHT } else { LEFT };
    }
    keys
}

/// Gegner in Blickrichtung zwischen near und far, Wände egal (fliegt drüber/durch).
fn aligned(game: &Game, t: &Tank, foes: &[usize], near: f64, far: f64, cone: f64) -> bool {
    foes.iter().any(|&o| {
        let o = &game.tanks[o];
        let d = hypot(o.x - t.x, o.y - t.y);
        let a = (o.y - t.y).atan2(o.x - t.x);
        near < d && d < far && angle_diff(a, t.a).abs() < cone
    })
}

fn rail_lined(game: &Game, t: &Tank, foes: &[usize]) -> bool {
    let ((ex, ey), _) = rail_path(&game.arena, t.x, t.y, t.a);
    foes.iter().any(|&o| {
        let o = &game.tanks[o];
        seg_dist(o.x, o.y, t.x, t.y, ex, ey) < tank_radius(&o.eff)
    })
}

/// Trifft ein Schuss in Blickrichtung (auch über die Bande) einen Gegner und nicht
/// zuerst einen selbst? Getarnte Panzer zählen nicht, die sieht der Bot ja nicht.
fn lined_up(game: &Game, ti: usize, length: f64) -> bool {
    let tanks: Vec<usize> =
        (0..game.tanks.len()).filter(|&i| i == ti || (game.tanks[i].alive && !game.tanks[i].has(Eff::Invis))).collect();
    let circles: Vec<_> =
        tanks.iter().map(|&i| (game.tanks[i].x, game.tanks[i].y, game.tanks[i].radius() + 1.0)).collect();
    let t = &game.tanks[ti];
    let hit = game.arena.raycast(t.x, t.y, t.a, length, &circles).1;
    hit.is_some_and(|h| tanks[h] != ti)
}

/// Steht ein Gegner in Reichweite ungefähr vor dem Rohr (mit freier Sicht)?
fn facing(game: &Game, t: &Tank, foes: &[usize], reach: f64, cone: f64) -> bool {
    foes.iter().any(|&o| {
        let o = &game.tanks[o];
        let d = hypot(o.x - t.x, o.y - t.y);
        d < reach && angle_diff((o.y - t.y).atan2(o.x - t.x), t.a).abs() < cone && game.arena.clear(t.x, t.y, o.x, o.y)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bot in Slot 1 hat gerade geplant: Slot 2 steht genau vor ihm und ist sein Ziel.
    /// Der nächste think() plant nicht neu, das Ziel stammt also noch von vor dem Gehen.
    fn bot_mit_ziel(players: &[usize]) -> (Game, Bot) {
        let mut g = Game::with_rng(players, Rng::new(7));
        g.tanks[1].a = 0.0;
        g.tanks[2].x = g.tanks[1].x + 1.0;
        g.tanks[2].y = g.tanks[1].y;
        let mut bot = Bot::new(1, None);
        bot.round = g.maze_id; // sonst würfelt think() den Planungszeitpunkt neu
        bot.plan_in = 0.0;
        bot.think(&g);
        assert_eq!(bot.target, Some(2));
        assert!(bot.aim.is_some());
        (g, bot)
    }

    /// So ist der Server abgestürzt: Das Ziel war der letzte Panzer im Vec, dann ging ein
    /// anderer Spieler und der Vec wurde kürzer.
    #[test]
    fn anderer_spieler_geht_und_ziel_war_der_letzte_panzer() {
        let (mut g, mut bot) = bot_mit_ziel(&[0, 1, 2]);
        g.remove_player(0);
        bot.think(&g);
        assert_eq!(bot.target, Some(2));
    }

    #[test]
    fn ziel_selbst_geht() {
        let (mut g, mut bot) = bot_mit_ziel(&[0, 1, 2]);
        g.remove_player(2);
        bot.think(&g);
    }

    /// Ist das Ziel nicht der letzte Panzer, stürzt nichts ab, ein gemerkter Index zeigt aber
    /// still auf den falschen Panzer.
    #[test]
    fn ziel_bleibt_derselbe_panzer_wenn_jemand_geht() {
        let (mut g, mut bot) = bot_mit_ziel(&[0, 1, 2, 3]);
        let (x, y) = (g.tanks[1].x, g.tanks[1].y);
        g.tanks[3].x = x + 500.0; // rückt durchs Gehen auf den alten Index von Slot 2
        bot.goal = Some((x, y));
        g.remove_player(0);
        let (keys, _) = bot.think(&g);
        assert_eq!(keys & UP, 0, "das Ziel steht direkt davor, der Bot darf nicht vorfahren");
    }
}
