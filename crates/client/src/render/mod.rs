//! Renderer: zeichnet einen View (Snapshot) mit Licht, Partikeln, Wackeln und HUD.
//! Alle Effekte entstehen hier aus den Snapshots, jeder Client berechnet sie selbst.

mod events;
mod hud;
mod maze;
mod world;

use std::collections::{HashMap, HashSet};

use macroquad::prelude::*;
use panzer_core::consts::*;
use panzer_core::geom::Wall;
use panzer_core::items::Item;
use panzer_core::physics::Effects;
use panzer_core::snap::{TankView, View};
use panzer_core::Arena;

use crate::audio::{Music, Sfx};
use crate::gfx::*;
use crate::particles::Particles;
use crate::raster::Canvas;
use maze::MazeLayer;

/// Namen der Spieler (online vom Server, lokal die Farbnamen)
#[derive(Clone)]
pub struct Roster {
    pub names: [String; MAX_PLAYERS],
    pub ping: [u16; MAX_PLAYERS],
    pub admin: [bool; MAX_PLAYERS],
    pub bots: u8,
}

impl Roster {
    pub fn local() -> Roster {
        Roster {
            names: COLOR_NAMES.map(|s| s.to_string()),
            ping: [0; MAX_PLAYERS],
            admin: [false; MAX_PLAYERS],
            bots: 0,
        }
    }

    pub fn name(&self, slot: usize) -> String {
        self.names.get(slot).cloned().unwrap_or_default()
    }
}

/// Was pro Bild gezeichnet wird
pub struct Frame<'a> {
    pub arena: &'a Arena,
    pub view: &'a View,
    /// eigener Slot (None = lokal mit mehreren Spielern)
    pub me: Option<usize>,
    pub roster: &'a Roster,
    /// Hintergrund im Menü: ohne Ton, Wackeln und HUD
    pub quiet: bool,
    pub hud: bool,
}

/// abgesprengter Turm in der Luft
struct Flying {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    a: f32,
    spin: f32,
    slot: usize,
}

/// Umgebungspartikel (Staub, Schnee, Glühwürmchen ...)
struct Amb {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    age: f32,
    life: f32,
    ph: f32,
}

const TURRET_AIR: f32 = 0.8;

pub struct Renderer {
    pub sfx: Sfx,
    pub music: Music,
    pub debug: bool,
    parts: Particles,
    maze: Option<MazeLayer>,
    round_id: Option<u32>,
    round_walls: Vec<Wall>,
    round_start: f64,
    seen_shots: HashSet<u32>,
    seen_fx: HashSet<u32>,
    tracks: HashMap<usize, (f32, f32, f32)>,
    taunts: HashMap<usize, u8>,
    last_win: bool,
    trauma: f32,
    flash: f32,
    last_frame: f64,
    hist: HashMap<u32, Vec<(f32, f32, f64)>>,
    after: HashMap<usize, Vec<(f32, f32, f32, f64)>>,
    prev: HashMap<usize, TankView>,
    flying: Vec<Flying>,
    box_seen: HashMap<(i32, i32, Item), f64>,
    landed: HashSet<(i32, i32, Item)>,
    marks: HashSet<(u32, u8)>,
    scores_seen: [Option<u32>; MAX_PLAYERS],
    score_pop: [Option<f64>; MAX_PLAYERS],
    amb: Vec<Amb>,
    amb_id: Option<u32>,
    me_pos: Option<(f32, f32)>,
    world_w: f32,
    quiet: bool,
    slow_seen: bool,
    focus: Option<(f32, f32)>,
    /// Showcase: fester Ausschnitt der Welt (x, y, Breite, Höhe), sonst das ganze Labyrinth
    pub crop: Option<Rect>,
    cine: f32,
    my_eff: Effects,
    me: Option<usize>,
    jets: Vec<(f32, f32, f32)>,
    scorch: HashMap<usize, Canvas>,
    turret_cv: Vec<Canvas>,
    fog: Option<RenderTarget>,
    lightmap: Option<RenderTarget>,
}

impl Renderer {
    pub async fn new() -> Renderer {
        Renderer {
            sfx: Sfx::new().await,
            music: Music::new().await,
            debug: false,
            parts: Particles::new(),
            maze: None,
            round_id: None,
            round_walls: Vec::new(),
            round_start: 0.0,
            seen_shots: HashSet::new(),
            seen_fx: HashSet::new(),
            tracks: HashMap::new(),
            taunts: HashMap::new(),
            last_win: false,
            trauma: 0.0,
            flash: 0.0,
            last_frame: get_time(),
            hist: HashMap::new(),
            after: HashMap::new(),
            prev: HashMap::new(),
            flying: Vec::new(),
            box_seen: HashMap::new(),
            landed: HashSet::new(),
            marks: HashSet::new(),
            scores_seen: [None; MAX_PLAYERS],
            score_pop: [None; MAX_PLAYERS],
            amb: Vec::new(),
            amb_id: None,
            me_pos: None,
            world_w: W as f32,
            quiet: false,
            slow_seen: false,
            focus: None,
            crop: None,
            cine: 0.0,
            my_eff: Effects::default(),
            me: None,
            jets: Vec::new(),
            scorch: HashMap::new(),
            turret_cv: COLORS
                .iter()
                .map(|&c| crate::sprites::turret_canvas(crate::raster::mix(c, [34, 28, 26], 0.7), 1.0))
                .collect(),
            fog: None,
            lightmap: None,
        }
    }

    /// Neues Spiel (ein neues Game fängt wieder bei Runde 1 an)
    pub fn reset(&mut self) {
        self.maze = None;
        self.round_id = None;
        self.scores_seen = [None; MAX_PLAYERS];
        self.score_pop = [None; MAX_PLAYERS];
        self.trauma = 0.0;
        self.flash = 0.0;
    }

    fn sound(&mut self, name: &'static str, x: Option<f32>, vol: f32) {
        if !self.quiet {
            self.sfx.play(name, x.map(|x| (x / self.world_w) as f64), vol as f64);
        }
    }

    /// Wackeln. Im Netzspiel schwächer, je weiter weg vom eigenen Panzer.
    fn shake(&mut self, mut amount: f32, at: Option<(f32, f32)>) {
        if self.quiet {
            return;
        }
        if let (Some(me), Some((x, y))) = (self.me_pos, at) {
            let d = (x - me.0).hypot(y - me.1);
            amount *= (1.0 - d / 750.0).max(0.35);
        }
        self.trauma = (self.trauma + amount).min(1.0);
    }

    fn new_round(&mut self, arena: &Arena, now: f64) {
        self.round_id = Some(arena.id);
        self.round_walls = arena.walls.clone();
        self.round_start = now;
        self.seen_shots.clear();
        self.seen_fx.clear();
        self.tracks.clear();
        self.hist.clear();
        self.after.clear();
        self.prev.clear();
        self.box_seen.clear();
        self.landed.clear();
        self.marks.clear();
        self.parts.items.clear();
        self.flying.clear();
        self.slow_seen = false;
        self.focus = None;
        self.maze = Some(MazeLayer::new(arena));
        self.sound("round", None, 1.0);
    }

    /// Mitten in der Runde sind Wände weggesprengt oder gebaut worden
    fn walls_changed(&mut self, arena: &Arena) {
        let old: Vec<Wall> = std::mem::take(&mut self.round_walls);
        if let Some(m) = self.maze.as_mut() {
            m.walls_changed(&old, arena);
        }
        let th = arena.theme();
        for w in old.iter().filter(|w| !arena.walls.contains(w)) {
            let (x, y, ww, wh) = (w[0] as f32, w[1] as f32, w[2] as f32, w[3] as f32);
            for _ in 0..(((ww + wh) / 25.0) as usize).max(2) {
                let a = self.parts.u(0.0, std::f32::consts::TAU);
                let v = self.parts.u(60.0, 240.0);
                let (px, py) = (x + self.parts.u(0.0, ww), y + self.parts.u(0.0, wh));
                let life = self.parts.u(0.8, 1.6);
                let size = self.parts.u(3.0, 6.0);
                let c = crate::raster::mix(th.wall, th.hi, self.parts.u(0.0, 0.5));
                let spin = self.parts.u(-10.0, 10.0);
                self.parts.add(
                    crate::particles::P::Debris,
                    px,
                    py,
                    a.cos() * v,
                    a.sin() * v,
                    life,
                    size,
                    crate::particles::o(c).drag(3.0).spin(spin),
                );
            }
            self.parts.burst(
                crate::particles::P::Dust,
                x + ww / 2.0,
                y + wh / 2.0,
                5.0,
                80.0,
                1.0,
                9.0,
                crate::particles::o(th.dust).drag(2.0),
            );
        }
        self.round_walls = arena.walls.clone();
    }

    /// Ein Bild der Spielwelt plus HUD. Muss mit der Bildschirmkamera aufgerufen werden.
    pub fn draw(&mut self, g: &mut Gfx, f: &Frame) {
        let now = get_time();
        let dt = ((now - self.last_frame) as f32).clamp(0.0, 0.05);
        self.last_frame = now;
        let (arena, view) = (f.arena, f.view);
        self.quiet = f.quiet;
        self.me = f.me;
        let live = view.maze_id == arena.id;
        let ts = if live { view.ts as f32 } else { 1.0 };
        let vdt = dt * ts;
        if self.round_id != Some(arena.id) || self.maze.is_none() {
            self.new_round(arena, now);
        } else if self.maze.as_ref().is_some_and(|m| m.rev != arena.rev) {
            self.walls_changed(arena);
        }
        g.dark = live && arena.mutator == Some(Mutator::Dunkel);
        g.light_log.clear();
        self.world_w = arena.ww as f32;
        if live {
            let mine = f.me.and_then(|m| view.tank(m));
            self.me_pos = mine.filter(|t| !t.hidden).map(|t| (t.x as f32, t.y as f32));
            self.my_eff = mine.filter(|t| t.alive).map(|t| t.eff).unwrap_or_default();
            self.stamp_tracks(view, arena);
            self.events(view, f, now);
            if f.quiet {
                self.flash = 0.0;
                self.trauma = 0.0;
            }
            self.track_shots(view, now);
            self.emit(view, arena, vdt, now);
        }
        let landed = self.parts.update(vdt, Some(arena));
        for p in landed {
            // liegen gebliebene Trümmer
            let pts = rot_rect(p.x, p.y, p.size, p.size * 0.65, p.angle);
            if let Some(m) = self.maze.as_mut() {
                m.poly(&pts, crate::raster::shade(p.color, 0.55));
            }
        }
        self.update_flying(vdt, arena);
        if let Some(m) = self.maze.as_mut() {
            m.upload();
        }

        // --- Welt ---
        let lay = Layout::now();
        self.trauma = (self.trauma - dt * 1.5).max(0.0);
        let s = self.trauma.powi(2) * 15.0;
        let t = now as f32;
        let shake = vec2(
            s * ((t * 53.1).sin() * 0.6 + (t * 91.7 + 1.3).sin() * 0.4),
            s * ((t * 61.3 + 2.1).sin() * 0.6 + (t * 83.9 + 0.7).sin() * 0.4),
        );
        let (ww, wh) = (arena.ww as f32, arena.wh as f32);
        let zoom = if self.focus.is_some() && !f.quiet { 1.0 + 0.16 * self.cine } else { 1.0 };
        let mut src = Rect::new(0.0, 0.0, ww / zoom, wh / zoom);
        if let Some(c) = self.crop {
            src = c;
        }
        if let Some((fx, fy)) = self.focus.filter(|_| zoom > 1.002) {
            src.x = (fx - src.w / 2.0).clamp(0.0, ww - src.w);
            src.y = (fy - src.h / 2.0).clamp(0.0, wh - src.h);
        }
        set_camera(&lay.world_camera(src, shake));
        if let Some(m) = &self.maze {
            draw_texture_ex(
                &m.tex,
                0.0,
                0.0,
                WHITE,
                DrawTextureParams { dest_size: Some(vec2(ww, wh)), ..Default::default() },
            );
        }
        let (low, polys_low, high, polys_high) = self.collect(g);
        for (x, y, r, c, a) in low {
            g.puff(x, y, r, c, a);
        }
        for (c, pts) in polys_low {
            poly(&pts, col(c));
        }
        self.draw_ambient(g, arena, true);
        if live {
            let leader = leader(view);
            self.draw_boxes(g, view, now);
            self.draw_shots(g, view, arena, now);
            let mut order: Vec<&TankView> = view.tanks.iter().collect();
            order.sort_by_key(|t| t.alive); // Wracks zuerst
            for tk in order {
                self.draw_tank(g, tk, now);
            }
            self.draw_fx(g, view, now);
            self.draw_flying(g);
            for (jx, jy, ang) in std::mem::take(&mut self.jets) {
                let jet = &g.spr.jet;
                let size = jet.size() / 2.0;
                draw_texture_ex(
                    jet,
                    jx + 34.0 - size.x / 2.0,
                    jy + 52.0 - size.y / 2.0,
                    Color::new(0.0, 0.0, 0.0, 0.4),
                    DrawTextureParams { dest_size: Some(size), rotation: ang, ..Default::default() },
                );
                draw_texture_ex(
                    jet,
                    jx - size.x / 2.0,
                    jy - size.y / 2.0,
                    WHITE,
                    DrawTextureParams { dest_size: Some(size), rotation: ang, ..Default::default() },
                );
                g.light([255, 150, 60], jx - ang.cos() * 33.0, jy - ang.sin() * 33.0, 18.0, 7.0);
            }
            for (x, y, r, c, a) in high {
                g.puff(x, y, r, c, a);
            }
            for (c, pts) in polys_high {
                poly(&pts, col(c));
            }
            self.draw_ambient(g, arena, false);
            g.flush_light();
            for tk in &view.tanks {
                self.draw_tank_ui(g, tk, arena, view, f.roster, leader, now);
            }
            self.draw_fx_ui(g, view);
            g.flush_light(); // Lichter der Anzeigen (Krone, Kartoffel)
            if arena.mutator == Some(Mutator::Nebel) {
                self.draw_fog(g, arena, view);
            } else if g.dark {
                self.darkness(g, arena, view, now, lay, src, shake);
            }
        } else {
            self.draw_ambient(g, arena, false);
            g.flush_light();
        }
        // Zeitlupe: Kinobalken und Zoom auf den letzten Abschuss, weich nachgeführt
        let target = if live && !f.quiet { ((1.0 - ts) / (1.0 - SLOWMO_SCALE as f32)).min(1.0) } else { 0.0 };
        self.cine += (target - self.cine) * (dt * 7.0).min(1.0);
        self.music.update(self.sfx.on, if f.quiet { 1.0 } else { ts as f64 });

        // --- Bildschirm ---
        set_camera(&lay.screen_camera());
        self.overlays(g, if live { Some(view) } else { None }, f.me, now, dt);
        if live && f.hud {
            self.draw_banners(g, view, arena, f.roster, now);
            self.draw_feed(g, view, f.roster);
            self.hud(g, view, f.me, arena, f.roster, now);
        }
        self.draw_transition(if live { Some(view) } else { None }, arena, now);
        if f.hud {
            g.fancy(&format!("Runde {}", arena.id), 40.0, 9.0, 16.0, DIM_C, 2.0, false, 1.0, 1.0);
        }
    }
}

/// Wer hat allein die meisten Punkte? (bekommt die Krone)
fn leader(view: &View) -> Option<usize> {
    let scores: Vec<(usize, u32)> = view.tanks.iter().map(|t| (t.slot, view.scores[t.slot])).collect();
    let best = scores.iter().map(|s| s.1).max().unwrap_or(0);
    if best > 0 && scores.iter().filter(|s| s.1 == best).count() == 1 {
        scores.iter().find(|s| s.1 == best).map(|s| s.0)
    } else {
        None
    }
}
