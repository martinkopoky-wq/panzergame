//! Spielmodi (lokal, hosten, beitreten) und das Menü.

use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use macroquad::prelude::*;
use panzer_core::bot::Bot;
use panzer_core::consts::*;
use panzer_core::text::clean_name;
use panzer_core::Game;
use panzer_server::{Config as ServerConfig, Server};

use crate::gfx::*;
use crate::input::KeyInput;
use crate::net::{NetClient, NetState};
use crate::render::{Frame, Roster};
use crate::theme;
use crate::ui::{button, drain_chars, primary_button, scoreboard, Chat, Field};
use crate::{Action, App};

const NET_HINT: &str = "Pfeile/WASD fahren  ·  Leertaste schießen  ·  1–7 Sprüche  ·  T Chat  ·  Tab Spieler  ·  F2 Ton  ·  F4 Musik  ·  F3 Ping  ·  Esc Menü";

/// IP des Interfaces mit Default-Route (es wird nichts gesendet)
fn local_ip() -> String {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".into())
}

fn hint(g: &Gfx, text: &str) {
    g.text(text, VW / 2.0, VH - 11.0, 13.0, col(theme::SECONDARY), 0.5);
}

/// players an einer Tastatur (1 = allein gegen Bots), dazu bots Computergegner.
/// true = Fenster wurde geschlossen.
pub async fn run_local(app: &mut App, players: usize, bots: usize) -> bool {
    let n = (players + bots).min(MAX_PLAYERS).max(2);
    let slots: Vec<usize> = (0..n).collect();
    let mut game = Game::with_rule(&slots, app.cfg.rule);
    // nur für Tests: bestimmte Sonderregel erzwingen (PANZER_TEST_MUT=nebel|dunkel|...)
    if let Ok(want) = std::env::var("PANZER_TEST_MUT") {
        for _ in 0..500 {
            if game.mutator.is_some_and(|m| m.name().to_lowercase().contains(&want)) {
                break;
            }
            game.new_round();
        }
    }
    let mut robots: Vec<Bot> = (players.min(n)..n).map(|s| Bot::new(s, None)).collect();
    game.bots = robots.iter().map(|b| b.slot).collect();
    let mut inputs = if players == 1 { vec![KeyInput::net()] } else { KeyInput::local(players) };
    let me = if players == 1 { Some(0) } else { None };
    let text = if players == 1 {
        "Pfeile/WASD fahren  ·  Leertaste schießen  ·  1–7 Sprüche  ·  F2 Ton  ·  F4 Musik  ·  F11 Vollbild  ·  Esc Menü".to_string()
    } else {
        let mut parts: Vec<&str> = ["Rot: ESDF + Q", "Grün: Pfeile + M", "Blau: IJKL + U", "Orange: Numpad 8456 + 0"]
            .into_iter()
            .take(players)
            .collect();
        parts.extend(["F2 Ton", "F4 Musik", "Esc Menü"]);
        parts.join("  ·  ")
    };
    let roster = Roster::local();
    show_mouse(false);
    let mut acc = 0.0f64;
    loop {
        if app.common_keys() {
            show_mouse(true);
            return false;
        }
        if is_quit_requested() {
            return true;
        }
        drain_chars();
        acc = (acc + get_frame_time() as f64).min(0.25);
        // Tasten einmal pro Bild lesen (sonst zählt ein Druck in jedem Tick)
        let states: Vec<(u8, u16)> = inputs.iter_mut().map(|i| i.state(true)).collect();
        while acc >= TICK {
            acc -= TICK;
            for (slot, &(k, f)) in states.iter().enumerate() {
                game.apply_input(slot, k, f, TICK);
            }
            for b in robots.iter_mut() {
                let (k, f) = b.think(&game);
                game.apply_input(b.slot, k, f, TICK);
            }
            game.step(TICK);
        }
        app.begin();
        let view = game.net_snap(me, false).view();
        let frame = Frame { arena: &game.arena, view: &view, me, roster: &roster, quiet: false, hud: true };
        app.r.draw(&mut app.g, &frame);
        hint(&app.g, &text);
        if app.r.debug {
            app.g.text(&format!("{} FPS", get_fps()), 8.0, 28.0, 15.0, col([255, 90, 180]), 0.0);
        }
        app.present().await;
    }
}

/// Meldung, weiter mit beliebiger Taste
pub async fn message(app: &mut App, text: &str) -> bool {
    show_mouse(true);
    let start = get_time();
    loop {
        if is_quit_requested() {
            return true;
        }
        app.begin();
        app.backdrop();
        theme::panel(Rect::new(96.0, 240.0, 808.0, 152.0), 16.0, theme::PAPER);
        let text = theme::fit(&app.g, text, 24.0, 744.0);
        app.g.text(&text, VW / 2.0, 300.0, 24.0, col(theme::INK), 0.5);
        app.g.text("Weiter mit einer beliebigen Taste", VW / 2.0, 350.0, 15.0, col(theme::SECONDARY), 0.5);
        let any = get_last_key_pressed().is_some() || is_mouse_button_pressed(MouseButton::Left);
        drain_chars();
        if any && get_time() - start > 0.3 {
            return false;
        }
        app.present().await;
    }
}

struct HostCtx {
    ip: String,
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for HostCtx {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

pub async fn run_host(app: &mut App, port: u16, bots: usize) -> bool {
    let host_key = panzer_core::rng::Rng::from_entropy().next_u64() | 1;
    let cfg = ServerConfig { port, bots, rule: app.cfg.rule, host_key, ..Default::default() };
    let mut server = match Server::bind(cfg) {
        Ok(s) => s,
        Err(e) => return message(app, &format!("Port {port} geht nicht: {e}")).await,
    };
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::spawn(move || server.run(&flag));
    let ctx = HostCtx { ip: local_ip(), port, stop, thread: Some(thread) };
    let net = match NetClient::connect(&format!("127.0.0.1:{port}"), &app.cfg.name, app.cfg.token, host_key) {
        Ok(n) => n,
        Err(e) => return message(app, &format!("Verbindung zum eigenen Server: {e}")).await,
    };
    run_online(app, net, Some(&ctx)).await
}

pub async fn run_join(app: &mut App, addr: &str) -> bool {
    let net = match NetClient::connect(addr, &app.cfg.name, app.cfg.token, 0) {
        Ok(n) => n,
        Err(e) => return message(app, &format!("Verbindung fehlgeschlagen: {e}")).await,
    };
    run_online(app, net, None).await
}

async fn run_online(app: &mut App, mut net: NetClient, host: Option<&HostCtx>) -> bool {
    let mut inp = KeyInput::net();
    let mut chat = Chat::new();
    show_mouse(false);
    loop {
        net.update();
        if let NetState::Closed(reason) = &net.state {
            let reason = reason.clone();
            show_mouse(true);
            return message(app, &reason).await;
        }
        let was_open = chat.open;
        if let Some(text) = chat.update() {
            net.say(&text);
        }
        let esc = app.common_keys();
        if esc && !was_open {
            net.bye();
            show_mouse(true);
            return false;
        }
        if is_quit_requested() && !(esc && was_open) {
            net.bye();
            return true;
        }
        let tab = is_key_down(KeyCode::Tab);
        let (keys, fires) = inp.state(!chat.open);
        net.send_input(keys, fires, get_frame_time() as f64);
        app.begin();
        let view = net.view();
        let queued = net.queued();
        let shown = match (&net.arena, &view) {
            (Some(arena), Some(v)) if v.maze_id == arena.id && v.maze_id != 0 => {
                let me = if queued { None } else { Some(net.slot) };
                let frame = Frame { arena, view: v, me, roster: &net.roster, quiet: false, hud: true };
                app.r.draw(&mut app.g, &frame);
                true
            }
            _ => false,
        };
        if !shown {
            app.backdrop();
            let g = &mut app.g;
            theme::panel(Rect::new(160.0, 192.0, 680.0, 280.0), 16.0, theme::PAPER);
            match (&net.state, host) {
                (NetState::Connecting, _) => {
                    g.text("Verbinde …", VW / 2.0, 300.0, 32.0, col(theme::INK), 0.5);
                }
                (_, Some(h)) => {
                    g.text("Warte auf Mitspieler …", VW / 2.0, 250.0, 32.0, col(theme::INK), 0.5);
                    g.text(
                        &format!("Deine IP:  {}   Port: {}", h.ip, h.port),
                        VW / 2.0,
                        320.0,
                        24.0,
                        col(theme::INK),
                        0.5,
                    );
                    g.text(
                        &format!("Bis zu {} Freunde wählen \"Beitreten\" und geben diese IP ein.", MAX_PLAYERS - 1),
                        VW / 2.0,
                        362.0,
                        15.0,
                        col(theme::SECONDARY),
                        0.5,
                    );
                    g.text(
                        "Nicht im selben Netz? Tailscale/ZeroTier-IP oder Port-Forwarding (UDP).",
                        VW / 2.0,
                        387.0,
                        15.0,
                        col(theme::SECONDARY),
                        0.5,
                    );
                    g.text("Bots dazu: im Chat /bots 3", VW / 2.0, 412.0, 15.0, col(theme::SECONDARY), 0.5);
                }
                _ => {
                    g.text("Warte auf Spielstart …", VW / 2.0, 300.0, 32.0, col(theme::INK), 0.5);
                }
            }
        }
        if queued && shown {
            let g = &mut app.g;
            theme::panel(Rect::new(VW / 2.0 - 280.0, 44.0, 560.0, 36.0), 8.0, theme::PAPER);
            g.text("Warteschlange – ab der nächsten Runde bist du dabei", VW / 2.0, 61.0, 18.0, col(theme::INK), 0.5);
        }
        if let Some(h) = host {
            let used = net.roster.names.iter().filter(|n| !n.is_empty()).count();
            let free = MAX_PLAYERS.saturating_sub(used);
            if free > 0 && shown {
                app.g.text(&format!("{free} Platz frei  ·  {}:{}", h.ip, h.port), VW - 8.0, 9.0, 13.0, col(DIM_C), 1.0);
            }
        }
        chat.draw(&app.g, &net);
        if tab {
            show_mouse(true);
            let scores = view.as_ref().map_or([0; MAX_PLAYERS], |v| v.scores);
            let m = Layout::now().mouse();
            if let Some(cmd) = scoreboard(&mut app.g, &net.roster, &scores, net.slot, net.admin, &net.queued_list, m) {
                net.say(&cmd);
            }
        } else {
            show_mouse(false);
        }
        if app.r.debug {
            let ping = net.ping.map_or("–".into(), |p| format!("{p:.0} ms"));
            let info = format!(
                "{} FPS  ·  Ping {ping}  ·  ↓ {:.1} KB/s  ↑ {:.1} KB/s{}",
                get_fps(),
                net.rate_in / 1024.0,
                net.rate_out / 1024.0,
                if host.is_some() { "  ·  Host" } else { "" }
            );
            app.g.text(&info, 8.0, 28.0, 15.0, col([255, 90, 180]), 0.0);
        }
        if !chat.open {
            hint(&app.g, NET_HINT);
        }
        app.present().await;
    }
}

/// Hauptmenü. None = beenden.
#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuMode {
    Solo,
    Local,
    Host,
    Join,
}

impl MenuMode {
    fn online(self) -> bool {
        matches!(self, Self::Host | Self::Join)
    }
    fn label(self) -> &'static str {
        match self {
            Self::Solo => "Solo",
            Self::Local => "Eine Tastatur",
            Self::Host => "Hosten",
            Self::Join => "Beitreten",
        }
    }
    fn detail(self) -> &'static str {
        match self {
            Self::Solo => "Du gegen die Bots",
            Self::Local => "2–4 Menschen am Keyboard",
            Self::Host => "Eine Online-Runde eröffnen",
            Self::Join => "Über die IP des Hosts",
        }
    }
}

fn mode_tile(g: &Gfx, rect: Rect, mode: MenuMode, selected: bool, mouse: Vec2) {
    let hot = rect.contains(mouse);
    let fill = if selected {
        theme::DECK
    } else if hot {
        theme::SURFACE
    } else {
        theme::PAPER
    };
    theme::plate(rect, fill, if selected { theme::DECK } else { theme::LINE });
    let text = if selected { theme::ON_DECK } else { theme::INK };
    let dim = if selected { theme::DECK_DIM } else { theme::SECONDARY };
    let slot = match mode {
        MenuMode::Solo => 0,
        MenuMode::Local => 1,
        MenuMode::Host => 3,
        MenuMode::Join => 5,
    };
    g.sprite(&g.spr.tanks[slot], rect.x + 28.0, rect.y + 31.0, -0.35, 0.8, WHITE);
    if mode == MenuMode::Local {
        g.sprite(&g.spr.tanks[0], rect.x + 36.0, rect.y + 39.0, -0.35, 0.55, WHITE);
    }
    g.text(mode.label(), rect.x + 56.0, rect.y + 28.0, 18.0, col(text), 0.0);
    g.text(mode.detail(), rect.x + 16.0, rect.y + 65.0, 12.0, col(dim), 0.0);
    if selected {
        theme::treads(rect.right() - 30.0, rect.y + 20.0, theme::YELLOW);
    }
}

pub async fn menu(app: &mut App) -> Option<Action> {
    show_mouse(true);
    let mut mode = MenuMode::Solo;
    let mut local_players = 2;
    let mut name = Field::new(&app.cfg.name, 16);
    let mut ip = Field::new(&app.cfg.last_ip, 60);
    let mut error: Option<(String, f64)> = None;
    let modes = [MenuMode::Solo, MenuMode::Local, MenuMode::Host, MenuMode::Join];
    let tiles: Vec<Rect> = (0..4).map(|i| Rect::new(40.0 + i as f32 * 232.0, 108.0, 220.0, 92.0)).collect();
    let name_r = Rect::new(64.0, 328.0, 304.0, 40.0);
    let ip_r = Rect::new(64.0, 408.0, 304.0, 40.0);
    let start_r = Rect::new(64.0, 532.0, 304.0, 48.0);
    let rules = RoundRule::all();
    drain_chars();
    loop {
        let esc = app.common_keys();
        if is_quit_requested() && !esc {
            return None;
        }
        let m = Layout::now().mouse();
        let click = is_mouse_button_pressed(MouseButton::Left);
        if click {
            if let Some(i) = tiles.iter().position(|r| r.contains(m)) {
                mode = modes[i];
                error = None;
                name.focus = mode.online() && name.text.is_empty();
                ip.focus = mode == MenuMode::Join && !name.focus;
            } else {
                name.focus = mode.online() && name_r.contains(m);
                ip.focus = mode == MenuMode::Join && ip_r.contains(m);
            }
        }
        if is_key_pressed(KeyCode::Tab) && mode.online() {
            if mode == MenuMode::Join {
                let was_name = name.focus;
                name.focus = !was_name;
                ip.focus = was_name;
            } else {
                name.focus = true;
                ip.focus = false;
            }
        }
        name.typing(|c| c.is_alphanumeric() || " _-.!?'".contains(c));
        ip.typing(|c| c.is_ascii_alphanumeric() || ".:-[]".contains(c));
        if !name.focus && !ip.focus {
            drain_chars();
        }
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        let mut submit = enter;
        if enter && mode == MenuMode::Join && name.focus {
            name.focus = false;
            ip.focus = true;
            submit = false;
        }

        app.begin();
        draw_rectangle(0.0, 0.0, VW, VH, col(theme::PAPER));
        draw_rectangle(0.0, 0.0, VW, 84.0, col(theme::DECK));
        theme::plate(Rect::new(416.0, 224.0, 544.0, 380.0), theme::DECK, theme::DECK);
        app.arena_preview(Rect::new(432.0, 272.0, 512.0, 296.0));
        let g = &mut app.g;
        g.fancy("PANZER", 108.0, 42.0, 40.0, theme::ON_DECK, 0.0, true, 1.0, 1.0);
        draw_line(192.0, 28.0, 192.0, 56.0, 1.0, col(theme::DECK_RAISED));
        g.text("Jeder Winkel zählt", 216.0, 42.0, 17.0, col(theme::DECK_DIM), 0.0);
        g.text("Eine Tastatur oder LAN", 960.0, 42.0, 13.0, col(theme::DECK_DIM), 1.0);
        for (i, &candidate) in modes.iter().enumerate() {
            mode_tile(g, tiles[i], candidate, mode == candidate, m);
        }
        theme::plate(Rect::new(40.0, 224.0, 352.0, 380.0), theme::SURFACE, theme::LINE);
        g.text(mode.label(), 64.0, 256.0, 26.0, col(theme::INK), 0.0);
        g.text(mode.detail(), 64.0, 286.0, 13.0, col(theme::SECONDARY), 0.0);
        g.text("Arena-Vorschau", 436.0, 248.0, 15.0, col(theme::ON_DECK), 0.0);
        draw_circle(926.0, 248.0, 3.0, col([136, 199, 160]));
        g.text("Live", 916.0, 248.0, 12.0, col(theme::DECK_DIM), 1.0);
        for i in 0..MAX_PLAYERS {
            draw_circle(440.0 + i as f32 * 10.0, 586.0, 2.5, col(COLORS[i]));
        }
        g.text("Bis zu 8 Panzer", 534.0, 586.0, 12.0, col(theme::DECK_DIM), 0.0);
        g.text("Neues Labyrinth pro Runde", 940.0, 586.0, 12.0, col(theme::DECK_DIM), 1.0);

        if mode.online() {
            g.text("Dein Online-Name", 64.0, 312.0, 12.0, col(theme::SECONDARY), 0.0);
            name.draw(g, name_r, "1–16 Zeichen", m);
        } else if mode == MenuMode::Local {
            for n in 2..=MAX_LOCAL {
                let r = Rect::new(64.0 + (n - 2) as f32 * 104.0, 320.0, 96.0, 48.0);
                theme::plate(r, if local_players == n { theme::DECK } else { theme::PAPER }, theme::LINE);
                g.sprite(&g.spr.tanks[n - 1], r.x + 22.0, r.y + 24.0, -0.25, 0.65, WHITE);
                g.text(
                    &n.to_string(),
                    r.x + 64.0,
                    r.y + 24.0,
                    20.0,
                    col(if local_players == n { theme::ON_DECK } else { theme::INK }),
                    0.5,
                );
                if click && r.contains(m) {
                    local_players = n;
                }
            }
        } else {
            let shown = app.cfg.bots.clamp(1, MAX_PLAYERS - 1);
            g.sprite(&g.spr.tanks[0], 96.0, 340.0, -0.3, 1.25, WHITE);
            g.text("vs", 148.0, 340.0, 15.0, col(theme::SECONDARY), 0.5);
            for j in 0..shown {
                g.sprite(&g.spr.tanks[MAX_PLAYERS - shown + j], 184.0 + j as f32 * 25.0, 340.0, 0.3, 0.65, WHITE);
            }
        }
        if mode == MenuMode::Join {
            g.text("IP-Adresse des Hosts", 64.0, 392.0, 12.0, col(theme::SECONDARY), 0.0);
            ip.draw(g, ip_r, "z. B. 192.168.1.10", m);
            g.text("Die Rundenregeln bestimmt der Host", 64.0, 478.0, 12.0, col(theme::SECONDARY), 0.0);
        } else {
            let cap = if mode == MenuMode::Local { MAX_PLAYERS - local_players } else { MAX_PLAYERS - 1 };
            let min = if mode == MenuMode::Solo { 1 } else { 0 };
            let bots = app.cfg.bots.clamp(min, cap);
            g.text("Bots", 64.0, 400.0, 16.0, col(theme::INK), 0.0);
            for (r, delta, label) in
                [(Rect::new(248.0, 384.0, 32.0, 32.0), -1, "–"), (Rect::new(336.0, 384.0, 32.0, 32.0), 1, "+")]
            {
                if button(g, r, label, m, theme::YELLOW, 20.0) {
                    app.cfg.bots = (bots as i32 + delta).clamp(min as i32, cap as i32) as usize;
                    app.r.sfx.play("blip", None, 1.0);
                }
            }
            g.text(&bots.to_string(), 308.0, 400.0, 20.0, col(theme::INK), 0.5);
            g.text("Rundenregel", 64.0, 450.0, 16.0, col(theme::INK), 0.0);
            let here = rules.iter().position(|&r| r == app.cfg.rule).unwrap_or(0);
            for (r, step, label) in [
                (Rect::new(248.0, 434.0, 32.0, 32.0), rules.len() - 1, "<"),
                (Rect::new(336.0, 434.0, 32.0, 32.0), 1, ">"),
            ] {
                if button(g, r, label, m, theme::YELLOW, 20.0) {
                    app.cfg.rule = rules[(here + step) % rules.len()];
                    app.r.sfx.play("blip", None, 1.0);
                }
            }
            // The value sits on its own line, so long rule names never collide with the arrows.
            let rule = theme::fit(g, app.cfg.rule.name(), 13.0, 152.0);
            g.text(&rule, 64.0, 478.0, 13.0, col(theme::SECONDARY), 0.0);
        }
        let label = match mode {
            MenuMode::Solo => "In die Arena",
            MenuMode::Local => "Runde starten",
            MenuMode::Host => "Server starten",
            MenuMode::Join => "Verbinden",
        };
        let launch = primary_button(g, start_r, label, m) || submit;
        let mut action = None;
        if launch {
            if mode.online() && clean_name(&name.text).is_none() {
                error = Some(("Dein Online-Name fehlt (1–16 Zeichen)".into(), get_time()));
                name.focus = true;
                ip.focus = false;
            } else if mode == MenuMode::Join && ip.text.trim().is_empty() {
                error = Some(("Zum Verbinden brauchst du eine IP-Adresse".into(), get_time()));
                ip.focus = true;
                name.focus = false;
            } else {
                action = Some(match mode {
                    MenuMode::Solo => Action::Local { players: 1, bots: app.cfg.bots.clamp(1, MAX_PLAYERS - 1) },
                    MenuMode::Local => {
                        Action::Local { players: local_players, bots: app.cfg.bots.min(MAX_PLAYERS - local_players) }
                    }
                    MenuMode::Host => Action::Host { port: PORT, bots: app.cfg.bots.min(MAX_PLAYERS - 1) },
                    MenuMode::Join => Action::Join(ip.text.trim().to_string()),
                });
            }
        }
        if let Some((text, _)) = error.as_ref().filter(|(_, at)| get_time() - at < 4.0) {
            let text = theme::fit(g, text, 12.0, 304.0);
            g.text(&text, 64.0, 508.0, 12.0, col([170, 49, 39]), 0.0);
        } else if mode != MenuMode::Join {
            let desc = theme::fit(g, app.cfg.rule.desc(), 11.0, 304.0);
            g.text(&desc, 64.0, 506.0, 11.0, col(theme::SECONDARY), 0.0);
        }
        let mut x = 40.0;
        for (key, text) in
            [("Enter", "Starten"), ("Tab", "Eingabefeld"), ("F11", "Vollbild"), ("F2", "Ton"), ("F4", "Musik")]
        {
            x += theme::keyhint(g, key, text, x, 642.0, false) + 24.0;
        }
        if let Some(a) = action {
            if let Some(n) = clean_name(&name.text) {
                app.cfg.name = n;
            }
            app.cfg.last_ip = ip.text.trim().to_string();
            app.cfg.save();
            app.present().await;
            return Some(a);
        }
        app.present().await;
    }
}
