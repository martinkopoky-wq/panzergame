//! Aufnahme für die Waffen-Doku: `panzer showcase <waffe> <ordner>`.
//! Jede Waffe bekommt ein Drehbuch: wie die Panzer stehen, wann geschossen oder gedrückt
//! wird und wie der Gegner darauf reagiert. Jedes Bild wird als PNG gespeichert, daraus
//! macht scripts/encode_videos.py das Video.

use macroquad::prelude::*;
use panzer_core::consts::*;
use panzer_core::items::{Eff, Item};
use panzer_core::rng::Rng;
use panzer_core::Game;
use std::f64::consts::PI;

use crate::render::{Frame, Roster};
use crate::App;

/// Länge der Aufnahme in Sekunden (nach dem Vorlauf)
const DURATION: f64 = 5.0;
/// Vorlauf ohne Aufnahme, damit der Rundenbanner verschwindet
const WARMUP: f64 = 2.2;
/// freie Strecke, auf der die Szene spielt
const SPAN: f64 = 420.0;
/// Breite des gezeigten Ausschnitts in Welteinheiten
const VIEW_W: f32 = 460.0;

/// Ein Schritt im Drehbuch
#[derive(Clone, Copy)]
enum Act {
    /// Kiste mit dieser Waffe/Effekt auf den Panzer legen (wird sofort aufgenommen)
    Box(usize, Item),
    /// einmal abdrücken (eine Flanke)
    Fire(usize),
    /// Tasten ab jetzt: UP/DOWN/LEFT/RIGHT/SHOOT als Bitmaske
    Keys(usize, u8),
}

/// Aufstellung: Abstand vom Start der Linie und Blickrichtung
#[derive(Clone, Copy)]
struct Pos {
    dx: f64,
    /// seitlicher Versatz (Eckenschuss braucht einen Gegner abseits der Linie)
    dy: f64,
    a: f64,
}

struct Plan {
    me: Pos,
    foe: Pos,
    /// Waffe des Gegners zu Beginn (Entwaffnung)
    foe_weapon: Option<Item>,
    /// eigener Panzer ist unverwundbar, damit die Szene zu Ende läuft
    me_safe: bool,
    events: Vec<(f64, Act)>,
}

const FW: f64 = 0.0; // Blick nach rechts (+x)
const BW: f64 = PI; // Blick nach links (-x)

fn pos(dx: f64, a: f64) -> Pos {
    Pos { dx, dy: 0.0, a }
}

fn pos_y(dx: f64, dy: f64, a: f64) -> Pos {
    Pos { dx, dy, a }
}

/// Drehbuch zu einer Waffe oder einem Effekt
fn plan(item: Item) -> Plan {
    use Act::*;
    let box_me = (0.1, Box(0, item));
    // Schuss auf den stehenden Gegner
    let direct = |at: f64, dist: f64| Plan {
        me: pos(60.0, FW),
        foe: pos(60.0 + dist, BW),
        foe_weapon: None,
        me_safe: true,
        events: vec![box_me, (at, Fire(0))],
    };
    // Mine/Falle: Schütze legt sie hinter sich, der Gegner fährt von hinten hinein
    let laid = |at: f64, drive_at: f64, drive_for: f64| Plan {
        me: pos(300.0, FW),
        foe: pos(60.0, FW),
        foe_weapon: None,
        me_safe: true,
        events: vec![box_me, (at, Fire(0)), (drive_at, Keys(1, UP)), (drive_at + drive_for, Keys(1, 0))],
    };
    // Dauerfeuer: Feuer gedrückt halten, Gegner steht still
    let hold = |from: f64, to: f64, dist: f64| Plan {
        me: pos(60.0, FW),
        foe: pos(60.0 + dist, BW),
        foe_weapon: None,
        me_safe: true,
        events: vec![box_me, (from, Keys(0, SHOOT)), (to, Keys(0, 0))],
    };
    // Effekt aus der Kiste: Schütze und Gegner stehen sich gegenüber
    let buff = |events: Vec<(f64, Act)>| {
        let mut e = vec![box_me];
        e.extend(events);
        Plan { me: pos(60.0, FW), foe: pos(300.0, BW), foe_weapon: None, me_safe: false, events: e }
    };
    let fire_foe = |t: f64| (t, Fire(1));

    match item {
        // ---------- Waffen ----------
        Item::Frag => {
            let mut p = direct(0.5, 200.0);
            p.events.push((1.4, Fire(0))); // zweiter Druck zündet die Splitterbombe
            p
        }
        Item::Mg => hold(0.5, 2.2, 240.0),
        Item::Laser => direct(0.5, 280.0),
        Item::Rocket => {
            let mut p = direct(0.5, 260.0);
            p.events.push((0.7, Keys(1, UP))); // Gegner flieht, die Rakete sucht ihn
            p.events.push((1.9, Keys(1, 0)));
            p
        }
        Item::Shotgun => direct(0.5, 160.0),
        Item::Bouncy | Item::Split => {
            let mut p = direct(0.5, 240.0);
            p.me.a = BW; // erst in die nahe Wand schießen, damit die besondere Reaktion sichtbar wird
            p
        }
        Item::Mine => laid(0.5, 1.0, 2.5),
        Item::Swap => {
            let mut p = direct(0.5, 240.0);
            p.events.push((1.6, Fire(0))); // zweiter Druck: Teleport zur Kugel
            p
        }
        Item::Rang => direct(0.5, 220.0),
        Item::Ice => direct(0.5, 240.0),
        Item::Hole => Plan {
            me: pos(60.0, FW),
            foe: pos(230.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0))],
        },
        Item::Strike => Plan {
            me: pos(60.0, FW),
            foe: pos(340.0, FW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0))],
        },
        Item::Flame => hold(0.5, 2.4, 90.0),
        Item::Zap => direct(0.5, 200.0),
        Item::Rail => {
            let mut p = hold(0.5, 1.6, 260.0);
            p.events.push((1.6, Fire(0))); // Loslassen feuert
            p
        }
        Item::Rc => {
            let mut p = direct(0.5, 240.0);
            p.events.push((0.8, Keys(0, RIGHT)));
            p.events.push((1.6, Keys(0, 0)));
            p
        }
        Item::Sentry => laid(0.5, 1.0, 2.0),
        Item::Wall => laid(0.5, 1.0, 2.0),
        Item::Oil => laid(0.5, 1.0, 2.5),
        Item::Sticky => direct(0.5, 200.0),
        Item::Gl => direct(0.5, 260.0),
        Item::Swarm => direct(0.5, 200.0),
        Item::Saw => direct(0.5, 200.0),
        Item::Plasma => direct(0.5, 200.0),
        Item::Fw => direct(0.5, 200.0),
        Item::Push => direct(0.5, 110.0),
        Item::Spider => laid(0.5, 1.0, 2.5),
        Item::Ram => Plan {
            me: pos(60.0, FW),
            foe: pos(270.0, FW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0)), (0.5, Keys(0, UP)), (2.7, Keys(0, 0))],
        },
        Item::Nova => direct(0.5, 110.0),
        Item::Seek => direct(0.5, 200.0),
        Item::Phase => direct(0.5, 240.0),
        Item::Cannon => direct(0.5, 200.0),
        Item::Arty => direct(0.5, 200.0),
        Item::Trap => laid(0.5, 1.0, 2.5),
        Item::Jump => direct(0.5, 200.0),
        Item::Frost => direct(0.5, 100.0),
        Item::Orbit => direct(0.5, 240.0),
        Item::Prism => direct(0.5, 200.0),
        Item::Wave => direct(0.5, 200.0),
        Item::Timebomb => laid(0.5, 1.0, 2.5),
        Item::Napalm => direct(0.5, 200.0),
        Item::Fence => laid(0.5, 1.0, 2.5),
        Item::Sprinkler => laid(0.5, 1.0, 2.5),
        Item::Hook => direct(0.5, 200.0),
        Item::Destruct => direct(0.5, 100.0),
        Item::Shear => Plan {
            me: pos(60.0, FW),
            foe: pos(300.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![fire_foe(0.05), box_me, (0.5, Fire(0))],
        },
        Item::Ring => Plan {
            me: pos(40.0, FW),
            foe: pos(300.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0))],
        },
        Item::Dist => direct(0.3, 190.0),
        Item::Eater => Plan {
            me: pos(60.0, FW),
            foe: pos(300.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, fire_foe(0.0), fire_foe(0.05), fire_foe(0.1), fire_foe(0.15), (0.35, Fire(0))],
        },
        Item::Disarm => Plan {
            me: pos(60.0, FW),
            foe: pos(150.0, BW),
            foe_weapon: Some(Item::Rocket),
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0))],
        },
        // ---------- Effekte (aus Kisten) ----------
        Item::Shield => buff(vec![fire_foe(0.9)]),
        Item::Turbo => buff(vec![(0.3, Keys(0, UP)), (2.4, Keys(0, 0))]),
        Item::Mini => buff(vec![fire_foe(0.9)]),
        Item::Ghost => buff(vec![(0.3, Keys(0, UP)), (2.8, Keys(0, 0))]),
        Item::Invis => buff(vec![fire_foe(0.9)]),
        Item::Confuse => buff(vec![(0.4, Keys(1, UP)), (2.6, Keys(1, 0))]),
        Item::Mirror => buff(vec![fire_foe(0.9)]),
        Item::Star => buff(vec![(0.3, Keys(0, UP)), (2.4, Keys(0, 0))]),
        Item::Giant => buff(vec![(0.3, Keys(0, UP)), (2.4, Keys(0, 0))]),
        Item::Rapid => Plan {
            me: pos(60.0, FW),
            foe: pos(300.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.4, Keys(0, SHOOT)), (2.4, Keys(0, 0))],
        },
        Item::Extra => buff(vec![fire_foe(0.9)]),
        Item::Slow => buff(vec![(0.4, Keys(1, UP)), (2.6, Keys(1, 0))]),
        Item::Twin => Plan {
            me: pos(60.0, FW),
            foe: pos(300.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.4, Keys(0, SHOOT)), (1.2, Keys(0, 0))],
        },
        Item::Tp => buff(vec![]),
        Item::Disco => buff(vec![]),
        Item::Radar => buff(vec![]),
        // ---------- nur aus der Wundertüte ----------
        Item::Nuke => Plan {
            me: pos(60.0, FW),
            foe: pos(230.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0))],
        },
        Item::Gold => direct(0.5, 220.0),
        Item::Potato => Plan {
            me: pos(60.0, FW),
            foe: pos(230.0, BW),
            foe_weapon: None,
            me_safe: false,
            events: vec![box_me, (0.4, Keys(0, UP)), (1.6, Keys(0, 0))],
        },
        Item::Meteor => buff(vec![]),
        // ---------- Batch 2 ----------
        Item::Double => {
            let mut p = direct(0.5, 200.0);
            p.events.push((1.4, Fire(0))); // zweiter Druck: der Treffer zählt
            p
        }
        Item::Corner => Plan {
            me: pos(60.0, FW),
            foe: pos_y(170.0, 90.0, PI / 2.0),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0)), (0.9, Fire(0))],
        },
        Item::Chain => Plan {
            me: pos(60.0, FW),
            foe: pos(90.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0))],
        },
        Item::Hammer => Plan {
            me: pos(60.0, FW),
            foe: pos(90.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0))],
        },
        Item::Pause => Plan {
            me: pos(60.0, FW),
            foe: pos(240.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, fire_foe(0.1), (0.6, Fire(0))],
        },
        Item::Decoy => Plan {
            me: pos(60.0, FW),
            foe: pos(260.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0)), fire_foe(1.5)],
        },
        Item::Shadow => Plan {
            me: pos(60.0, FW),
            foe: pos(240.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0))],
        },
        Item::Twist => Plan {
            me: pos(60.0, FW),
            foe: pos(120.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0))],
        },
        // ---------- Batch 3 ----------
        Item::Switch => Plan {
            me: pos(60.0, FW),
            foe: pos(240.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0)), fire_foe(0.9)],
        },
        Item::Afterburn => direct(0.5, 180.0),
        Item::Thorn => Plan {
            me: pos(260.0, FW),
            foe: pos(340.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0)), (1.0, Keys(1, UP)), (2.5, Keys(1, 0))],
        },
        Item::Plate => {
            let mut e = vec![box_me, (0.5, Fire(0))];
            // dreimal hinein und wieder heraus
            for k in 0..3 {
                let t = 1.0 + k as f64 * 1.0;
                e.push((t, Keys(1, UP)));
                e.push((t + 0.5, Keys(1, DOWN)));
                e.push((t + 1.0, Keys(1, 0)));
            }
            Plan { me: pos(300.0, FW), foe: pos(200.0, FW), foe_weapon: None, me_safe: true, events: e }
        }
        Item::Pincer => Plan {
            me: pos(60.0, FW),
            foe: pos(300.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0))],
        },
        Item::Cross => direct(0.5, 180.0),
        Item::Pendel => Plan {
            me: pos(60.0, FW),
            foe: pos(150.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0))],
        },
        // ---------- Batch 4 ----------
        Item::Verdict => direct(0.5, 180.0),
        Item::Catcher => Plan {
            me: pos(300.0, FW),
            foe: pos(60.0, FW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0)), fire_foe(1.2)],
        },
        Item::Parry => Plan {
            me: pos(60.0, FW),
            foe: pos(112.0, BW),
            foe_weapon: None,
            me_safe: false,
            events: vec![box_me, fire_foe(0.5), (0.53, Fire(0))],
        },
        Item::Rust => Plan {
            me: pos(60.0, FW),
            foe: pos(240.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0)), fire_foe(1.2)],
        },
        Item::Cushion => Plan {
            me: pos(60.0, FW),
            foe: pos(300.0, BW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0)), (1.5, Keys(1, UP)), (2.5, Keys(1, 0))],
        },
        Item::Cracker => Plan {
            me: pos(300.0, FW),
            foe: pos(250.0, FW),
            foe_weapon: None,
            me_safe: true,
            events: vec![box_me, (0.5, Fire(0)), (0.8, Keys(1, UP)), (0.95, Keys(1, 0))],
        },
        _ => panic!("kein Drehbuch für {item:?}"),
    }
}

/// Erster Platz, an dem die Linie `SPAN` frei ist. Liefert den Startpunkt (links).
fn find_line(game: &Game) -> Option<(f64, f64)> {
    game.arena
        .cells()
        .into_iter()
        .map(|c| game.arena.center(c))
        .find(|&(x, y)| game.arena.clear(x - 60.0, y, x - 60.0 + SPAN, y))
        .map(|(x, y)| (x - 60.0, y))
}

/// Spiel mit der Aufstellung aus dem Drehbuch. Liefert Spiel und Startpunkt der Linie.
fn setup(p: &Plan, item: Item) -> (Game, f64, f64) {
    let mut seed = 1u64;
    loop {
        let mut game = Game::with_rng(&[0, 1], Rng::new(seed));
        for t in game.tanks.iter_mut() {
            t.eff.remove(Eff::Spawn);
        }
        game.box_timer = 1e9;
        game.mutator = None; // keine Sonderregel (Stromausfall würde die Szene verdunkeln)
        game.maze.mutator = None; // auch nach Wandbau/Plasma bleibt die neu gebaute Arena ohne Sonderregel
        game.arena.mutator = None;
        if let Some((mut sx, sy)) = find_line(&game) {
            if matches!(item, Item::Bouncy | Item::Split) {
                let d = panzer_core::physics::first_wall(&game.arena, sx + p.me.dx, sy, BW, game.arena.ww);
                sx -= (d - 55.0).max(0.0);
            }
            let me = &mut game.tanks[0];
            me.x = sx + p.me.dx;
            me.y = sy + p.me.dy;
            me.a = p.me.a;
            if p.me_safe {
                me.eff.set(Eff::Inv, 1e6);
            }
            let foe = &mut game.tanks[1];
            foe.x = sx + p.foe.dx;
            foe.y = sy + p.foe.dy;
            foe.a = p.foe.a;
            foe.weapon = p.foe_weapon;
            foe.ammo = p.foe_weapon.map_or(0, |w| w.ammo());
            return (game, sx, sy);
        }
        seed += 1;
        assert!(seed < 500, "keine passende Linie gefunden");
    }
}

/// Tasten und Schussnummern pro Panzer, damit jeder Schuss genau einmal zählt
struct Input {
    keys: [u8; 2],
    fires: [u16; 2],
}

impl Input {
    fn new(game: &Game) -> Input {
        let seen = |s: usize| game.tanks[s].fire_seen.unwrap_or(0);
        Input { keys: [0; 2], fires: [seen(0), seen(1)] }
    }

    fn apply(&mut self, game: &mut Game, a: Act) {
        match a {
            Act::Box(slot, item) => {
                let (x, y) = (game.tanks[slot].x, game.tanks[slot].y);
                game.boxes.push((x, y, item));
            }
            Act::Fire(slot) => self.fires[slot] = self.fires[slot].wrapping_add(1),
            Act::Keys(slot, k) => self.keys[slot] = k,
        }
    }

    /// jeder Tick: Tasten und Schussnummer an die Panzer geben
    fn tick(&self, game: &mut Game) {
        for slot in 0..2 {
            game.apply_input(slot, self.keys[slot], self.fires[slot], TICK);
        }
    }
}

/// Die Aufnahme ist eine einzelne Szene: ein Abschuss darf keine neue Runde starten.
fn advance(game: &mut Game, input: &Input) {
    game.end_timer = Some(1e6);
    game.slow = 0.0;
    input.tick(game);
    game.step(TICK);
}

/// Nimmt die Szene auf. Jedes Bild wird als PNG in `out` gespeichert.
pub async fn run(app: &mut App, item: Item, out: &str) {
    let p = plan(item);
    let (mut game, sx, sy) = setup(&p, item);
    let mut input = Input::new(&game);

    // Ausschnitt: VIEW_W breit, auf die Mitte der Szene zentriert, Seitenverhältnis wie das Labyrinth
    let (ww, wh) = (game.arena.ww as f32, game.arena.wh as f32);
    let h = VIEW_W * wh / ww;
    let focus_x = (sx + (p.me.dx + p.foe.dx) / 2.0) as f32;
    let x = (focus_x - VIEW_W / 2.0).clamp(0.0, (ww - VIEW_W).max(0.0));
    let y = (sy as f32 - h / 2.0).clamp(0.0, (wh - h).max(0.0));
    app.r.reset();
    app.r.crop = Some(Rect::new(x, y, VIEW_W, h));

    let warm = (WARMUP / TICK).round() as usize;
    let frames = (DURATION / TICK).round() as usize;
    let mut events = p.events.clone();
    events.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut next = 0;
    // Für Sichtprüfungen nur jedes n-te PNG schreiben; Simulation und Animation bleiben bei 60 Hz.
    let stride =
        std::env::var("PANZER_SHOWCASE_STRIDE").ok().and_then(|s| s.parse::<usize>().ok()).unwrap_or(1).clamp(1, 60);
    for f in 0..warm + frames {
        let now = (f as f64 - warm as f64) * TICK;
        while next < events.len() && now + 1e-9 >= events[next].0 {
            input.apply(&mut game, events[next].1);
            next += 1;
        }
        advance(&mut game, &input);
        app.begin();
        let view = game.net_snap(None, false).view();
        let roster = Roster::local();
        let frame = Frame { arena: &game.arena, view: &view, me: None, roster: &roster, quiet: true, hud: true };
        app.r.scene_time = Some(f as f64 * TICK);
        app.r.draw(&mut app.g, &frame);
        if f >= warm && (f - warm) % stride == 0 {
            let mut img = get_screen_data();
            for px in img.bytes.chunks_mut(4) {
                px[3] = 255;
            }
            img.export_png(&format!("{out}/{:04}.png", (f - warm) / stride));
        }
        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use panzer_core::items::{Kind, ALL_ITEMS};

    fn simulate(item: Item, mut observe: impl FnMut(&Game)) -> Game {
        let mut p = plan(item);
        let (mut game, _, _) = setup(&p, item);
        let mut input = Input::new(&game);
        p.events.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut next = 0;
        for f in 0..((WARMUP + DURATION) / TICK).round() as usize {
            let now = f as f64 * TICK - WARMUP;
            while next < p.events.len() && now + 1e-9 >= p.events[next].0 {
                input.apply(&mut game, p.events[next].1);
                next += 1;
            }
            advance(&mut game, &input);
            observe(&game);
        }
        game
    }

    #[test]
    fn all_weapon_scenes_stay_in_one_round_without_random_rules() {
        for &item in ALL_ITEMS.iter().filter(|w| w.is_weapon()) {
            let mut id = None;
            simulate(item, |game| {
                assert_eq!(*id.get_or_insert(game.maze_id), game.maze_id, "{item:?}: round restarted");
                assert!(game.arena.mutator.is_none(), "{item:?}: unrelated special rule");
                assert!(game.win.is_none(), "{item:?}: winner banner covers the demo");
                assert_eq!(game.ts, 1.0, "{item:?}: unexpected slow motion");
            });
        }
    }

    #[test]
    fn bounce_scenes_show_the_mechanic_before_the_target() {
        let mut bounced = false;
        simulate(Item::Bouncy, |game| {
            bounced |= game.shots.iter().any(|s| s.kind == Kind::Bnc && s.bounces > 0);
        });
        assert!(bounced, "Flummi demo must show a wall bounce");
        let mut split = false;
        simulate(Item::Split, |game| {
            split |= game.shots.iter().filter(|s| s.kind == Kind::Split).count() > 1;
        });
        assert!(split, "Querschläger demo must show splitting at a wall");
    }

    #[test]
    fn close_range_demos_actually_connect() {
        let mut failures = Vec::new();
        for item in [Item::Ram, Item::Parry, Item::Decoy, Item::Cracker] {
            let game = simulate(item, |_| {});
            if !game.tanks[0].alive || game.tanks[1].alive {
                failures.push(format!(
                    "{item:?}: attacker alive={}, victim alive={} (x={:.1})",
                    game.tanks[0].alive, game.tanks[1].alive, game.tanks[1].x
                ));
            }
        }
        assert!(failures.is_empty(), "Demos did not connect: {}", failures.join("; "));
    }
}
