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
use crate::ui::{button, drain_chars, scoreboard, Chat, Field};
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
    g.text(text, VW / 2.0, VH - 11.0, 13.0, col([110, 116, 132]), 0.5);
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
        app.g.fancy(text, VW / 2.0, 300.0, 30.0, [255, 255, 255], 3.0, false, 1.0, 1.0);
        app.g.text("Beliebige Taste …", VW / 2.0, 350.0, 15.0, col(DIM_C), 0.5);
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
            match (&net.state, host) {
                (NetState::Connecting, _) => {
                    g.fancy("Verbinde …", VW / 2.0, 300.0, 40.0, [255, 255, 255], 3.0, true, 1.0, 1.0);
                }
                (_, Some(h)) => {
                    g.fancy("Warte auf Mitspieler …", VW / 2.0, 250.0, 44.0, [255, 255, 255], 3.0, true, 1.0, 1.0);
                    g.fancy(
                        &format!("Deine IP:  {}   Port: {}", h.ip, h.port),
                        VW / 2.0,
                        320.0,
                        28.0,
                        ACCENT_C,
                        2.0,
                        false,
                        1.0,
                        1.0,
                    );
                    g.text(
                        &format!("Bis zu {} Freunde wählen \"Beitreten\" und geben diese IP ein.", MAX_PLAYERS - 1),
                        VW / 2.0,
                        362.0,
                        15.0,
                        col(DIM_C),
                        0.5,
                    );
                    g.text(
                        "Nicht im selben Netz? Tailscale/ZeroTier-IP oder Port-Forwarding (UDP).",
                        VW / 2.0,
                        387.0,
                        15.0,
                        col(DIM_C),
                        0.5,
                    );
                    g.text("Bots dazu: im Chat /bots 3", VW / 2.0, 412.0, 15.0, col(DIM_C), 0.5);
                }
                _ => {
                    g.fancy("Warte auf Spielstart …", VW / 2.0, 300.0, 44.0, [255, 255, 255], 3.0, true, 1.0, 1.0);
                }
            }
        }
        if queued && shown {
            let g = &mut app.g;
            draw_rectangle(VW / 2.0 - 280.0, 44.0, 560.0, 34.0, Color::new(0.04, 0.045, 0.06, 0.85));
            g.fancy(
                "Warteschlange – ab der nächsten Runde bist du dabei",
                VW / 2.0,
                61.0,
                20.0,
                ACCENT_C,
                2.0,
                false,
                1.0,
                1.0,
            );
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
pub async fn menu(app: &mut App) -> Option<Action> {
    show_mouse(true);
    let cx = VW / 2.0;
    let mut name = Field::new(&app.cfg.name, 16);
    let mut ip = Field::new(&app.cfg.last_ip, 60);
    name.focus = name.text.is_empty();
    ip.focus = !name.focus;
    let mut error: Option<(String, f64)> = None;
    let name_r = Rect::new(cx - 190.0, 202.0, 380.0, 42.0);
    let host_b = Rect::new(cx - 190.0, 254.0, 380.0, 50.0);
    let ip_r = Rect::new(cx - 190.0, 318.0, 380.0, 42.0);
    let join_b = Rect::new(cx - 190.0, 368.0, 380.0, 50.0);
    let local_b: Vec<(usize, Rect)> =
        (1..=4).map(|n| (n, Rect::new(cx - 190.0 + (n - 1) as f32 * 97.0, 458.0, 89.0, 44.0))).collect();
    let minus_b = Rect::new(cx + 44.0, 514.0, 40.0, 32.0);
    let plus_b = Rect::new(cx + 150.0, 514.0, 40.0, 32.0);
    let rule_prev = Rect::new(cx - 94.0, 552.0, 34.0, 32.0);
    let rule_next = Rect::new(cx + 156.0, 552.0, 34.0, 32.0);
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
            name.focus = name_r.contains(m);
            ip.focus = ip_r.contains(m);
        }
        if is_key_pressed(KeyCode::Tab) {
            let n = name.focus;
            name.focus = !n;
            ip.focus = n;
        }
        name.typing(|c| c.is_alphanumeric() || " _-.!?'".contains(c));
        ip.typing(|c| c.is_ascii_alphanumeric() || ".:-[]".contains(c));
        if !name.focus && !ip.focus {
            drain_chars();
        }
        let enter = is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::KpEnter);
        if enter && name.focus {
            name.focus = false;
            ip.focus = true;
        }
        // Name ist Pflicht fürs Netzspiel
        let named = |name: &mut Field, error: &mut Option<(String, f64)>| -> bool {
            if clean_name(&name.text).is_some() {
                return true;
            }
            *error = Some(("Bitte zuerst deinen Namen eingeben (1–16 Zeichen).".into(), get_time()));
            name.focus = true;
            false
        };

        app.begin();
        app.backdrop();
        let g = &mut app.g;
        let t = get_time() as f32;
        let pulse = 0.5 + 0.5 * (t * 2.2).sin();
        g.text_glow("PANZER", cx, 96.0, 120.0, [255, 150, 40], 5.0 + 3.0 * pulse, true);
        g.flush_light();
        g.fancy("PANZER", cx, 96.0, 120.0, [255, 196, 60], 4.0, true, 1.0, 1.0);
        g.fancy(
            "Labyrinth-Panzerschlacht  ·  1–8 Spieler  ·  Bots  ·  LAN & Internet",
            cx,
            166.0,
            20.0,
            [225, 230, 240],
            2.0,
            false,
            1.0,
            1.0,
        );
        draw_rectangle(
            cx - 215.0,
            186.0,
            430.0,
            446.0,
            Color::new(13.0 / 255.0, 15.0 / 255.0, 21.0 / 255.0, 215.0 / 255.0),
        );
        draw_rectangle_lines(cx - 215.0, 186.0, 430.0, 446.0, 1.0, col([70, 78, 100]));
        name.draw(g, name_r, "Dein Name (Pflicht fürs Online-Spiel)", m);
        let mut action = None;
        if button(g, host_b, "Spiel hosten", m, ACCENT_C, 24.0) && named(&mut name, &mut error) {
            action = Some(Action::Host { port: PORT, bots: app.cfg.bots.min(MAX_PLAYERS - 1) });
        }
        ip.draw(g, ip_r, "IP-Adresse des Hosts eingeben", m);
        let join = button(g, join_b, "Beitreten", m, ACCENT_C, 24.0) || (enter && ip.focus);
        if join && named(&mut name, &mut error) {
            if ip.text.trim().is_empty() {
                error = Some(("Bitte die IP-Adresse des Hosts eingeben.".into(), get_time()));
                ip.focus = true;
                name.focus = false;
            } else {
                action = Some(Action::Join(ip.text.trim().to_string()));
            }
        }
        g.fancy("LOKAL AN EINER TASTATUR", cx, 442.0, 16.0, DIM_C, 2.0, false, 1.0, 1.0);
        for &(n, r) in &local_b {
            let label = ["Solo", "2 Spieler", "3 Spieler", "4 Spieler"][n - 1];
            let accent = if n == 1 { [110, 200, 255] } else { ACCENT_C };
            if button(g, r, label, m, accent, 20.0) {
                let bots = app.cfg.bots;
                action = Some(if n == 1 {
                    Action::Local { players: 1, bots: bots.clamp(1, MAX_PLAYERS - 1) }
                } else {
                    Action::Local { players: n, bots: bots.min(MAX_PLAYERS - n) }
                });
            }
        }
        g.fancy("BOTS DAZU", cx - 130.0, 530.0, 20.0, TEXT_C, 2.0, false, 1.0, 1.0);
        for (r, d) in [(minus_b, -1i32), (plus_b, 1)] {
            if button(g, r, if d < 0 { "–" } else { "+" }, m, ACCENT_C, 26.0) {
                app.cfg.bots = (app.cfg.bots as i32 + d).clamp(0, MAX_PLAYERS as i32 - 1) as usize;
                app.r.sfx.play("blip", None, 1.0);
            }
        }
        g.fancy(
            &app.cfg.bots.to_string(),
            (minus_b.x + minus_b.w + plus_b.x) / 2.0,
            530.0,
            34.0,
            ACCENT_C,
            3.0,
            true,
            1.0,
            1.0,
        );
        // Sonderregel: gilt für die ganze Partie, wenn man nicht nur beitritt
        g.fancy("REGEL", cx - 150.0, 568.0, 20.0, TEXT_C, 2.0, false, 1.0, 1.0);
        let here = rules.iter().position(|&r| r == app.cfg.rule).unwrap_or(0);
        for (r, step, label) in [(rule_prev, rules.len() - 1, "<"), (rule_next, 1, ">")] {
            if button(g, r, label, m, ACCENT_C, 22.0) {
                app.cfg.rule = rules[(here + step) % rules.len()];
                app.r.sfx.play("blip", None, 1.0);
            }
        }
        let rule_x = (rule_prev.x + rule_prev.w + rule_next.x) / 2.0;
        g.fancy(app.cfg.rule.name(), rule_x, 568.0, 20.0, ACCENT_C, 2.0, false, 1.0, 1.0);
        g.text(app.cfg.rule.desc(), cx, 597.0, 13.0, col(TEXT_C), 0.5);
        let b = app.cfg.bots.max(1);
        g.text(
            &format!(
                "Solo: du gegen {b} Bot{}  ·  beim Hosten und lokal füllen Bots freie Plätze",
                if b > 1 { "s" } else { "" }
            ),
            cx,
            616.0,
            13.0,
            col(DIM_C),
            0.5,
        );
        if let Some((e, at)) = &error {
            if get_time() - at < 4.0 {
                g.fancy(e, cx, 646.0, 20.0, [255, 110, 90], 2.0, false, 1.0, 1.0);
            }
        }
        g.text(
            "F11 Vollbild  ·  F2 Ton  ·  F4 Musik  ·  Tab wechselt das Feld",
            cx,
            VH - 16.0,
            15.0,
            col([120, 126, 142]),
            0.5,
        );
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
