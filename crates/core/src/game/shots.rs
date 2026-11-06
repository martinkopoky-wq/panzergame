//! Ein Simulationsschritt: Effekte, Zusammenstöße, Kisten, alle Geschossarten, Rundenende.

use super::{Game, Win};
use crate::consts::*;
use crate::geom::seg_dist;
use crate::geom::{angle_diff, circle_hits_walls, hypot};
use crate::items::{Eff, Item, Kind};
use crate::physics::{ahead, clamp_arena, fence_ends, first_wall};
use crate::snap::{qp, FxData};

/// Rangfolge beim Zusammenstoß: Superstern > Rammbock > Riesenpilz > normal.
fn crusher(t: &super::Tank) -> (u8, Kind) {
    if t.has(Eff::Star) {
        (3, Kind::Star)
    } else if t.has(Eff::Ram) {
        (2, Kind::Ram)
    } else if t.has(Eff::Giant) {
        (1, Kind::Giant)
    } else {
        (0, Kind::B)
    }
}

impl Game {
    pub fn step(&mut self, dt: f64) {
        self.time += dt;
        if self.slow > 0.0 {
            self.slow = (self.slow - dt).max(0.0);
        }
        self.ts = self.time_scale();
        let gdt = dt * self.ts; // Spielzeit: in der Zeitlupe langsamer, Panzer fahren normal
        self.gtime += gdt;
        for f in self.feed.iter_mut() {
            f.age += dt;
        }
        self.feed.retain(|f| f.age < FEED_TIME);

        for ti in 0..self.tanks.len() {
            if !self.tanks[ti].alive {
                self.tanks[ti].dead_age += dt;
                continue;
            }
            let t = &mut self.tanks[ti];
            t.shield = (t.shield - gdt).max(0.0);
            t.taunt_time = (t.taunt_time - dt).max(0.0);
            t.nopick = (t.nopick - dt).max(0.0);
            for &e in crate::items::ALL_EFFS {
                if !self.tanks[ti].has(e) {
                    continue; // nicht aktiv oder Panzer ist gerade explodiert
                }
                let v = self.tanks[ti].eff.get(e) - gdt;
                if v > 0.0 {
                    self.tanks[ti].eff.set(e, v);
                    continue;
                }
                self.tanks[ti].eff.remove(e);
                match e {
                    Eff::Ghost | Eff::Mini => self.settle_in_place(ti), // aus der Wand raus bzw. wieder groß
                    Eff::Potato => {
                        let (x, y, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].slot);
                        self.blast(x, y, POTATO_BLAST, Some(slot), Kind::Potato, 0.85, true);
                    }
                    _ => {}
                }
            }
        }

        // Panzer gegeneinander
        let n = self.tanks.len();
        for i in 0..n {
            for j in i + 1..n {
                if !(self.tanks[i].alive && self.tanks[j].alive) {
                    continue;
                }
                let r = self.tank_r(i) + self.tank_r(j);
                let (dx, dy) = (self.tanks[j].x - self.tanks[i].x, self.tanks[j].y - self.tanks[i].y);
                let d = hypot(dx, dy);
                if d < r + 3.0 {
                    self.pass_potato(i, j);
                    let (r1, e1) = crusher(&self.tanks[i]);
                    let (r2, e2) = crusher(&self.tanks[j]);
                    if r1 != r2 {
                        // Superstern, Rammbock oder Riese walzt den anderen platt
                        let (big, small, how) = if r1 > r2 { (i, j, e1) } else { (j, i, e2) };
                        let a =
                            (self.tanks[small].y - self.tanks[big].y).atan2(self.tanks[small].x - self.tanks[big].x);
                        let by = self.tanks[big].slot;
                        self.hit(small, Some(by), how, false, Some(a), 0);
                        if !self.tanks[small].alive {
                            continue;
                        }
                    }
                }
                if 1e-6 < d && d < r {
                    let p = (r - d) / 2.0;
                    let (x1, y1) = (self.tanks[i].x - dx / d * p, self.tanks[i].y - dy / d * p);
                    let (x2, y2) = (self.tanks[j].x + dx / d * p, self.tanks[j].y + dy / d * p);
                    let a = self.settle(i, x1, y1);
                    self.tanks[i].x = a.0;
                    self.tanks[i].y = a.1;
                    let b = self.settle(j, x2, y2);
                    self.tanks[j].x = b.0;
                    self.tanks[j].y = b.1;
                }
            }
        }

        // Gebaute Wände zerfallen nach BUILD_TIME
        if !self.temp.is_empty() {
            for w in self.temp.iter_mut() {
                w.left -= gdt;
            }
            if self.temp.iter().any(|w| w.left <= 0.0) {
                self.temp.retain(|w| w.left > 0.0);
                self.rebuild_walls();
                // Wer zwischen gebauter und fester Wand eingeklemmt war, kommt jetzt frei
                for ti in 0..self.tanks.len() {
                    if self.tanks[ti].alive {
                        self.settle_in_place(ti);
                    }
                }
            }
        }

        // Power-up-Kisten
        let rain = self.mutator == Some(Mutator::Kisten);
        self.box_timer -= dt;
        if self.box_timer <= 0.0 {
            self.box_timer = if rain { self.rng.uniform(1.0, 2.0) } else { self.rng.uniform(BOX_EVERY.0, BOX_EVERY.1) };
            self.box_timer *= BOX_MAX as f64 / self.box_max as f64;
            let cap = if rain { self.box_max + 5 } else { self.box_max };
            if self.boxes.len() < cap && self.end_timer.is_none() {
                self.spawn_box();
            }
        }
        let mut bi = 0;
        while bi < self.boxes.len() {
            let (bx, by, kind) = self.boxes[bi];
            let mut taken = false;
            for ti in 0..self.tanks.len() {
                let t = &self.tanks[ti];
                if !t.alive || t.nopick > 0.0 || hypot(t.x - bx, t.y - by) > t.radius() + 12.0 {
                    continue;
                }
                if self.give(ti, kind) {
                    taken = true;
                    break;
                }
            }
            if taken {
                self.boxes.remove(bi);
            } else {
                bi += 1;
            }
        }

        // Geschosse (neue kommen erst im nächsten Schritt dran)
        let count = self.shots.len();
        for si in 0..count {
            if self.shots[si].dead {
                continue;
            }
            self.shots[si].age += gdt;
            if self.shots[si].age > self.shots[si].life {
                self.expire(si);
                continue;
            }
            let k = self.shots[si].kind;
            match k {
                Kind::Mine => {
                    self.mine_check(si);
                    continue;
                }
                Kind::Trap => {
                    self.trap_check(si);
                    continue;
                }
                Kind::Strike | Kind::Arty | Kind::Meteor | Kind::Timebomb | Kind::Ring => continue,
                Kind::Chain | Kind::Hammer | Kind::Shadow | Kind::Twist | Kind::Afterburn => continue,
                Kind::Switch => {
                    self.switch_step(si);
                    continue;
                }
                Kind::Thorn => {
                    self.thorn_check(si);
                    continue;
                }
                Kind::Plate => {
                    self.plate_check(si, gdt);
                    continue;
                }
                Kind::Pincer => {
                    self.pincer_step(si, gdt);
                    continue;
                }
                Kind::Cross => {
                    self.cross_step(si);
                    continue;
                }
                Kind::Pendel => {
                    self.pendel_step(si, gdt);
                    continue;
                }
                Kind::Verdict => {
                    self.verdict_step(si, gdt);
                    continue;
                }
                Kind::Catcher => {
                    self.catcher_step(si);
                    continue;
                }
                Kind::Rust => {
                    self.rust_step(si);
                    continue;
                }
                Kind::Cushion => {
                    self.cushion_step(si);
                    continue;
                }
                Kind::Cracker => continue,
                Kind::Decoy => {
                    self.decoy_check(si);
                    continue;
                }
                Kind::Oil | Kind::Oilf | Kind::Burn => {
                    self.oil(si);
                    continue;
                }
                Kind::Fence => {
                    self.fence(si);
                    continue;
                }
                Kind::Sprinkler => {
                    self.sprinkler(si, gdt);
                    continue;
                }
                Kind::Destruct => {
                    self.stick(si);
                    continue;
                }
                Kind::Hook if self.shots[si].stuck.is_some() => {
                    self.hook_pull(si, gdt);
                    continue;
                }
                Kind::Sentry => {
                    self.sentry(si, gdt);
                    continue;
                }
                Kind::Orbit => {
                    self.orbit(si, gdt);
                    continue;
                }
                Kind::Hole if self.shots[si].age > HOLE_TRAVEL => {
                    self.hole_pull(si, gdt);
                    continue;
                }
                Kind::Sticky if self.shots[si].stuck.is_some() => {
                    self.stick(si);
                    continue;
                }
                Kind::Gl => {
                    // fliegt über die Wände
                    let s = &self.shots[si];
                    let (x, y) = clamp_arena(s.x + s.vx * gdt, s.y + s.vy * gdt, 6.0, &self.arena);
                    self.shots[si].x = x;
                    self.shots[si].y = y;
                    continue;
                }
                _ => {}
            }
            if self.shots[si].cool > 0.0 && matches!(k, Kind::B | Kind::Mg | Kind::Sg | Kind::Bnc) {
                self.shots[si].cool = (self.shots[si].cool - gdt).max(0.0);
                continue;
            }
            if k == Kind::Spider && !self.spider(si, gdt) {
                continue;
            }
            if matches!(k, Kind::Phase | Kind::Plasma) {
                if !self.glide(si, gdt) {
                    continue;
                }
            } else if self.fly(si, gdt) {
                continue;
            }
            if k == Kind::Eater {
                self.eat(si);
            }
            for ti in 0..self.tanks.len() {
                let (s, t) = (&self.shots[si], &self.tanks[ti]);
                // Distanzzünder ist erst nach DIST_ARM Flugstrecke scharf
                if !t.alive || (t.slot == s.owner && s.age < s.grace) || (k == Kind::Dist && s.age * s.speed < DIST_ARM) {
                    continue;
                }
                if hypot(t.x - s.x, t.y - s.y) < t.radius() + s.r - 1.0 {
                    self.contact(si, ti);
                    if self.shots[si].dead || !k.pierce() {
                        break;
                    }
                }
            }
        }
        self.shots.retain(|s| !s.dead);

        for f in self.fx.iter_mut() {
            f.age += gdt;
        }
        self.fx.retain(|f| f.age <= f.life);

        // Rundenende: sobald es entschieden ist, kurz Zeitlupe
        let alive = self.alive_slots();
        if self.end_timer.is_none() && alive.len() <= 1 {
            self.end_timer = Some(ROUND_END_DELAY);
            self.slowmo(SLOWMO_TIME);
        }
        if let Some(et) = self.end_timer {
            let et = et - dt;
            self.end_timer = Some(et);
            if et < ROUND_END_DELAY - 1.0 {
                if let Some(&w) = alive.first() {
                    let n = if self.last_winner == Some(w) { self.streak[w] + 1 } else { 1 };
                    self.win = Some(Win { slot: Some(w), streak: n });
                } else {
                    self.win = Some(Win { slot: None, streak: 0 });
                }
            }
            if et <= 0.0 {
                if alive.len() == 1 {
                    let w = alive[0];
                    self.scores[w] += 1;
                    self.streak[w] = if self.last_winner == Some(w) { self.streak[w] + 1 } else { 1 };
                    self.last_winner = Some(w);
                } else {
                    self.last_winner = None;
                }
                self.new_round();
            }
        }
    }

    /// Lenken und Fliegen mit Abprallen. True, wenn das Geschoss danach nichts mehr
    /// trifft (weg, klebt fest, rollt oder ist noch unterwegs zum Ziel).
    fn fly(&mut self, si: usize, dt: f64) -> bool {
        let k = self.shots[si].kind;
        let age = self.shots[si].age;
        match k {
            Kind::Rocket if age > ROCKET_DELAY => self.rocket_steer(si, dt, ROCKET_TURN),
            Kind::Mrk if age > 0.25 => self.rocket_steer(si, dt, SWARM_TURN),
            Kind::Seek if age > 0.3 => self.seek(si, dt),
            Kind::Rc if self.shots[si].keys != 0 => {
                let s = &mut self.shots[si];
                let turn = (if s.keys & RIGHT != 0 { 1.0 } else { 0.0 }) - (if s.keys & LEFT != 0 { 1.0 } else { 0.0 });
                let a = s.heading() + turn * RC_TURN * dt;
                s.vx = a.cos() * s.speed;
                s.vy = a.sin() * s.speed;
            }
            Kind::Rang => self.rang_steer(si, dt),
            Kind::Wave => {
                // Schlangenlinie: der Kurs pendelt um die Schussrichtung
                let s = &mut self.shots[si];
                let a = s.heading() + WAVE_SWING * (age * WAVE_FREQ).cos() * dt;
                s.vx = a.cos() * s.speed;
                s.vy = a.sin() * s.speed;
            }
            Kind::Fl | Kind::Nuke => {
                // Flammen und die rollende Bombe werden langsamer
                let drag = if k == Kind::Fl { FLAME_DRAG } else { NUKE_DRAG };
                let f = (1.0 - drag * dt).max(0.0);
                let s = &mut self.shots[si];
                s.vx *= f;
                s.vy *= f;
                s.speed *= f;
            }
            _ => {}
        }
        let arena = &self.arena;
        let bounces = self.shots[si].travel(dt, arena);
        self.shots[si].bounces += bounces;
        if bounces > 0 {
            match k {
                Kind::Bnc => {
                    // Flummi wird mit jedem Abpraller schneller
                    let s = &mut self.shots[si];
                    s.speed = (s.speed * BOUNCY_GAIN.powi(bounces as i32)).min(BOUNCY_MAX);
                    let a = s.heading();
                    s.vx = a.cos() * s.speed;
                    s.vy = a.sin() * s.speed;
                }
                Kind::Rc => {
                    self.boom_shot(si, 0.8); // Fernlenkrakete geht an der Wand hoch
                    return true;
                }
                Kind::Sticky => {
                    // klebt an der Wand
                    let s = &mut self.shots[si];
                    s.stuck = Some(-1);
                    s.vx = 0.0;
                    s.vy = 0.0;
                    s.speed = 0.0;
                    s.life = s.age + STICKY_FUSE;
                    return true;
                }
                Kind::Fw => {
                    self.burst(si);
                    return true;
                }
                Kind::Napalm => {
                    // Kurs zeigt nach dem Abprallen von der Wand weg: dorthin rücken,
                    // sonst brennt die halbe Fläche in der Wand
                    let s = &self.shots[si];
                    let (x, y, a) = (s.x, s.y, s.heading());
                    let d = (first_wall(&self.arena, x, y, a, NAPALM_R * 0.6) - 8.0).max(0.0);
                    self.shots[si].x = x + a.cos() * d;
                    self.shots[si].y = y + a.sin() * d;
                    self.napalm(si);
                    return true;
                }
                Kind::Hook => {
                    // hakt sich in die Wand: zieht den Schützen hin
                    let s = &mut self.shots[si];
                    s.stuck = Some(-1);
                    s.vx = 0.0;
                    s.vy = 0.0;
                    s.speed = 0.0;
                    s.life = s.age + HOOK_HOLD;
                    return true;
                }
                Kind::Split => self.split(si),
                _ => {
                    let max = match k {
                        Kind::Saw => SAW_BOUNCES,
                        Kind::Cannon => CANNON_BOUNCES,
                        _ => 99,
                    };
                    if self.shots[si].bounces > max {
                        self.shots[si].dead = true;
                        let (x, y) = qp(self.shots[si].x, self.shots[si].y);
                        self.fx(FxData::Boom { x, y, size: 30 }, 0.3);
                        return true;
                    }
                }
            }
        }
        // unterwegs trifft das Loch niemanden, die Bombe rollt
        matches!(k, Kind::Hole | Kind::Nuke)
    }

    fn turn_toward(&mut self, si: usize, tx: f64, ty: f64, rate: f64, dt: f64) {
        let s = &mut self.shots[si];
        let mut cur = s.heading();
        let diff = angle_diff((ty - s.y).atan2(tx - s.x), cur);
        cur += diff.clamp(-rate * dt, rate * dt);
        s.vx = cur.cos() * s.speed;
        s.vy = cur.sin() * s.speed;
    }

    /// Weg durchs Labyrinth zum Ziel: Mitte der nächsten Zelle bzw. das Ziel selbst
    fn waypoint(&self, x: f64, y: f64, tx: f64, ty: f64) -> (f64, f64) {
        let path = self.arena.path(self.arena.cell(x, y), self.arena.cell(tx, ty));
        if path.len() <= 2 {
            (tx, ty)
        } else {
            self.arena.center(path[1])
        }
    }

    fn rocket_steer(&mut self, si: usize, dt: f64, turn: f64) {
        let s = &self.shots[si];
        // Getarnte Panzer sieht die Rakete nicht
        let mut enemies: Vec<usize> = (0..self.tanks.len())
            .filter(|&i| {
                let t = &self.tanks[i];
                t.alive && t.slot != s.owner && !t.has(Eff::Invis)
            })
            .collect();
        if enemies.is_empty() && s.age > s.grace {
            if let Some(oi) = self.tank_index(s.owner) {
                if self.tanks[oi].alive {
                    enemies.push(oi);
                }
            }
        }
        let Some(&target) = enemies.iter().min_by(|&&a, &&b| {
            let da = hypot(self.tanks[a].x - s.x, self.tanks[a].y - s.y);
            let db = hypot(self.tanks[b].x - s.x, self.tanks[b].y - s.y);
            da.total_cmp(&db)
        }) else {
            return;
        };
        let (tx, ty) = self.waypoint(s.x, s.y, self.tanks[target].x, self.tanks[target].y);
        self.turn_toward(si, tx, ty, turn, dt);
    }

    /// Bumerang: fliegt erst eine Kurve, dann zurück zum Werfer.
    fn rang_steer(&mut self, si: usize, dt: f64) {
        let owner = self.tank_index(self.shots[si].owner).filter(|&i| self.tanks[i].alive);
        match owner {
            Some(oi) if self.shots[si].age >= RANG_BACK => {
                let (ox, oy) = (self.tanks[oi].x, self.tanks[oi].y);
                self.turn_toward(si, ox, oy, RANG_TURN, dt);
            }
            _ => {
                let s = &mut self.shots[si];
                let cur = s.heading() + RANG_CURVE * dt;
                s.vx = cur.cos() * s.speed;
                s.vy = cur.sin() * s.speed;
            }
        }
    }

    /// Aktives Schwarzes Loch: zieht Panzer und Geschosse an, verschluckt alles in der Mitte.
    fn hole_pull(&mut self, si: usize, dt: f64) {
        {
            let s = &mut self.shots[si];
            s.speed = 0.0;
            s.vx = 0.0;
            s.vy = 0.0;
        }
        let (sx, sy, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        for ti in 0..self.tanks.len() {
            if !self.tanks[ti].alive {
                continue;
            }
            let (dx, dy) = (sx - self.tanks[ti].x, sy - self.tanks[ti].y);
            let d = hypot(dx, dy);
            if d < HOLE_KILL {
                self.hit(ti, Some(owner), Kind::Hole, true, Some(dy.atan2(dx)), 0);
            } else if d < HOLE_RANGE {
                let step = (HOLE_PULL * (1.0 - d / HOLE_RANGE) * dt).min(d);
                let (tx, ty) = (self.tanks[ti].x + dx / d * step, self.tanks[ti].y + dy / d * step);
                let (nx, ny) = self.settle(ti, tx, ty);
                self.tanks[ti].x = nx;
                self.tanks[ti].y = ny;
            }
        }
        for o in self.shots.iter_mut() {
            if o.dead || o.kind.is_static() {
                continue;
            }
            let (dx, dy) = (sx - o.x, sy - o.y);
            let d = hypot(dx, dy);
            if d < 10.0 {
                o.dead = true;
            } else if d < HOLE_RANGE {
                let acc = HOLE_SHOT_PULL * (1.0 - d / HOLE_RANGE) * dt;
                let (vx, vy) = (o.vx + dx / d * acc, o.vy + dy / d * acc);
                let mut v = hypot(vx, vy);
                if v == 0.0 {
                    v = 1.0;
                }
                o.vx = vx / v * o.speed;
                o.vy = vy / v * o.speed;
            }
        }
    }

    fn mine_check(&mut self, si: usize) {
        let (sx, sy) = (self.shots[si].x, self.shots[si].y);
        if self.shots[si].age > MINE_ARM
            && self.tanks.iter().any(|t| t.alive && hypot(t.x - sx, t.y - sy) < t.radius() + MINE_R)
        {
            self.mine_boom(si);
            return;
        }
        // Draufschießen zündet die Mine auch (Flammen ebenfalls)
        for oi in 0..self.shots.len() {
            let o = &self.shots[oi];
            if o.dead || o.kind.is_static() || matches!(o.kind, Kind::Swap | Kind::Ice | Kind::Spider) {
                continue;
            }
            if hypot(o.x - sx, o.y - sy) < o.r + MINE_R {
                if o.kind == Kind::Frag {
                    self.explode(oi);
                } else {
                    self.shots[oi].dead = true;
                }
                self.mine_boom(si);
                return;
            }
        }
    }

    /// Geschoss si berührt Panzer ti.
    fn contact(&mut self, si: usize, ti: usize) {
        let k = self.shots[si].kind;
        let tslot = self.tanks[ti].slot;
        if self.tanks[ti].has(Eff::Mirror) && k.reflect() && self.shots[si].owner != tslot {
            // zurück zum Absender
            let s = &mut self.shots[si];
            s.vx = -s.vx;
            s.vy = -s.vy;
            s.owner = tslot;
            s.grace = s.age + 0.25;
            let (x, y) = qp(s.x, s.y);
            self.fx(FxData::Mirror { x, y }, 0.3);
            return;
        }
        if k.pierce() {
            // fliegt weiter, trifft aber jeden nur einmal
            let bit = 1u8 << tslot;
            if self.shots[si].hit_mask & bit == 0 {
                self.shots[si].hit_mask |= bit;
                let pierce = matches!(k, Kind::Cannon | Kind::Gold);
                let s = &self.shots[si];
                let (a, owner, b) = (s.heading(), s.owner, s.bounces);
                self.hit(ti, Some(owner), k, pierce, Some(a), b);
            }
            return;
        }
        if k == Kind::Double {
            let (by, now) = (self.shots[si].owner, self.gtime);
            match self.tanks[ti].mark {
                Some((owner, until)) if owner == by && until > now => self.tanks[ti].mark = None,
                _ => {
                    self.tanks[ti].mark = Some((by, now + DOUBLE_MARK));
                    self.shots[si].dead = true;
                    return;
                }
            }
        }
        if k == Kind::Sticky {
            // klebt am Panzer fest
            let s = &mut self.shots[si];
            s.stuck = Some(tslot as i8);
            s.vx = 0.0;
            s.vy = 0.0;
            s.speed = 0.0;
            s.life = s.age + STICKY_FUSE;
            return;
        }
        if k.blast().is_some() {
            self.boom_shot(si, 0.8);
            return;
        }
        if k == Kind::Fw {
            self.burst(si);
            return;
        }
        if k == Kind::Napalm {
            self.napalm(si);
            return;
        }
        if k == Kind::Hook {
            // zieht den Getroffenen zum Schützen, kurz kann er nichts machen
            let s = &mut self.shots[si];
            s.stuck = Some(tslot as i8);
            s.vx = 0.0;
            s.vy = 0.0;
            s.speed = 0.0;
            s.life = s.age + HOOK_HOLD;
            self.freeze(ti, HOOK_STUN);
            return;
        }
        self.shots[si].dead = true;
        match k {
            Kind::Rang if tslot == self.shots[si].owner => {
                // gefangen -> wieder werfen
                if self.tanks[ti].weapon.is_none() {
                    self.tanks[ti].weapon = Some(Item::Rang);
                    self.tanks[ti].ammo = 1;
                }
            }
            Kind::Swap => self.swap(si, ti),
            Kind::Ice => self.freeze(ti, FREEZE_TIME),
            _ => {
                if k == Kind::Mrk {
                    let (x, y) = qp(self.shots[si].x, self.shots[si].y);
                    self.fx(FxData::Boom { x, y, size: 30 }, 0.3);
                }
                let s = &self.shots[si];
                let (a, owner, b) = (s.heading(), s.owner, s.bounces);
                self.hit(ti, Some(owner), k, false, Some(a), b);
            }
        }
    }

    fn expire(&mut self, si: usize) {
        let k = self.shots[si].kind;
        let (x, y) = qp(self.shots[si].x, self.shots[si].y);
        match k {
            Kind::Chain => return self.chain_end(si),
            Kind::Hammer => return self.hammer_swing(si),
            Kind::Shadow => return self.shadow_fire(si),
            Kind::Twist => return self.twist_pulse(si),
            Kind::Afterburn => return self.afterburn_hit(si),
            Kind::Cracker => return self.cracker_boom(si),
            Kind::Cross => return self.cross_hit(si),
            Kind::Plate if self.shots[si].cool == 0.0 => {
                self.shots[si].dead = true;
                return;
            }
            // Köder, der nie ausgelöst wurde, verschwindet still
            Kind::Decoy if self.shots[si].cool == 0.0 => {
                self.shots[si].dead = true;
                return;
            }
            _ => {}
        }
        match k {
            Kind::Frag => return self.explode(si),
            Kind::Nuke => return self.nuke(si),
            Kind::Fw => return self.burst(si),
            Kind::Napalm => return self.napalm(si),
            Kind::Spider => {
                // zerfällt nach einer Weile harmlos
                self.shots[si].dead = true;
                self.fx(FxData::Boom { x, y, size: 30 }, 0.3);
                return;
            }
            _ => {}
        }
        if k == Kind::Ring {
            return self.ring_boom(si);
        }
        if k.blast().is_some() {
            let size = match k {
                Kind::Timebomb | Kind::Destruct => 1.4,
                Kind::Arty | Kind::Meteor => 1.0,
                _ => 0.8,
            };
            return self.boom_shot(si, size);
        }
        if k == Kind::Sentry {
            return self.sentry_boom(si);
        }
        self.shots[si].dead = true;
        match k {
            Kind::Rocket | Kind::Mrk | Kind::Plasma => self.fx(FxData::Boom { x, y, size: 50 }, 0.4), // Treibstoff alle
            Kind::Strike => {
                let (sx, sy, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
                self.blast(sx, sy, STRIKE_BLAST, Some(owner), Kind::Strike, 1.0, false);
            }
            Kind::Hole | Kind::Orbit => self.fx(FxData::Boom { x, y, size: 40 }, 0.35),
            _ => {}
        }
    }

    /// Kugelfresser: schluckt normale Kugeln, wird dabei größer und langsamer.
    fn eat(&mut self, si: usize) {
        let (ex, ey, eowner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        for j in 0..self.shots.len() {
            if self.shots[si].eaten >= EATER_MAX || self.shots[si].dead {
                break;
            }
            let o = &self.shots[j];
            if j == si || o.dead || o.owner == eowner || !matches!(o.kind, Kind::B | Kind::Mg | Kind::Sg | Kind::Bnc) {
                continue;
            }
            if hypot(o.x - ex, o.y - ey) >= self.shots[si].r + o.r {
                continue;
            }
            self.shots[j].dead = true;
            let s = &mut self.shots[si];
            s.eaten += 1;
            s.r += 3.0;
            let v = (s.speed - 20.0).max(40.0);
            let f = v / s.speed;
            s.vx *= f;
            s.vy *= f;
            s.speed = v;
        }
    }

    /// Hohlringgranate: trifft nur Panzer zwischen RING_IN und RING_OUT, Zentrum ist sicher.
    fn ring_boom(&mut self, si: usize) {
        let (x, y, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        self.shots[si].dead = true;
        let (px, py) = qp(x, y);
        self.fx(FxData::Boom { x: px, y: py, size: 60 }, 0.5);
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if !t.alive {
                continue;
            }
            let d = hypot(t.x - x, t.y - y);
            if (RING_IN..=RING_OUT).contains(&d) {
                let a = (t.y - y).atan2(t.x - x);
                self.hit(ti, Some(owner), Kind::Ring, false, Some(a), 0);
            }
        }
    }

    /// Köderladung: scharf nach DECOY_ARM, löst aus, sobald eine gegnerische Kugel nah genug kommt.
    fn decoy_check(&mut self, si: usize) {
        let s = &self.shots[si];
        if s.cool > 0.0 || s.age < DECOY_ARM {
            return;
        }
        let (x, y, owner) = (s.x, s.y, s.owner);
        let hit = self.shots.iter().any(|o| {
            !o.dead && o.owner != owner && o.kind != Kind::Decoy && hypot(o.x - x, o.y - y) <= DECOY_SENSE + o.r
        });
        if hit {
            let s = &mut self.shots[si];
            s.cool = 1.0;
            s.life = s.age + DECOY_FUSE;
        }
    }

    /// Kettenzange: nach der Haltezeit trifft die Zange das Opfer, falls der Angreifer noch lebt.
    fn chain_end(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (owner, victim) = (self.shots[si].owner, self.shots[si].stuck);
        let Some(vslot) = victim else { return };
        let Some(vi) = self.tank_index(vslot as usize) else { return };
        let attacker_alive = self.tank_index(owner).is_some_and(|ai| self.tanks[ai].alive);
        if attacker_alive && self.tanks[vi].alive {
            let a = (self.tanks[vi].y - self.shots[si].y).atan2(self.tanks[vi].x - self.shots[si].x);
            self.hit(vi, Some(owner), Kind::Chain, false, Some(a), 0);
        }
    }

    /// Schwenkhammer: schlägt einmal durch den Sektor vor dem Panzer.
    fn hammer_swing(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (x, y, a, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].ang, self.shots[si].owner);
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if !t.alive || t.slot == owner {
                continue;
            }
            if hypot(t.x - x, t.y - y) <= HAMMER_RANGE + t.radius()
                && angle_diff((t.y - y).atan2(t.x - x), a).abs() <= HAMMER_HALF
            {
                let ang = (t.y - y).atan2(t.x - x);
                self.hit(ti, Some(owner), Kind::Hammer, false, Some(ang), 0);
            }
        }
    }

    /// Schattensalve: die Silhouette feuert drei Kugeln in ihrer Blickrichtung.
    fn shadow_fire(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (x, y, a, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].ang, self.shots[si].owner);
        if !self.tank_index(owner).is_some_and(|ti| self.tanks[ti].alive) {
            return;
        }
        for d in [-SHADOW_SPREAD, 0.0, SHADOW_SPREAD] {
            self.spawn(Kind::B, owner, x, y, a + d, SHADOW_SPEED, 3.0, SHADOW_LIFE, OWNER_GRACE, MUZZLE);
        }
    }

    /// Rohrverdreher: Panzer im Umkreis drehen sich um 90°.
    fn twist_pulse(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (x, y) = (self.shots[si].x, self.shots[si].y);
        for t in self.tanks.iter_mut() {
            if t.alive && hypot(t.x - x, t.y - y) <= TWIST_RANGE {
                t.a += std::f64::consts::FRAC_PI_2;
            }
        }
    }

    /// Geschossweiche: dreht gegnerische Kugeln im Umkreis um 90° nach links (höchstens SWITCH_MAX).
    fn switch_step(&mut self, si: usize) {
        let (zx, zy, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        for j in 0..self.shots.len() {
            if self.shots[si].eaten >= SWITCH_MAX {
                break;
            }
            let o = &self.shots[j];
            if j == si || o.dead || o.eaten != 0 || o.owner == owner || !matches!(o.kind, Kind::B | Kind::Mg | Kind::Sg | Kind::Bnc) {
                continue;
            }
            if hypot(o.x - zx, o.y - zy) > SWITCH_R {
                continue;
            }
            let o = &mut self.shots[j];
            let a = o.heading() - std::f64::consts::FRAC_PI_2;
            o.vx = a.cos() * o.speed;
            o.vy = a.sin() * o.speed;
            o.eaten = 1;
            self.shots[si].eaten += 1;
        }
    }

    /// Nachbrennbild: am Ende trifft die Linie alle Panzer, die auf ihr stehen.
    fn afterburn_hit(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (x, y, a, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].ang, self.shots[si].owner);
        let (x1, y1) = (x + a.cos() * AFTERBURN_LEN, y + a.sin() * AFTERBURN_LEN);
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if t.alive && seg_dist(t.x, t.y, x, y, x1, y1) <= AFTERBURN_HALF_W + t.radius() {
                self.hit(ti, Some(owner), Kind::Afterburn, false, Some(a), 0);
            }
        }
    }

    /// Richtungsdorn: scharf nach THORN_FUSE, löst aus, wenn ein Panzer von der Pfeilseite kommt.
    fn thorn_check(&mut self, si: usize) {
        if self.shots[si].age < THORN_FUSE {
            return;
        }
        let (x, y, a, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].ang, self.shots[si].owner);
        let (dx, dy) = (a.cos(), a.sin());
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if !t.alive {
                continue;
            }
            let (rx, ry) = (t.x - x, t.y - y);
            // der Leger darf drüberfahren, nur Gegner von der Pfeilseite lösen aus
            if t.slot == owner || hypot(rx, ry) > THORN_REACH + t.radius() || rx * dx + ry * dy <= 0.0 {
                continue;
            }
            self.shots[si].dead = true;
            self.hit(ti, Some(owner), Kind::Thorn, false, Some(a), 0);
            return;
        }
    }

    /// Trittplatte: zählt Betritte, nach PLATE_COUNT Stück gibt es eine Vorwarnung und dann die Explosion.
    fn plate_check(&mut self, si: usize, dt: f64) {
        if self.shots[si].cool > 0.0 {
            let c = self.shots[si].cool - dt;
            if c <= 0.0 {
                self.boom_shot(si, 0.8);
            } else {
                self.shots[si].cool = c;
            }
            return;
        }
        let (x, y) = (self.shots[si].x, self.shots[si].y);
        let inside = self.tanks.iter().any(|t| t.alive && hypot(t.x - x, t.y - y) < PLATE_R);
        let was = self.shots[si].ang > 0.0;
        if inside && !was {
            self.shots[si].eaten += 1;
            if self.shots[si].eaten >= PLATE_COUNT {
                self.shots[si].cool = PLATE_WARN;
            }
        }
        self.shots[si].ang = if inside { 1.0 } else { 0.0 };
    }

    /// Zangenmarkierung: die Zange läuft nach PINCER_FUSE auf die Mitte zu und trifft jeden Panzer auf dem Weg.
    fn pincer_step(&mut self, si: usize, dt: f64) {
        if self.shots[si].age < PINCER_FUSE {
            return;
        }
        let step = self.shots[si].speed * dt;
        {
            let s = &mut self.shots[si];
            s.x += s.vx * step;
            s.y += s.vy * step;
            s.cool += step;
        }
        if self.shots[si].cool >= PINCER_LAT {
            self.shots[si].dead = true;
            return;
        }
        let (x, y, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if t.alive && hypot(t.x - x, t.y - y) < PINCER_R + t.radius() {
                let a = (t.y - y).atan2(t.x - x);
                self.shots[si].dead = true;
                self.hit(ti, Some(owner), Kind::Pincer, false, Some(a), 0);
                return;
            }
        }
    }

    /// Fadenkreuzfluch: folgt dem Ziel bis CROSS_FOLLOW, danach bleibt es stehen.
    fn cross_step(&mut self, si: usize) {
        if self.shots[si].age >= CROSS_FOLLOW {
            return;
        }
        let Some(vs) = self.shots[si].stuck else { return };
        if let Some(vi) = self.tank_index(vs as usize) {
            let t = &self.tanks[vi];
            if t.alive {
                let (x, y) = (t.x, t.y);
                self.shots[si].x = x;
                self.shots[si].y = y;
            }
        }
    }

    /// Fadenkreuzfluch: nach dem Einfrieren trifft alles im Kreis.
    fn cross_hit(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (x, y, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if t.alive && hypot(t.x - x, t.y - y) <= CROSS_R {
                let a = (t.y - y).atan2(t.x - x);
                self.hit(ti, Some(owner), Kind::Cross, false, Some(a), 0);
            }
        }
    }

    /// Kreuzpendel: ein Arm dreht sich um den Sockel. Nach PENDEL_WARN trifft er alle außer dem Schützen.
    fn pendel_step(&mut self, si: usize, dt: f64) {
        self.shots[si].ang += PENDEL_SPEED * dt;
        if self.shots[si].age < PENDEL_WARN {
            return;
        }
        let (x, y, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        let a = self.shots[si].ang;
        let (x1, y1) = (x + a.cos() * PENDEL_LEN, y + a.sin() * PENDEL_LEN);
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if t.alive && t.slot != owner && seg_dist(t.x, t.y, x, y, x1, y1) <= PENDEL_R + t.radius() {
                self.hit(ti, Some(owner), Kind::Pendel, false, Some(a), 0);
            }
        }
    }

    /// Stillstandsurteil: folgt dem Ziel. Bleibt es VERDICT_STILL lang stehen, trifft das Urteil.
    fn verdict_step(&mut self, si: usize, dt: f64) {
        let owner = self.shots[si].owner;
        let Some(vs) = self.shots[si].stuck else {
            self.shots[si].dead = true;
            return;
        };
        let Some(vi) = self.tank_index(vs as usize) else {
            self.shots[si].dead = true;
            return;
        };
        if !self.tanks[vi].alive {
            self.shots[si].dead = true;
            return;
        }
        let (tx, ty) = (self.tanks[vi].x, self.tanks[vi].y);
        self.shots[si].x = tx;
        self.shots[si].y = ty;
        let (rx, ry) = (self.shots[si].vx, self.shots[si].vy);
        if hypot(tx - rx, ty - ry) > VERDICT_TOL {
            let s = &mut self.shots[si];
            s.vx = tx;
            s.vy = ty;
            s.cool = 0.0;
        } else {
            self.shots[si].cool += dt;
            if self.shots[si].cool >= VERDICT_STILL {
                self.shots[si].dead = true;
                self.hit(vi, Some(owner), Kind::Verdict, false, None, 0);
            }
        }
    }

    /// Geschossbrüter: wirft gegnerische Kugeln im Fangradius zurück, bis CATCHER_MAX erreicht sind.
    fn catcher_step(&mut self, si: usize) {
        let (x, y, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        for j in 0..self.shots.len() {
            if self.shots[si].eaten >= CATCHER_MAX {
                break;
            }
            let o = &self.shots[j];
            if j == si || o.dead || o.owner == owner || o.eaten != 0 || !matches!(o.kind, Kind::B | Kind::Mg | Kind::Sg | Kind::Bnc) {
                continue;
            }
            if hypot(o.x - x, o.y - y) > CATCHER_R {
                continue;
            }
            let o = &mut self.shots[j];
            o.vx = -o.vx;
            o.vy = -o.vy;
            o.owner = owner;
            o.grace = o.age + 0.25; // der Schütze des Geschossbrüters wird nicht sofort getroffen
            o.eaten = 1;
            self.shots[si].eaten += 1;
        }
    }

    /// Rostwolke: wer in der Wolke steht, kann nicht schießen (und danach RUST_PAUSE lang auch nicht).
    fn rust_step(&mut self, si: usize) {
        let (x, y) = (self.shots[si].x, self.shots[si].y);
        for t in self.tanks.iter_mut() {
            if t.alive && hypot(t.x - x, t.y - y) <= RUST_R {
                t.cooldown = t.cooldown.max(RUST_PAUSE);
            }
        }
    }

    /// Prallkissen: nach CUSHION_ARM scharf. Jeder, der neu hineinfährt, wird um CUSHION_PUSH weggeschoben.
    fn cushion_step(&mut self, si: usize) {
        if self.shots[si].age < CUSHION_ARM {
            return;
        }
        let (x, y, mask) = (self.shots[si].x, self.shots[si].y, self.shots[si].hit_mask);
        let mut now_inside = 0u8;
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if !t.alive || hypot(t.x - x, t.y - y) >= CUSHION_R + t.radius() {
                continue;
            }
            let bit = 1u8 << t.slot;
            now_inside |= bit;
            if mask & bit == 0 {
                let a = (t.y - y).atan2(t.x - x);
                let (nx, ny) = (t.x + a.cos() * CUSHION_PUSH, t.y + a.sin() * CUSHION_PUSH);
                let (sx, sy) = self.settle(ti, nx, ny);
                self.tanks[ti].x = sx;
                self.tanks[ti].y = sy;
            }
        }
        self.shots[si].hit_mask = now_inside;
    }

    /// Kanonenschlag: zündet und wirft CRACKER_N kleine Splitter in zufällige Richtungen.
    fn cracker_boom(&mut self, si: usize) {
        self.shots[si].dead = true;
        let (x, y, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        let base = self.rng.uniform(0.0, std::f64::consts::TAU);
        for k in 0..CRACKER_N {
            let a = base + std::f64::consts::TAU * k as f64 / CRACKER_N as f64;
            self.spawn(Kind::Frg, owner, x, y, a, CRACKER_SPEED, 3.0, 0.6, 0.0, 0.0);
        }
        let (px, py) = qp(x, y);
        self.fx(FxData::Boom { x: px, y: py, size: 25 }, 0.4);
    }

    /// Öl macht rutschig. Kommt Feuer dran, brennt es und erwischt jeden darin.
    /// Napalm brennt von Anfang an und kommt nicht durch Wände.
    fn oil(&mut self, si: usize) {
        let (sx, sy, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        let kind = self.shots[si].kind;
        let burning = kind != Kind::Oil;
        if !burning && self.shots.iter().any(|o| o.kind == Kind::Fl && hypot(o.x - sx, o.y - sy) < OIL_R) {
            self.ignite(si);
            return;
        }
        let (how, reach) = if kind == Kind::Burn { (Kind::Burn, NAPALM_R) } else { (Kind::Oil, OIL_R) };
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if !t.alive {
                continue;
            }
            if hypot(t.x - sx, t.y - sy) < reach + t.radius() * 0.5 {
                if kind == Kind::Burn && !self.arena.clear(sx, sy, t.x, t.y) {
                    continue;
                }
                if burning {
                    let a = (t.y - sy).atan2(t.x - sx);
                    self.hit(ti, Some(owner), how, false, Some(a), 0);
                } else {
                    self.tanks[ti].eff.set(Eff::Slip, 0.25);
                }
            }
        }
    }

    /// Bärenfalle: wer drüberfährt, hängt fest (auch der, der sie gelegt hat).
    fn trap_check(&mut self, si: usize) {
        if self.shots[si].age < MINE_ARM {
            return;
        }
        let (sx, sy) = (self.shots[si].x, self.shots[si].y);
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if t.alive && hypot(t.x - sx, t.y - sy) < t.radius() + MINE_R {
                self.shots[si].dead = true;
                self.freeze(ti, TRAP_TIME);
                let (x, y) = qp(sx, sy);
                self.fx(FxData::Trap { x, y }, 0.5);
                return;
            }
        }
    }

    /// Geschützturm: dreht sich zum nächsten sichtbaren Gegner und schießt.
    fn sentry(&mut self, si: usize, dt: f64) {
        let (sx, sy, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        let mut cool = self.shots[si].cool - dt;
        let mut ang = self.shots[si].ang;
        if let Some(oi) = self.nearest(sx, sy, Some(owner), SENTRY_RANGE, true) {
            let o = &self.tanks[oi];
            let diff = angle_diff((o.y - sy).atan2(o.x - sx), ang);
            ang += diff.clamp(-SENTRY_TURN * dt, SENTRY_TURN * dt);
            if diff.abs() < 0.1 && cool <= 0.0 {
                self.spawn(Kind::Sb, owner, sx, sy, ang, 260.0, 3.0, 2.2, 0.0, SENTRY_R + 4.0);
                cool = SENTRY_RATE;
            }
        }
        self.shots[si].cool = cool;
        self.shots[si].ang = ang;
        for oi in 0..self.shots.len() {
            // Treffer zerstören den Turm
            let o = &self.shots[oi];
            if o.dead
                || oi == si
                || o.kind.is_static()
                || matches!(o.kind, Kind::Swap | Kind::Spider)
                || (o.kind == Kind::Sb && o.owner == owner)
            {
                continue;
            }
            if hypot(o.x - sx, o.y - sy) < o.r + SENTRY_R {
                if !o.kind.pierce() {
                    self.shots[oi].dead = true;
                }
                self.sentry_boom(si);
                return;
            }
        }
    }

    /// Orbitallaser: erst zielen, dann wandert ein Strahl von oben weiter, Wände egal.
    fn orbit(&mut self, si: usize, dt: f64) {
        if self.shots[si].age < ORBIT_DELAY {
            return;
        }
        let a = self.shots[si].ang;
        let (nx, ny) = ahead(&self.arena, self.shots[si].x, self.shots[si].y, a, ORBIT_SPEED * dt, 4.0);
        self.shots[si].x = nx;
        self.shots[si].y = ny;
        let owner = self.shots[si].owner;
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if t.alive && hypot(t.x - nx, t.y - ny) < ORBIT_R + t.radius() {
                self.hit(ti, Some(owner), Kind::Orbit, true, Some(a), 0);
            }
        }
        for oi in 0..self.shots.len() {
            let o = &self.shots[oi];
            if o.dead || hypot(o.x - nx, o.y - ny) > ORBIT_R + 8.0 {
                continue;
            }
            match o.kind {
                Kind::Mine => self.mine_boom(oi),
                Kind::Oil => self.ignite(oi),
                _ => {}
            }
        }
    }

    /// Laserschranke: nach kurzer Zeit tödlich für jeden, der den Strahl berührt.
    fn fence(&mut self, si: usize) {
        if self.shots[si].age < FENCE_ARM {
            return;
        }
        let (sx, sy, a, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].ang, self.shots[si].owner);
        let ((x0, y0), (x1, y1)) = fence_ends(&self.arena, sx, sy, a);
        for ti in 0..self.tanks.len() {
            let t = &self.tanks[ti];
            if t.alive && seg_dist(t.x, t.y, x0, y0, x1, y1) < t.radius() {
                self.hit(ti, Some(owner), Kind::Fence, false, Some(a), 0);
            }
        }
    }

    /// Kugelsprinkler: dreht sich und schießt in zwei entgegengesetzte Richtungen.
    fn sprinkler(&mut self, si: usize, dt: f64) {
        let s = &mut self.shots[si];
        s.ang += SPRINKLER_TURN * dt;
        s.cool -= dt;
        if s.cool > 0.0 {
            return;
        }
        s.cool = SPRINKLER_RATE;
        let (x, y, owner, a) = (s.x, s.y, s.owner, s.ang);
        for off in [0.0, std::f64::consts::PI] {
            self.spawn(Kind::Spb, owner, x, y, a + off, SPRINKLER_SPEED, 3.0, SPRINKLER_LIFE, 0.5, SENTRY_R + 4.0);
        }
    }

    /// Enterhaken: an der Wand zieht er den Schützen hin, an einem Panzer den Panzer her.
    fn hook_pull(&mut self, si: usize, dt: f64) {
        let owner = self.tank_index(self.shots[si].owner).filter(|&i| self.tanks[i].alive);
        let Some(oi) = owner else {
            self.shots[si].dead = true;
            return;
        };
        let (mover, tx, ty, stop) = match self.shots[si].stuck {
            Some(slot) if slot >= 0 => {
                let Some(vi) = self.tank_index(slot as usize).filter(|&i| self.tanks[i].alive) else {
                    self.shots[si].dead = true;
                    return;
                };
                (vi, self.tanks[oi].x, self.tanks[oi].y, self.tank_r(oi) + self.tank_r(vi) + 4.0)
            }
            _ => (oi, self.shots[si].x, self.shots[si].y, self.tank_r(oi) + 8.0),
        };
        let (mx, my) = (self.tanks[mover].x, self.tanks[mover].y);
        let d = hypot(tx - mx, ty - my);
        let step = (HOOK_PULL * dt).min(d - stop);
        if step <= 0.0 {
            self.shots[si].dead = true; // angekommen
            return;
        }
        let (nx, ny) = self.settle(mover, mx + (tx - mx) / d * step, my + (ty - my) / d * step);
        if hypot(nx - mx, ny - my) < step * 0.25 {
            self.shots[si].dead = true; // hängt an einer Ecke fest
            return;
        }
        self.tanks[mover].x = nx;
        self.tanks[mover].y = ny;
        if mover != oi {
            self.shots[si].x = nx;
            self.shots[si].y = ny;
        }
    }

    /// Klebebombe hängt an einem Panzer (Slot) oder an der Wand (-1).
    fn stick(&mut self, si: usize) {
        if let Some(slot) = self.shots[si].stuck {
            if slot >= 0 {
                if let Some(ti) = self.tank_index(slot as usize) {
                    if self.tanks[ti].alive {
                        self.shots[si].x = self.tanks[ti].x;
                        self.shots[si].y = self.tanks[ti].y;
                    }
                }
            }
        }
    }

    /// Spinnenmine: wird scharf, wartet und krabbelt dann zum nächsten Gegner.
    fn spider(&mut self, si: usize, dt: f64) -> bool {
        if self.shots[si].age < MINE_ARM {
            return false;
        }
        let (sx, sy, owner) = (self.shots[si].x, self.shots[si].y, self.shots[si].owner);
        let Some(oi) = self.nearest(sx, sy, Some(owner), SPIDER_SENSE, false) else {
            let s = &mut self.shots[si];
            s.vx = 0.0;
            s.vy = 0.0;
            s.speed = 0.0;
            return true;
        };
        self.shots[si].speed = SPIDER_SPEED;
        let (tx, ty) = self.waypoint(sx, sy, self.tanks[oi].x, self.tanks[oi].y);
        self.turn_toward(si, tx, ty, 6.0, dt);
        true
    }

    /// Phasenkugel fliegt durch Wände, die Plasmakugel schmilzt sie weg.
    /// False, wenn das Geschoss dabei verschwindet.
    fn glide(&mut self, si: usize, dt: f64) -> bool {
        let s = &self.shots[si];
        let (nx, ny) = (s.x + s.vx * dt, s.y + s.vy * dt);
        let r = s.r;
        if s.kind == Kind::Phase {
            let (x0, y0, x1, y1) = self.arena.bounds();
            if !(x0 < nx && nx < x1 && y0 < ny && ny < y1) {
                self.shots[si].dead = true;
                return false;
            }
        } else if circle_hits_walls(nx, ny, r, self.arena.near(nx, ny)) {
            self.shots[si].bounces += 1;
            self.blow_walls(nx, ny, 60.0);
            let (qx, qy) = qp(nx, ny);
            self.fx(FxData::Boom { x: qx, y: qy, size: 50 }, 0.4);
            if self.shots[si].bounces >= PLASMA_WALLS || circle_hits_walls(nx, ny, r, self.arena.near(nx, ny)) {
                self.shots[si].dead = true;
                return false;
            }
        }
        self.shots[si].x = nx;
        self.shots[si].y = ny;
        true
    }

    /// Suchkugel lenkt zum nächsten sichtbaren Panzer, nach 1 s auch zum Schützen.
    fn seek(&mut self, si: usize, dt: f64) {
        let s = &self.shots[si];
        let owner = if s.age < 1.0 { Some(s.owner) } else { None };
        if let Some(oi) = self.nearest(s.x, s.y, owner, 1e9, true) {
            let (ox, oy) = (self.tanks[oi].x, self.tanks[oi].y);
            self.turn_toward(si, ox, oy, SEEK_TURN, dt);
        }
    }
}
