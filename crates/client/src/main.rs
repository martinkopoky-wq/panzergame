//! Panzer – Labyrinth-Panzerschlacht mit Multiplayer über UDP, Bots und ordentlich Krawall.
//!
//!     panzer                          Menü
//!     panzer host [port] [bots]       Spiel hosten (Standard-Port 5555), freie Plätze mit Bots
//!     panzer join <ip[:port]>         Einem Spiel beitreten
//!     panzer local [2|3|4] [bots]     2 bis 4 Spieler an einer Tastatur, dazu Bots
//!     panzer solo [bots]              Allein gegen Bots (Standard: 3)
//!
//! Der Server ist autoritativ (auch beim Hosten läuft er als eigener Thread), die Clients
//! schicken nur Tasten. Siehe crates/server für den dedizierten Server.

// Windows: kein Konsolenfenster neben dem Spiel (nur im Release-Build)
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod audio;
mod config;
mod gfx;
mod input;
mod maze_art;
mod modes;
mod net;
mod particles;
mod raster;
mod render;
mod showcase;
mod sprites;
mod ui;

use macroquad::prelude::*;
use panzer_core::bot::Bot;
use panzer_core::consts::*;
use panzer_core::Game;

use crate::config::Config;
use crate::gfx::*;
use crate::render::{Frame, Renderer, Roster};

pub struct App {
    pub g: Gfx,
    pub r: Renderer,
    pub cfg: Config,
    pub demo: Option<Demo>,
    pub fullscreen: bool,
    pub frames: u64,
}

/// Bots, die im Hintergrund von Menü und Wartebildschirmen gegeneinander kämpfen
pub struct Demo {
    game: Game,
    bots: Vec<Bot>,
    acc: f64,
}

impl Demo {
    fn new() -> Demo {
        let n = 3 + (get_time() as usize % 2);
        let slots: Vec<usize> = (0..n).collect();
        let mut game = Game::new(&slots);
        game.bots = slots.clone();
        Demo { game, bots: slots.iter().map(|&s| Bot::new(s, Some(0.85))).collect(), acc: 0.0 }
    }

    fn step(&mut self, dt: f64) {
        self.acc = (self.acc + dt).min(0.1);
        while self.acc >= TICK {
            self.acc -= TICK;
            for b in self.bots.iter_mut() {
                let (k, f) = b.think(&self.game);
                self.game.apply_input(b.slot, k, f, TICK);
            }
            self.game.step(TICK);
        }
    }
}

pub enum Action {
    Host { port: u16, bots: usize },
    Join(String),
    Local { players: usize, bots: usize },
}

impl App {
    /// Tasten, die überall gelten. true = Esc (zurück)
    pub fn common_keys(&mut self) -> bool {
        if is_key_pressed(KeyCode::F11) {
            self.fullscreen = !self.fullscreen;
            set_fullscreen(self.fullscreen);
        }
        if is_key_pressed(KeyCode::F3) {
            self.r.debug = !self.r.debug;
        }
        if is_key_pressed(KeyCode::F2) {
            self.r.sfx.on = !self.r.sfx.on;
            self.cfg.sfx = self.r.sfx.on;
            self.cfg.save();
        }
        if is_key_pressed(KeyCode::F4) {
            self.r.music.on = !self.r.music.on;
            self.cfg.music = self.r.music.on;
            self.cfg.save();
        }
        is_key_pressed(KeyCode::Escape)
    }

    /// Hintergrund ohne eigenes Spiel: Bots kämpfen, darüber ein dunkler Schleier
    pub fn backdrop(&mut self) {
        if self.demo.as_ref().is_none_or(|d| d.game.maze_id > 40) {
            self.demo = Some(Demo::new());
            self.r.reset();
        }
        let dt = get_frame_time() as f64;
        let Some(demo) = self.demo.as_mut() else { return };
        demo.step(dt);
        let view = demo.game.net_snap(None, false).view();
        let roster = Roster::local();
        let frame = Frame { arena: &demo.game.arena, view: &view, me: None, roster: &roster, quiet: true, hud: false };
        self.r.draw(&mut self.g, &frame);
        draw_rectangle(0.0, 0.0, VW, VH, Color::new(7.0 / 255.0, 8.0 / 255.0, 12.0 / 255.0, 150.0 / 255.0));
    }

    /// Ende jedes Bildes: für Tests (PANZER_SHOT=datei.png, PANZER_SHOT_FRAMES=n)
    /// nach n Bildern einen Screenshot speichern und beenden
    pub fn shot(&mut self) {
        self.frames += 1;
        let Ok(path) = std::env::var("PANZER_SHOT") else { return };
        let n: u64 = std::env::var("PANZER_SHOT_FRAMES").ok().and_then(|v| v.parse().ok()).unwrap_or(120);
        if self.frames >= n {
            let mut img = get_screen_data();
            for px in img.bytes.chunks_mut(4) {
                px[3] = 255; // Alpha des Framebuffers ist bedeutungslos
            }
            img.export_png(&path);
            std::process::exit(0);
        }
    }

    /// Beginn jedes Bildes
    pub fn begin(&self) {
        clear_background(BLACK);
        set_camera(&Layout::now().screen_camera());
        draw_rectangle(0.0, 0.0, VW, VH, col(BG));
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "Panzer".to_owned(),
        window_width: W as i32,
        window_height: H as i32,
        window_resizable: true,
        sample_count: 4,
        high_dpi: false,
        ..Default::default()
    }
}

fn num(args: &[String], i: usize, default: usize) -> usize {
    args.get(i).and_then(|a| a.parse().ok()).unwrap_or(default)
}

#[macroquad::main(window_conf)]
async fn main() {
    prevent_quit();
    let cfg = Config::load();
    // Ladebildschirm, während die Sprites und Klänge entstehen
    clear_background(col(BG));
    next_frame().await;
    let g = Gfx::new();
    let mut r = Renderer::new().await;
    r.sfx.on = cfg.sfx;
    r.music.on = cfg.music;
    let mut app = App { g, r, cfg, demo: None, fullscreen: false, frames: 0 };

    let args: Vec<String> = std::env::args().skip(1).collect();
    // Aufnahme für die Doku: panzer showcase <waffe> <ordner>
    if args.len() == 3 && args[0] == "showcase" {
        let item = panzer_core::items::ALL_ITEMS.iter().copied().find(|i| format!("{i:?}").eq_ignore_ascii_case(&args[1]));
        let Some(item) = item else { panic!("unbekannte Waffe: {}", args[1]) };
        showcase::run(&mut app, item, &args[2]).await;
        app.cfg.save();
        return;
    }
    let mut action = match args.first().map(|s| s.as_str()) {
        Some("host") => Some(Action::Host {
            port: num(&args, 1, PORT as usize) as u16,
            bots: num(&args, 2, 0).min(MAX_PLAYERS - 1),
        }),
        Some("join") if args.len() > 1 => Some(Action::Join(args[1].clone())),
        Some("local") => {
            let n = num(&args, 1, 2).clamp(2, MAX_LOCAL);
            Some(Action::Local { players: n, bots: num(&args, 2, 0).min(MAX_PLAYERS - n) })
        }
        Some("solo") => Some(Action::Local { players: 1, bots: num(&args, 1, 3).clamp(1, MAX_PLAYERS - 1) }),
        _ => None,
    };
    // Host/Join von der Kommandozeile brauchen auch einen Namen
    if matches!(action, Some(Action::Host { .. }) | Some(Action::Join(_)))
        && panzer_core::text::clean_name(&app.cfg.name).is_none()
    {
        action = None;
    }
    loop {
        let a = match action.take() {
            Some(a) => a,
            None => match modes::menu(&mut app).await {
                Some(a) => a,
                None => break,
            },
        };
        app.r.reset();
        let quit = match a {
            Action::Local { players, bots } => modes::run_local(&mut app, players, bots).await,
            Action::Host { port, bots } => modes::run_host(&mut app, port, bots).await,
            Action::Join(addr) => modes::run_join(&mut app, &addr).await,
        };
        app.r.reset();
        app.demo = None;
        if quit {
            break;
        }
    }
    app.cfg.save();
}
