//! Autoritative Spiellogik (läuft nur auf dem Server bzw. lokal).

mod shots;
mod view;
mod weapons;

use std::f64::consts::PI;

use crate::arena::Arena;
use crate::consts::*;
use crate::geom::{circle_hits_walls, hypot, push_out};
use crate::items::{Eff, Item, Kind};
use crate::maze::Maze;
use crate::physics::{bullet_params, clamp_arena, move_tank, tank_radius, Body, Effects};
use crate::rng::Rng;
use crate::snap::{qp, Ann, FxData, PopText};
use crate::text;

#[derive(Clone, Debug)]
pub struct Tank {
    /// Spieler-Nummer = Farbe, bleibt über Runden gleich
    pub slot: usize,
    pub x: f64,
    pub y: f64,
    pub a: f64,
    pub alive: bool,
    pub dead_age: f64,
    pub cooldown: f64,
    pub fire_seen: Option<u16>,
    pub weapon: Option<Item>,
    pub ammo: i32,
    pub shield: f64,
    /// Railgun: so lange wird schon geladen
    pub charge: f64,
    pub eff: Effects,
    pub taunt: u8,
    pub taunt_time: f64,
    pub taunt_key: u8,
    /// Entwaffnet: so lange kann die liegende Waffe nicht aufgehoben werden
    pub nopick: f64,
    /// Zweitakt: wer den Panzer markiert hat und bis wann (Spielzeit)
    pub mark: Option<(usize, f64)>,
}

impl Tank {
    fn new(slot: usize, x: f64, y: f64, a: f64) -> Tank {
        Tank {
            slot,
            x,
            y,
            a,
            alive: true,
            dead_age: 0.0,
            cooldown: 0.0,
            fire_seen: None,
            weapon: None,
            ammo: 0,
            shield: 0.0,
            charge: 0.0,
            eff: Effects::default(),
            taunt: 0,
            taunt_time: 0.0,
            taunt_key: 0,
            nopick: 0.0,
            mark: None,
        }
    }

    pub fn radius(&self) -> f64 {
        tank_radius(&self.eff)
    }

    pub fn has(&self, e: Eff) -> bool {
        self.eff.has(e)
    }
}

#[derive(Clone, Debug)]
pub struct Shot {
    pub id: u32,
    pub kind: Kind,
    pub owner: usize,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub speed: f64,
    pub r: f64,
    pub life: f64,
    pub grace: f64,
    pub age: f64,
    /// wird am Ende von Game::step() aussortiert
    pub dead: bool,
    /// Abpraller insgesamt (für Trickshots)
    pub bounces: u32,
    /// durchschlagende Geschosse: schon getroffene Slots
    pub hit_mask: u8,
    /// Fernlenkrakete: Lenktasten
    pub keys: u8,
    /// Klebebombe: an Slot (>= 0) oder an der Wand (-1)
    pub stuck: Option<i8>,
    /// Geschützturm: Nachladen
    pub cool: f64,
    /// Geschützturm/Orbitallaser: Winkel
    pub ang: f64,
    /// Kugelfresser: geschluckte Kugeln
    pub eaten: u8,
}

impl Shot {
    #[allow(clippy::too_many_arguments)]
    fn new(
        id: u32,
        kind: Kind,
        owner: usize,
        x: f64,
        y: f64,
        a: f64,
        speed: f64,
        r: f64,
        life: f64,
        grace: f64,
    ) -> Shot {
        Shot {
            id,
            kind,
            owner,
            x,
            y,
            vx: a.cos() * speed,
            vy: a.sin() * speed,
            speed,
            r,
            life,
            grace,
            age: 0.0,
            dead: false,
            bounces: 0,
            hit_mask: 0,
            keys: 0,
            stuck: None,
            cool: 0.0,
            ang: 0.0,
            eaten: 0,
        }
    }

    pub fn heading(&self) -> f64 {
        self.vy.atan2(self.vx)
    }

    /// Bewegt die Kugel, prallt achsweise an Wänden ab (Substeps von max. 3 px).
    /// Gibt die Anzahl der Abpraller zurück.
    pub fn travel(&mut self, dt: f64, arena: &Arena) -> u32 {
        let mut bounces = 0;
        let steps = ((self.speed * dt / 3.0).ceil() as i64).max(1);
        let f = dt / steps as f64;
        for _ in 0..steps {
            self.x += self.vx * f;
            if circle_hits_walls(self.x, self.y, self.r, arena.near(self.x, self.y)) {
                self.x -= self.vx * f;
                self.vx = -self.vx;
                bounces += 1;
            }
            self.y += self.vy * f;
            if circle_hits_walls(self.x, self.y, self.r, arena.near(self.x, self.y)) {
                self.y -= self.vy * f;
                self.vy = -self.vy;
                bounces += 1;
            }
        }
        bounces
    }
}

#[derive(Clone, Debug)]
pub struct Fx {
    pub id: u32,
    pub data: FxData,
    pub life: f64,
    pub age: f64,
}

#[derive(Clone, Debug)]
pub struct Feed {
    /// Schütze (None = Umwelt bzw. selbst)
    pub by: Option<usize>,
    pub victim: usize,
    pub how: Kind,
    /// welcher der allgemeinen Eigentor-Sprüche
    pub variant: u8,
    pub age: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Win {
    pub slot: Option<usize>,
    /// Siege in Folge (für den Untertitel)
    pub streak: u32,
}

/// Gebaute Wand: Rechteck und Restzeit
#[derive(Clone, Debug)]
pub struct TempWall {
    pub rect: [i32; 4],
    pub left: f64,
}

pub struct Game {
    pub players: Vec<usize>,
    pub scores: [u32; MAX_PLAYERS],
    /// gewonnene Runden in Folge
    pub streak: [u32; MAX_PLAYERS],
    pub last_winner: Option<usize>,
    pub maze_id: u32,
    /// Echtzeit seit Spielbeginn
    pub time: f64,
    /// Spielzeit (in der Zeitlupe langsamer)
    pub gtime: f64,
    pub next_id: u32,
    pub fire_seen: [Option<u16>; MAX_PLAYERS],
    pub feed: Vec<Feed>,
    pub mutator: Option<Mutator>,
    /// welche Sonderregeln dieses Spiel benutzt
    pub rule: RoundRule,
    pub theme: Option<usize>,
    /// Slots der Computergegner (nur für die Anzeige)
    pub bots: Vec<usize>,
    pub maze: Maze,
    pub arena: Arena,
    pub tanks: Vec<Tank>,
    by_slot: [Option<usize>; MAX_PLAYERS],
    pub shots: Vec<Shot>,
    pub boxes: Vec<(f64, f64, Item)>,
    pub fx: Vec<Fx>,
    pub box_timer: f64,
    pub box_max: usize,
    pub end_timer: Option<f64>,
    pub win: Option<Win>,
    /// slot -> Zeitpunkte seiner Abschüsse in dieser Runde
    pub kills: [Vec<f64>; MAX_PLAYERS],
    pub first_blood: bool,
    /// Zeitlupe: Restzeit und Gesamtlänge
    pub slow: f64,
    pub slow_len: f64,
    /// aktueller Zeitfaktor
    pub ts: f64,
    pub temp: Vec<TempWall>,
    /// Spielerliste für die nächste Runde (wer beitritt, wartet bis dahin)
    pub pending: Option<(Vec<usize>, Vec<usize>)>,
    pub rng: Rng,
}

impl Game {
    pub fn new(players: &[usize]) -> Game {
        Self::with_rng(players, Rng::from_entropy())
    }

    pub fn with_rng(players: &[usize], rng: Rng) -> Game {
        Self::build(players, rng, RoundRule::default())
    }

    /// Spiel mit festgelegter Sonderregel (gilt schon für die erste Runde)
    pub fn with_rule(players: &[usize], rule: RoundRule) -> Game {
        Self::build(players, Rng::from_entropy(), rule)
    }

    fn build(players: &[usize], mut rng: Rng, rule: RoundRule) -> Game {
        let maze = Maze::generate(0, players.len().max(2), &mut rng);
        let arena = Arena::new(&maze);
        let mut g = Game {
            players: players.to_vec(),
            scores: [0; MAX_PLAYERS],
            streak: [0; MAX_PLAYERS],
            last_winner: None,
            maze_id: 0,
            time: 0.0,
            gtime: 0.0,
            next_id: 1,
            fire_seen: [None; MAX_PLAYERS],
            feed: Vec::new(),
            mutator: None,
            rule,
            theme: None,
            bots: Vec::new(),
            maze,
            arena,
            tanks: Vec::new(),
            by_slot: [None; MAX_PLAYERS],
            shots: Vec::new(),
            boxes: Vec::new(),
            fx: Vec::new(),
            box_timer: BOX_FIRST,
            box_max: BOX_MAX,
            end_timer: None,
            win: None,
            kills: Default::default(),
            first_blood: false,
            slow: 0.0,
            slow_len: 0.0,
            ts: 1.0,
            temp: Vec::new(),
            pending: None,
            rng,
        };
        g.new_round();
        g
    }

    /// Neue Spielerliste -> sofort neue Runde. Neue Spieler (fresh) starten bei 0 Punkten.
    pub fn set_players(&mut self, players: &[usize], fresh: &[usize]) {
        for &slot in fresh {
            self.scores[slot] = 0;
            self.streak[slot] = 0;
            self.fire_seen[slot] = None; // neuer Client zählt seine Schüsse ab 0
        }
        self.players = players.to_vec();
        self.new_round();
    }

    /// Neue Spielerliste ab der nächsten Runde (die laufende Runde geht weiter).
    /// fresh = neue Spieler, starten bei 0 Punkten.
    pub fn queue_players(&mut self, players: &[usize], fresh: &[usize]) {
        let mut f: Vec<usize> = self.pending.take().map(|p| p.1).unwrap_or_default();
        for &s in fresh {
            if !f.contains(&s) {
                f.push(s);
            }
        }
        f.retain(|s| players.contains(s));
        self.pending = Some((players.to_vec(), f));
    }

    /// Spieler verlässt das Spiel: sein Panzer verschwindet, die Runde läuft weiter.
    pub fn remove_player(&mut self, slot: usize) {
        self.players.retain(|&s| s != slot);
        self.tanks.retain(|t| t.slot != slot);
        self.by_slot = [None; MAX_PLAYERS];
        for (i, t) in self.tanks.iter().enumerate() {
            self.by_slot[t.slot] = Some(i);
        }
        self.fire_seen[slot] = None;
        if let Some((p, f)) = self.pending.as_mut() {
            p.retain(|&s| s != slot);
            f.retain(|&s| s != slot);
        }
    }

    pub fn tank_index(&self, slot: usize) -> Option<usize> {
        self.by_slot.get(slot).copied().flatten()
    }

    pub fn tank(&self, slot: usize) -> Option<&Tank> {
        self.tank_index(slot).map(|i| &self.tanks[i])
    }

    pub fn new_round(&mut self) {
        if let Some((players, fresh)) = self.pending.take() {
            for &slot in &fresh {
                self.scores[slot] = 0;
                self.streak[slot] = 0;
                self.fire_seen[slot] = None;
            }
            self.players = players;
        }
        self.maze_id += 1;
        let mut maze = Maze::generate(self.maze_id, self.players.len(), &mut self.rng);
        let m = match self.rule {
            RoundRule::Off => None,
            RoundRule::Only(m) => Some(m),
            // Zufall: nie zweimal dieselbe hintereinander
            RoundRule::Random if self.rng.random() < MUTATOR_CHANCE => {
                let pool: Vec<Mutator> = MUTATORS.iter().copied().filter(|&x| Some(x) != self.mutator).collect();
                Some(self.rng.choice(&pool))
            }
            RoundRule::Random => None,
        };
        self.mutator = m;
        maze.mutator = m;
        // Jede Runde ein anderer Look (rein optisch)
        let pool: Vec<usize> = (0..THEMES.len()).filter(|&k| Some(k) != self.theme).collect();
        let theme = self.rng.choice(&pool);
        self.theme = Some(theme);
        maze.theme = theme as u8;
        self.maze = maze;
        self.arena = Arena::new(&self.maze);
        let cells = self.arena.cells();
        let n = self.players.len();
        // Startfelder mit möglichst großem Abstand, wenn es nicht klappt etwas lockerer.
        let (rows, cols) = (self.maze.rows as i64, self.maze.cols as i64);
        let min_dist = ((rows + cols) / 2).min(2.max((1.6 * (cells.len() as f64 / n.max(1) as f64).sqrt()) as i64));
        let mut spots = Vec::new();
        for attempt in 0..400 {
            spots = self.rng.sample(&cells, n);
            let need = min_dist - attempt / 100;
            let ok = (0..spots.len()).all(|i| {
                (i + 1..spots.len()).all(|j| {
                    let (a, b) = (spots[i], spots[j]);
                    (a.0 as i64 - b.0 as i64).abs() + (a.1 as i64 - b.1 as i64).abs() >= need
                })
            });
            if ok {
                break;
            }
        }
        self.tanks.clear();
        self.by_slot = [None; MAX_PLAYERS];
        let players = self.players.clone();
        for (k, (&slot, &cell)) in players.iter().zip(spots.iter()).enumerate() {
            let (x, y) = self.arena.center(cell);
            let a = self.rng.choice(&[0.0, 0.5, 1.0, 1.5]) * PI;
            let mut t = Tank::new(slot, x, y, a);
            t.fire_seen = self.fire_seen[slot];
            t.eff.set(Eff::Spawn, SPAWN_SAFE); // z. B. an der Wand erschienen und sofort geschossen
            self.tanks.push(t);
            self.by_slot[slot] = Some(k);
        }
        self.shots.clear();
        self.boxes.clear();
        self.fx.clear();
        self.box_timer = if m == Some(Mutator::Kisten) { 1.0 } else { BOX_FIRST };
        self.box_max = BOX_MAX.max(cells.len() / 30); // große Labyrinthe: mehr Kisten
        self.end_timer = None;
        self.win = None;
        self.kills = Default::default();
        self.first_blood = false;
        self.slow = 0.0;
        self.slow_len = 0.0;
        self.ts = 1.0;
        self.temp.clear();
    }

    /// Eingabe eines Spielers. Beim Client-Panzer wird das pro empfangenem Input-Kommando
    /// mit dessen dt aufgerufen, damit die Prediction des Clients exakt übereinstimmt.
    pub fn apply_input(&mut self, slot: usize, keys: u8, fires: u16, dt: f64) {
        let Some(ti) = self.tank_index(slot) else { return };
        // Schuss-Zähler: jeder Tastendruck erhöht ihn -> keine verlorenen Taps
        let new_shot = matches!(self.tanks[ti].fire_seen, Some(seen) if seen != fires);
        self.tanks[ti].fire_seen = Some(fires);
        self.fire_seen[slot] = Some(fires);
        if !self.tanks[ti].alive {
            return;
        }
        let taunt = (keys >> TAUNT_SHIFT) & 7;
        {
            let t = &mut self.tanks[ti];
            if taunt != 0 && taunt != t.taunt_key && taunt as usize <= TAUNTS.len() {
                t.taunt = taunt;
                t.taunt_time = TAUNT_TIME;
            }
            t.taunt_key = taunt;
            let mut body = Body { x: t.x, y: t.y, a: t.a };
            move_tank(&mut body, &t.eff, t.charge > 0.0, keys, dt, &self.arena);
            t.x = body.x;
            t.y = body.y;
            t.a = body.a;
            t.cooldown = (t.cooldown - dt).max(0.0);
            if t.weapon != Some(Item::Rail) {
                t.charge = 0.0;
            }
            if t.has(Eff::Frozen) {
                return;
            }
        }
        if self.tanks[ti].has(Eff::Steer) {
            // Fernlenkrakete: Links/Rechts lenken, 2. Druck zündet
            if let Some(si) = self.shots.iter().position(|s| s.owner == slot && s.kind == Kind::Rc && !s.dead) {
                self.shots[si].keys = keys & (LEFT | RIGHT);
                if new_shot {
                    self.boom_shot(si, 0.8);
                }
                return;
            }
            self.tanks[ti].eff.remove(Eff::Steer);
        }
        if self.tanks[ti].weapon == Some(Item::Rail) {
            // Feuer halten lädt, loslassen schießt
            if keys & SHOOT != 0 && !self.in_wall(ti) {
                let t = &mut self.tanks[ti];
                t.charge = (t.charge + dt).min(RAIL_CHARGE);
            } else if self.tanks[ti].charge > 0.0 {
                if self.tanks[ti].charge >= RAIL_CHARGE && !self.in_wall(ti) {
                    self.rail(ti);
                    self.use_ammo(ti);
                    self.tanks[ti].cooldown = FIRE_COOLDOWN;
                }
                self.tanks[ti].charge = 0.0;
            }
        }
        if new_shot {
            self.trigger(ti);
        }
        // Dauerfeuer-Waffen: solange die Taste gedrückt ist
        let w = self.tanks[ti].weapon;
        if matches!(w, Some(Item::Mg) | Some(Item::Flame))
            && keys & SHOOT != 0
            && self.tanks[ti].cooldown == 0.0
            && !self.in_wall(ti)
        {
            let a = self.tanks[ti].a;
            if w == Some(Item::Mg) {
                let off = self.rng.uniform(-MG_SPREAD, MG_SPREAD);
                self.shoot(Kind::Mg, ti, a + off, BULLET_SPEED * 1.1, MG_R, MG_LIFE, OWNER_GRACE);
                self.tanks[ti].cooldown = MG_RATE;
            } else {
                let off = self.rng.uniform(-FLAME_SPREAD, FLAME_SPREAD);
                let speed = FLAME_SPEED * self.rng.uniform(0.85, 1.1);
                let life = FLAME_LIFE * self.rng.uniform(0.8, 1.1);
                // die eigenen Flammen verbrennen einen nicht
                self.shoot(Kind::Fl, ti, a + off, speed, FLAME_R, life, FLAME_LIFE * 2.0);
                self.tanks[ti].cooldown = FLAME_RATE;
            }
            self.use_ammo(ti);
        }
    }

    /// Ein Geist in der Wand kann nicht schießen (die Kugel würde drin stecken).
    pub fn in_wall(&self, ti: usize) -> bool {
        let t = &self.tanks[ti];
        t.has(Eff::Ghost) && circle_hits_walls(t.x, t.y, 11.0, self.arena.near(t.x, t.y))
    }

    fn use_ammo(&mut self, ti: usize) {
        let t = &mut self.tanks[ti];
        t.ammo -= 1;
        if t.ammo <= 0 {
            t.weapon = None;
        }
    }

    fn new_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        id
    }

    #[allow(clippy::too_many_arguments)]
    fn shoot(&mut self, kind: Kind, ti: usize, a: f64, speed: f64, r: f64, life: f64, grace: f64) -> usize {
        let (x, y, owner) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].slot);
        self.spawn(kind, owner, x, y, a, speed, r, life, grace, MUZZLE)
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn(
        &mut self,
        kind: Kind,
        owner: usize,
        mut x: f64,
        mut y: f64,
        a: f64,
        speed: f64,
        r: f64,
        life: f64,
        grace: f64,
        muzzle: f64,
    ) -> usize {
        let id = self.new_id();
        let s = if kind == Kind::Phase {
            // fliegt durch Wände, startet direkt am Rohrende
            x += a.cos() * muzzle;
            y += a.sin() * muzzle;
            Shot::new(id, kind, owner, x, y, a, speed, r, life, grace)
        } else {
            // Mini-Panzer an der Wand + dicke Kugel: Startpunkt erst aus der Wand schieben
            let (px, py) = push_out(x, y, r, self.arena.near(x, y));
            let mut s = Shot::new(id, kind, owner, px, py, a, speed, r, life, grace);
            if speed > 0.0 {
                s.travel(muzzle / speed, &self.arena); // bis zum Rohrende, prallt korrekt ab
            } else {
                s.travel(0.0, &self.arena);
            }
            s
        };
        self.shots.push(s);
        self.shots.len() - 1
    }

    /// Liegendes Objekt ohne Bewegung (Mine, Luftschlag-Markierung).
    fn place(&mut self, kind: Kind, owner: usize, x: f64, y: f64, r: f64, life: f64) -> usize {
        let id = self.new_id();
        self.shots.push(Shot::new(id, kind, owner, x, y, 0.0, 0.0, r, life, 0.0));
        self.shots.len() - 1
    }

    fn fx(&mut self, data: FxData, life: f64) {
        let id = self.new_id();
        self.fx.push(Fx { id, data, life, age: 0.0 });
    }

    fn feed(&mut self, by: Option<usize>, victim: usize, how: Kind) {
        let variant = self.rng.below(text::SELF_GENERIC.len()) as u8;
        self.feed.push(Feed { by, victim, how, variant, age: 0.0 });
        if self.feed.len() > FEED_MAX {
            let cut = self.feed.len() - FEED_MAX;
            self.feed.drain(..cut);
        }
    }

    /// Große Ansage in der Bildmitte (Doppelkill, Trickshot ...).
    fn announce(&mut self, ann: Ann, slot: usize) {
        self.fx(FxData::Ann { ann, slot: slot as u8 }, ANN_TIME);
    }

    fn slowmo(&mut self, length: f64) {
        if length > self.slow {
            self.slow = length;
            self.slow_len = length;
        }
    }

    /// Zeitfaktor der Zeitlupe: erst richtig langsam, dann weich zurück auf normal.
    fn time_scale(&self) -> f64 {
        if self.slow <= 0.0 {
            return 1.0;
        }
        let k = (self.slow_len - self.slow) / self.slow_len;
        if k < 0.4 {
            return SLOWMO_SCALE;
        }
        let k = (k - 0.4) / 0.6;
        SLOWMO_SCALE + (1.0 - SLOWMO_SCALE) * k * k * (3.0 - 2.0 * k)
    }

    /// Gültige Position für Panzer ti in der Nähe von (x, y).
    fn settle(&self, ti: usize, x: f64, y: f64) -> (f64, f64) {
        let t = &self.tanks[ti];
        let r = t.radius();
        if t.has(Eff::Ghost) {
            clamp_arena(x, y, r, &self.arena)
        } else {
            push_out(x, y, r, self.arena.near(x, y))
        }
    }

    fn settle_in_place(&mut self, ti: usize) {
        let (x, y) = (self.tanks[ti].x, self.tanks[ti].y);
        let (nx, ny) = self.settle(ti, x, y);
        self.tanks[ti].x = nx;
        self.tanks[ti].y = ny;
    }

    /// Trifft Panzer ti. pierce = Schild hilft nicht. True, wenn er zerstört wurde.
    /// angle = Flugrichtung des Treffers (dorthin fliegen die Trümmer).
    #[allow(clippy::too_many_arguments)]
    fn hit(&mut self, ti: usize, by: Option<usize>, how: Kind, pierce: bool, angle: Option<f64>, bounces: u32) -> bool {
        {
            let t = &self.tanks[ti];
            if !t.alive || t.has(Eff::Inv) || t.has(Eff::Spawn) {
                return false;
            }
            if t.has(Eff::Star) && !matches!(how, Kind::Hole | Kind::Nuke) {
                return false;
            }
        }
        let (x, y, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].slot);
        if self.tanks[ti].shield > 0.0 && !pierce {
            self.tanks[ti].shield = 0.0;
            self.fx(FxData::Block { x: qp(x, y).0, y: qp(x, y).1 }, 0.6);
            return false;
        }
        if self.tanks[ti].has(Eff::Extra) {
            // Ersatzteil rettet einmal, sogar vor durchschlagenden Treffern
            self.tanks[ti].eff.remove(Eff::Extra);
            self.tanks[ti].eff.set(Eff::Inv, 1.0);
            let (qx, qy) = qp(x, y);
            self.fx(FxData::Block { x: qx, y: qy }, 0.6);
            self.fx(FxData::Pop { x: qx, y: qy - 34, text: PopText::Spare, slot: slot as u8 }, 1.4);
            return false;
        }
        let t = &mut self.tanks[ti];
        t.alive = false;
        t.weapon = None;
        t.charge = 0.0;
        t.eff.clear(); // eine Kartoffel verschwindet mit
        let angle = angle.unwrap_or_else(|| self.rng.uniform(-PI, PI));
        let (qx, qy) = qp(x, y);
        self.fx(FxData::Boom { x: qx, y: qy, size: 125 }, 0.8);
        self.fx(FxData::Die { x: qx, y: qy, slot: slot as u8, angle: (angle * 100.0).round() as i16 }, 0.5);
        self.feed(by, slot, how);
        self.kill_event(ti, by, how, bounces);
        true
    }

    /// Ansagen: Eigentor, Mehrfachkill, Trickshot, Erstes Blut.
    fn kill_event(&mut self, ti: usize, by: Option<usize>, how: Kind, bounces: u32) {
        let (x, y, slot) = (self.tanks[ti].x, self.tanks[ti].y, self.tanks[ti].slot);
        let by = match by {
            Some(b) if b != slot => b,
            _ => {
                let (qx, qy) = qp(x, y);
                self.fx(FxData::Pop { x: qx, y: qy - 34, text: PopText::OwnGoal, slot: slot as u8 }, 1.6);
                return;
            }
        };
        let now = self.time;
        self.kills[by].push(now);
        let multi = self.kills[by].iter().filter(|&&k| now - k < MULTI_WINDOW).count();
        if multi >= 2 {
            self.announce(Ann::Multi(multi.min(4) as u8), by);
        } else if how.trick() && bounces >= TRICK_BOUNCES {
            self.announce(Ann::Trick(bounces.min(255) as u8), by);
        } else if !self.first_blood && self.tanks.len() > 2 {
            self.announce(Ann::FirstBlood, by);
        }
        self.first_blood = true;
    }

    /// (verfügbare Schuss, Sekunden bis wieder eine Kugel frei ist).
    pub fn reload_state(&self, ti: usize) -> (i32, f64) {
        let t = &self.tanks[ti];
        if let Some(w) = t.weapon {
            if w.has_ammo() {
                return (t.ammo, 0.0);
            }
        }
        let most = bullet_params(self.mutator).most * if t.has(Eff::Rapid) { 2 } else { 1 };
        let mut n = 0;
        let mut left = f64::INFINITY;
        for s in &self.shots {
            if s.owner == t.slot && s.kind == Kind::B {
                n += 1;
                left = left.min(s.life - s.age);
            }
        }
        let free = most as i32 - n;
        if free > 0 {
            return (free, 0.0);
        }
        // alle Kugeln unterwegs -> warten, bis die älteste verschwindet
        (0, left.max(0.0))
    }

    /// Nächster lebender, nicht getarnter Panzer außer owner in Reichweite, bei sight nur
    /// mit freier Sicht.
    fn nearest(&self, x: f64, y: f64, owner: Option<usize>, reach: f64, sight: bool) -> Option<usize> {
        let mut best = None;
        let mut best_d = reach;
        for (i, o) in self.tanks.iter().enumerate() {
            let d = hypot(o.x - x, o.y - y);
            if o.alive
                && Some(o.slot) != owner
                && !o.has(Eff::Invis)
                && d < best_d
                && (!sight || self.arena.clear(x, y, o.x, o.y))
            {
                best = Some(i);
                best_d = d;
            }
        }
        best
    }

    /// Zellen sind über die Runde gleich, nur Wände ändern sich
    pub fn alive_slots(&self) -> Vec<usize> {
        self.tanks.iter().filter(|t| t.alive).map(|t| t.slot).collect()
    }
}
