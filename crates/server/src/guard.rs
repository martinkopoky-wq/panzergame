//! Anti-Cheat pro Client.
//!
//! Der Server ist autoritativ: Clients schicken nur Tasten, alles andere rechnet der Server.
//! Was ein manipulierter Client trotzdem versuchen kann, wird hier abgefangen:
//! - Speedhack: mehr Eingabezeit schicken als real vergeht (die alte Python-Version hat
//!   bei mehr als 6 wartenden Eingaben einfach alle auf einmal ausgeführt). Jetzt gibt es
//!   ein Zeitbudget: pro Tick darf ein Client höchstens so viel Spielzeit verbrauchen, wie
//!   real vergangen ist (+5 % zum Aufholen). Überschüssige Eingaben verfallen und zählen.
//! - Paketflut und kaputte Pakete
//! - Dauerfeuer-Makros (unmenschlich viele Schuss-Tastendrücke)
//! - Chat-Spam
//! Wallhacks gehen nicht, weil der Server Verstecktes gar nicht erst schickt (game/view.rs).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use panzer_core::consts::TICK;
use panzer_core::proto::Cmd;

/// Eingabe-Budget: so viel Spielzeit darf sich ansammeln (Netz-Ruckler ausgleichen)
const MAX_BUDGET: f64 = 0.25;
/// längste erlaubte Framedauer einer Eingabe
pub const MAX_DT: f64 = 0.05;
/// mehr wartende Eingaben (in Sekunden) verfallen
const MAX_BACKLOG: f64 = 0.6;
/// Messfenster für das Verhältnis Eingabezeit / Echtzeit
const WINDOW: Duration = Duration::from_secs(10);
/// ab diesem Verhältnis über ein ganzes Fenster: Verwarnung
const SPEED_LIMIT: f64 = 1.15;
const MAX_PACKETS_PER_SEC: u32 = 300;
const MAX_FIRES_PER_SEC: f64 = 25.0;
/// so viele Verwarnungen, dann fliegt der Client
pub const STRIKES_KICK: f64 = 3.0;

pub struct Guard {
    pub budget: f64,
    pub queue: VecDeque<(u32, Cmd)>,
    /// höchste angenommene Eingabe
    pub last_seq: u32,
    /// zuletzt ausgeführte Eingabe (geht als Ack an den Client)
    pub done_seq: u32,
    window_start: Instant,
    window_input: f64,
    pkt_second: Instant,
    pkts: u32,
    fire_window: Instant,
    fire_presses: u32,
    last_fires: Option<u16>,
    pub strikes: f64,
    pub bad_packets: u32,
    pub dropped: f64,
    chat: VecDeque<Instant>,
}

pub enum Verdict {
    Ok,
    /// verdächtig: Grund fürs Log
    Warn(String),
}

impl Guard {
    pub fn new(now: Instant) -> Guard {
        Guard {
            budget: 0.0,
            queue: VecDeque::new(),
            last_seq: 0,
            done_seq: 0,
            window_start: now,
            window_input: 0.0,
            pkt_second: now,
            pkts: 0,
            fire_window: now,
            fire_presses: 0,
            last_fires: None,
            strikes: 0.0,
            bad_packets: 0,
            dropped: 0.0,
            chat: VecDeque::new(),
        }
    }

    /// Pro Paket: Paketflut erkennen. false = Paket verwerfen.
    pub fn packet(&mut self, now: Instant) -> bool {
        if now.duration_since(self.pkt_second) >= Duration::from_secs(1) {
            self.pkt_second = now;
            self.pkts = 0;
        }
        self.pkts += 1;
        self.pkts <= MAX_PACKETS_PER_SEC
    }

    pub fn bad_packet(&mut self) -> Verdict {
        self.bad_packets += 1;
        if self.bad_packets % 20 == 0 {
            self.strikes += 1.0;
            return Verdict::Warn(format!("{} kaputte Pakete", self.bad_packets));
        }
        Verdict::Ok
    }

    /// Neue Eingaben aus einem Paket in die Warteschlange (Doppelte durch die Redundanz
    /// werden übersprungen).
    pub fn accept(&mut self, first_seq: u32, cmds: &[Cmd]) {
        for (k, c) in cmds.iter().enumerate() {
            let seq = first_seq.wrapping_add(k as u32);
            if seq <= self.last_seq {
                continue;
            }
            self.last_seq = seq;
            let dt = (c.dt_ms as f64 / 1000.0).min(MAX_DT);
            self.window_input += dt;
            if self.last_fires.is_some_and(|f| f != c.fires) {
                self.fire_presses += 1;
            }
            self.last_fires = Some(c.fires);
            self.queue.push_back((seq, *c));
        }
    }

    /// Eingaben, die in diesem Tick ausgeführt werden dürfen
    pub fn take_tick(&mut self) -> Vec<(u32, Cmd, f64)> {
        let mut out = Vec::new();
        self.take_tick_into(&mut out);
        out
    }

    /// wie take_tick, aber in einen wiederverwendeten Puffer (läuft pro Client und Tick)
    pub fn take_tick_into(&mut self, out: &mut Vec<(u32, Cmd, f64)>) {
        out.clear();
        let backlog: f64 = self.queue.iter().map(|(_, c)| (c.dt_ms as f64 / 1000.0).min(MAX_DT)).sum();
        // leicht schneller abarbeiten, wenn sich etwas angestaut hat (Uhren laufen nie gleich)
        let gain = if backlog > 0.1 { 1.05 } else { 1.0 };
        self.budget = (self.budget + TICK * gain).min(MAX_BUDGET);
        while let Some(&(seq, c)) = self.queue.front() {
            let dt = (c.dt_ms as f64 / 1000.0).min(MAX_DT);
            if dt > self.budget + 1e-6 {
                break;
            }
            self.queue.pop_front();
            self.budget -= dt;
            self.done_seq = seq;
            out.push((seq, c, dt));
        }
        // zu viel angestaut: das Älteste verfällt (so kann niemand Zeit "vorproduzieren")
        let mut backlog: f64 = self.queue.iter().map(|(_, c)| (c.dt_ms as f64 / 1000.0).min(MAX_DT)).sum();
        while backlog > MAX_BACKLOG {
            let Some((seq, c)) = self.queue.pop_front() else { break };
            let dt = (c.dt_ms as f64 / 1000.0).min(MAX_DT);
            backlog -= dt;
            self.dropped += dt;
            self.done_seq = seq;
        }
    }

    /// Einmal pro Sekunde: Verhältnisse prüfen
    pub fn review(&mut self, now: Instant) -> Verdict {
        let mut warn = Verdict::Ok;
        let fire_span = now.duration_since(self.fire_window).as_secs_f64();
        if fire_span >= 3.0 {
            let rate = self.fire_presses as f64 / fire_span;
            if rate > MAX_FIRES_PER_SEC {
                self.strikes += 1.0;
                warn = Verdict::Warn(format!("Dauerfeuer-Makro? {rate:.0} Schüsse/s"));
            }
            self.fire_window = now;
            self.fire_presses = 0;
        }
        let span = now.duration_since(self.window_start);
        if span >= WINDOW {
            let ratio = self.window_input / span.as_secs_f64();
            if ratio > SPEED_LIMIT {
                self.strikes += 1.0;
                warn = Verdict::Warn(format!(
                    "Speedhack? {:.0} % Eingabezeit, {:.2} s verworfen",
                    ratio * 100.0,
                    self.dropped
                ));
            }
            self.window_start = now;
            self.window_input = 0.0;
            self.dropped = 0.0;
        }
        warn
    }

    pub fn should_kick(&self) -> bool {
        self.strikes >= STRIKES_KICK
    }

    /// Chat-Spam: höchstens 5 Nachrichten in 5 Sekunden
    pub fn chat_ok(&mut self, now: Instant) -> bool {
        while self.chat.front().is_some_and(|t| now.duration_since(*t) > Duration::from_secs(5)) {
            self.chat.pop_front();
        }
        if self.chat.len() >= 5 {
            return false;
        }
        self.chat.push_back(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(dt: u8) -> Cmd {
        Cmd { keys: 1, fires: 0, dt_ms: dt }
    }

    #[test]
    fn speedhack_gets_no_extra_time() {
        let now = Instant::now();
        let mut g = Guard::new(now);
        let mut seq = 1;
        let mut sim = 0.0;
        // Client schickt pro Tick 10 Eingaben à 50 ms (30-fache Geschwindigkeit)
        for _ in 0..600 {
            let cmds: Vec<Cmd> = (0..10).map(|_| cmd(50)).collect();
            g.accept(seq, &cmds);
            seq += 10;
            sim += g.take_tick().iter().map(|c| c.2).sum::<f64>();
        }
        let real = 600.0 * TICK;
        assert!(sim <= real * 1.06 + MAX_BUDGET, "sim {sim} real {real}");
    }

    #[test]
    fn honest_client_runs_full_speed() {
        let now = Instant::now();
        let mut g = Guard::new(now);
        let mut sim = 0.0;
        // 144 FPS-Client, Pakete kommen in Schüben
        let mut seq = 1;
        let mut pending = Vec::new();
        for tick in 0..6000 {
            let frames = if tick % 5 == 4 { 3 } else { 2 };
            for _ in 0..frames {
                pending.push(cmd(7));
            }
            if tick % 3 == 0 {
                g.accept(seq, &pending);
                seq += pending.len() as u32;
                pending.clear();
            }
            sim += g.take_tick().iter().map(|c| c.2).sum::<f64>();
        }
        let sent = (seq - 1) as f64 * 0.007;
        assert!(sim > sent - 0.5, "sim {sim} sent {sent}");
        assert!(g.dropped < 0.01);
    }
}
