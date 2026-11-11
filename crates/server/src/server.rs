//! Der Server: nimmt Spieler über UDP an, simuliert mit festen 60 Ticks/s und schickt jedem
//! Spieler 60 Delta-Snapshots pro Sekunde. Läuft als eigenes Programm (panzer-server) oder
//! im Spiel selbst, wenn man im Menü "Spiel hosten" wählt.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use panzer_core::bot::Bot;
use panzer_core::codec::{write_snap, Reader, Writer};
use panzer_core::consts::*;
use panzer_core::items::Eff;
use panzer_core::net::{split, RelIn, RelOut, MAX_PAYLOAD};
use panzer_core::proto::*;
use panzer_core::rng::Rng;
use panzer_core::snap::NetSnap;
use panzer_core::text::{clean_chat, clean_name};
use panzer_core::Game;

use crate::bans::{Ban, Bans};
use crate::guard::{Guard, Verdict};

/// so lange ohne Paket, dann gilt ein Client als weg
const TIMEOUT: Duration = Duration::from_secs(8);
/// so viele Snapshots merkt sich der Server pro Client als mögliche Delta-Basis
const HISTORY: usize = 64;

pub struct Config {
    pub port: u16,
    pub bind: IpAddr,
    /// freie Plätze mit so vielen Bots auffüllen
    pub bots: usize,
    /// welche Sonderregeln die Runden bekommen
    pub rule: RoundRule,
    /// wer diesen Schlüssel im Hello mitschickt, ist Admin (0 = keiner)
    pub host_key: u64,
    /// Passwort für /login (dedizierter Server)
    pub admin_pass: Option<String>,
    pub bans_file: Option<PathBuf>,
    /// Logzeilen ausgeben
    pub verbose: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            port: PORT,
            bind: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            bots: 0,
            rule: RoundRule::default(),
            host_key: 0,
            admin_pass: None,
            bans_file: Some(PathBuf::from("bans.txt")),
            verbose: true,
        }
    }
}

struct Client {
    addr: SocketAddr,
    session: u32,
    slot: usize,
    name: String,
    token: u64,
    admin: bool,
    muted: bool,
    rel_out: RelOut,
    rel_in: RelIn,
    guard: Guard,
    snap_seq: u32,
    history: VecDeque<(u32, NetSnap, Instant)>,
    /// letzter Snapshot, den der Client bestätigt hat
    acked: u32,
    ping: f64,
    last_heard: Instant,
    maze_sent: Option<(u32, u16)>,
    /// frisch beigetreten: startet bei 0 Punkten
    fresh: bool,
    /// spielt in der laufenden Runde mit (sonst Warteschlange bis zur nächsten Runde)
    in_game: bool,
}

pub struct Server {
    sock: UdpSocket,
    cfg: Config,
    clients: Vec<Client>,
    bots: BTreeMap<usize, Bot>,
    game: Option<Game>,
    roster_dirty: bool,
    bans: Bans,
    rng: Rng,
    start: Instant,
    hellos: HashMap<IpAddr, (Instant, u32)>,
    console: Option<Receiver<String>>,
    players_timer: f64,
    review_timer: f64,
    /// Runde, für die in_game/Bots zuletzt abgeglichen wurden
    round_seen: u32,
    /// kodiertes Labyrinth der laufenden Runde ((maze_id, rev), Bytes); wird nur bei
    /// Änderung neu geklont und kodiert, nicht jeden Tick
    maze_msg: Option<((u32, u16), Vec<u8>)>,
    /// Schreibpuffer für Snapshots und Fragmente (verhindert pro Tick neue Allokationen)
    wbuf: Writer,
    fbuf: Writer,
    /// Eingaben eines Ticks (take_tick_into)
    cmd_buf: Vec<(u32, Cmd, f64)>,
}

/// wen ein Befehl meint: Konsole oder Spieler (Index in clients)
#[derive(Clone, Copy)]
enum Who {
    Console,
    Client(u32),
}

impl Server {
    pub fn bind(cfg: Config) -> io::Result<Server> {
        let sock = UdpSocket::bind(SocketAddr::new(cfg.bind, cfg.port))?;
        sock.set_nonblocking(true)?;
        let bans = Bans::load(cfg.bans_file.clone());
        Ok(Server {
            sock,
            cfg,
            clients: Vec::new(),
            bots: BTreeMap::new(),
            game: None,
            roster_dirty: true,
            bans,
            rng: Rng::from_entropy(),
            start: Instant::now(),
            hellos: HashMap::new(),
            console: None,
            players_timer: 0.0,
            review_timer: 0.0,
            round_seen: 0,
            maze_msg: None,
            wbuf: Writer { buf: Vec::with_capacity(1536) },
            fbuf: Writer { buf: Vec::with_capacity(1536) },
            cmd_buf: Vec::new(),
        })
    }

    pub fn local_port(&self) -> u16 {
        self.sock.local_addr().map(|a| a.port()).unwrap_or(self.cfg.port)
    }

    /// Zeilen von der Konsole (stdin) als Admin-Befehle
    pub fn set_console(&mut self, rx: Receiver<String>) {
        self.console = Some(rx);
    }

    fn log(&self, msg: &str) {
        if self.cfg.verbose {
            let s = self.start.elapsed().as_secs();
            println!("[{:02}:{:02}:{:02}] {msg}", s / 3600, s / 60 % 60, s % 60);
        }
    }

    /// Hauptschleife, bis stop gesetzt wird
    pub fn run(&mut self, stop: &AtomicBool) {
        self.log(&format!("Server läuft auf UDP-Port {} ({} Bots)", self.local_port(), self.cfg.bots));
        let tick = Duration::from_secs_f64(TICK);
        let mut next = Instant::now();
        while !stop.load(Ordering::Relaxed) {
            let now = Instant::now();
            if now < next {
                let wait = next - now;
                if wait > Duration::from_millis(2) {
                    std::thread::sleep(wait - Duration::from_millis(1));
                } else {
                    std::thread::yield_now();
                }
                // in der Wartezeit schon Pakete annehmen (weniger Latenz)
                self.recv_all(Instant::now());
                continue;
            }
            next += tick;
            if now.duration_since(next) > Duration::from_millis(250) {
                next = now + tick; // Rechner hing: nicht alles nachholen
            }
            self.tick(now);
        }
        for c in &self.clients {
            let _ = self.sock.send_to(&encode_reason(S_KICK, "Server wurde beendet."), c.addr);
        }
        self.log("Server beendet");
    }

    fn tick(&mut self, now: Instant) {
        self.recv_all(now);
        self.console_commands();
        // Zeitüberschreitung
        let gone: Vec<u32> =
            self.clients.iter().filter(|c| now.duration_since(c.last_heard) > TIMEOUT).map(|c| c.session).collect();
        for s in gone {
            self.remove(s, "Zeitüberschreitung", false);
        }
        if self.roster_dirty {
            self.update_roster();
        }
        // Eingaben im Takt abarbeiten (Zeitbudget gegen Speedhacks)
        if let Some(game) = self.game.as_mut() {
            for c in self.clients.iter_mut() {
                c.guard.take_tick_into(&mut self.cmd_buf);
                for &(_, cmd, dt) in self.cmd_buf.iter() {
                    if c.in_game {
                        game.apply_input(c.slot, cmd.keys, cmd.fires, dt);
                    }
                }
            }
            // Slots der mitspielenden Menschen als Bitmaske (statt Vec pro Tick)
            let mut human_bits = 0u64;
            for c in self.clients.iter() {
                if c.in_game {
                    human_bits |= 1u64 << c.slot;
                }
            }
            for (&slot, bot) in self.bots.iter_mut() {
                if game.players.contains(&slot) && human_bits & (1u64 << slot) == 0 {
                    let (k, f) = bot.think(game);
                    game.apply_input(slot, k, f, TICK);
                }
            }
            game.step(TICK);
            if game.maze_id != self.round_seen {
                self.apply_round();
            }
        } else {
            for c in self.clients.iter_mut() {
                c.guard.take_tick_into(&mut self.cmd_buf);
            }
        }
        self.review_timer += TICK;
        if self.review_timer >= 1.0 {
            self.review_timer = 0.0;
            self.review(now);
        }
        self.players_timer -= TICK;
        if self.players_timer <= 0.0 {
            self.players_timer = 2.0;
            self.send_players();
        }
        self.send_snapshots(now);
    }

    // --- Empfangen ---

    fn recv_all(&mut self, now: Instant) {
        let mut buf = [0u8; 2048];
        loop {
            match self.sock.recv_from(&mut buf) {
                Ok((n, addr)) => self.handle(&buf[..n], addr, now),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                // Windows meldet ICMP "Port unreachable" als ConnectionReset: ignorieren
                Err(_) => continue,
            }
        }
    }

    fn client_index(&self, addr: SocketAddr) -> Option<usize> {
        self.clients.iter().position(|c| c.addr == addr)
    }

    fn handle(&mut self, data: &[u8], addr: SocketAddr, now: Instant) {
        let mut r = Reader::new(data);
        let Ok(kind) = r.u8() else { return };
        match kind {
            C_HELLO => {
                if let Ok(h) = Hello::decode(&mut r) {
                    self.hello(addr, h, now);
                }
            }
            C_INPUT => {
                let Some(ci) = self.client_index(addr) else {
                    // Server neu gestartet o. Ä.: Client soll sich neu verbinden
                    let _ = self.sock.send_to(&encode_reason(S_KICK, "Unbekannte Sitzung, bitte neu verbinden."), addr);
                    return;
                };
                if !self.clients[ci].guard.packet(now) {
                    return;
                }
                match Input::decode(&mut r) {
                    Ok(inp) if inp.session == self.clients[ci].session => self.input(ci, inp, now),
                    Ok(_) => {}
                    Err(_) => {
                        if let Verdict::Warn(w) = self.clients[ci].guard.bad_packet() {
                            let name = self.clients[ci].name.clone();
                            self.log(&format!("Anti-Cheat: {name}: {w}"));
                        }
                    }
                }
            }
            C_BYE => {
                if let Some(ci) = self.client_index(addr) {
                    if r.u32().ok() == Some(self.clients[ci].session) {
                        let s = self.clients[ci].session;
                        self.remove(s, "hat das Spiel verlassen", false);
                    }
                }
            }
            _ => {}
        }
    }

    fn reject(&self, addr: SocketAddr, reason: &str) {
        let _ = self.sock.send_to(&encode_reason(S_REJECT, reason), addr);
    }

    fn hello(&mut self, addr: SocketAddr, h: Hello, now: Instant) {
        // höchstens 5 Verbindungsversuche pro Sekunde und IP
        let e = self.hellos.entry(addr.ip()).or_insert((now, 0));
        if now.duration_since(e.0) > Duration::from_secs(1) {
            *e = (now, 0);
        }
        e.1 += 1;
        if e.1 > 5 {
            return;
        }
        if self.hellos.len() > 1024 {
            self.hellos.retain(|_, v| now.duration_since(v.0) < Duration::from_secs(5));
        }
        // Welcome ging verloren: nochmal schicken
        if let Some(ci) = self.client_index(addr) {
            let c = &self.clients[ci];
            if c.token == h.token {
                let w = Welcome { session: c.session, slot: c.slot as u8, admin: c.admin, name: c.name.clone() };
                let _ = self.sock.send_to(&w.encode(), addr);
                return;
            }
            let s = c.session;
            self.remove(s, "hat sich neu verbunden", false);
        }
        if h.proto != PROTO {
            return self.reject(addr, &format!("Andere Spielversion (Server {PROTO}, du {}).", h.proto));
        }
        if let Some(b) = self.bans.find(addr.ip(), h.token) {
            let why = if b.reason.is_empty() { String::new() } else { format!(": {}", b.reason) };
            self.log(&format!("Gebannter Spieler {} ({}) abgewiesen", h.name, addr.ip()));
            return self.reject(addr, &format!("Du bist auf diesem Server gebannt{why}"));
        }
        let Some(mut name) = clean_name(&h.name) else {
            return self.reject(addr, "Bitte gib einen gültigen Namen ein (1–16 Zeichen).");
        };
        let taken = |n: &str, cl: &[Client]| cl.iter().any(|c| c.name.to_lowercase() == n.to_lowercase());
        if taken(&name, &self.clients) {
            let base: String = name.chars().take(14).collect();
            for k in 2..20 {
                let cand = format!("{base}{k}");
                if !taken(&cand, &self.clients) {
                    name = cand;
                    break;
                }
            }
        }
        let used: Vec<usize> = self.clients.iter().map(|c| c.slot).collect();
        let Some(slot) = (0..MAX_PLAYERS).find(|s| !used.contains(s)) else {
            return self.reject(addr, &format!("Der Server ist voll ({MAX_PLAYERS} Spieler)."));
        };
        let admin = self.cfg.host_key != 0 && h.host_key == self.cfg.host_key;
        let mut session = self.rng.next_u64() as u32;
        if session == 0 {
            session = 1;
        }
        let w = Welcome { session, slot: slot as u8, admin, name: name.clone() };
        let _ = self.sock.send_to(&w.encode(), addr);
        self.log(&format!("{name} ist beigetreten ({}, Slot {})", addr, slot + 1));
        self.clients.push(Client {
            addr,
            session,
            slot,
            name: name.clone(),
            token: h.token,
            admin,
            muted: false,
            rel_out: RelOut::new(),
            rel_in: RelIn::default(),
            guard: Guard::new(now),
            snap_seq: 0,
            history: VecDeque::new(),
            acked: 0,
            ping: 0.0,
            last_heard: now,
            maze_sent: None,
            fresh: true,
            in_game: false,
        });
        self.roster_dirty = true;
        self.system(&format!("{name} ist beigetreten"));
        if self.game.as_ref().is_some_and(|g| g.players.len() >= 2) {
            self.tell(
                session,
                "Die Runde läuft schon – du bist in der Warteschlange und spielst ab der nächsten Runde mit.",
            );
        }
        if self.clients.len() == 1 && self.bots.is_empty() && self.cfg.bots == 0 {
            self.tell(session, "Warte auf Mitspieler … (Admins: /bots 3 füllt mit Bots auf)");
        }
    }

    fn input(&mut self, ci: usize, inp: Input, now: Instant) {
        let c = &mut self.clients[ci];
        c.last_heard = now;
        c.rel_out.ack(inp.rel_ack);
        if inp.snap_ack > c.acked {
            c.acked = inp.snap_ack;
            if let Some((_, _, sent)) = c.history.iter().find(|h| h.0 == inp.snap_ack) {
                let rtt = now.duration_since(*sent).as_secs_f64() * 1000.0;
                c.ping = if c.ping == 0.0 { rtt } else { c.ping * 0.9 + rtt * 0.1 };
            }
        }
        c.guard.accept(inp.first_seq, &inp.cmds);
        let session = c.session;
        let mut msgs = Vec::new();
        for (id, data) in inp.rel {
            match self.clients[ci].rel_in.receive(id, data) {
                Ok(m) => msgs.extend(m),
                Err(()) => {
                    self.remove(session, "Anti-Cheat: Nachrichtenflut", false);
                    return;
                }
            }
        }
        for m in msgs {
            match Msg::decode(&m) {
                Ok(Msg::Say(text)) => self.say(session, &text, now),
                _ => {
                    if let Some(ci) = self.find_session(session) {
                        let _ = self.clients[ci].guard.bad_packet();
                    }
                }
            }
        }
    }

    fn find_session(&self, session: u32) -> Option<usize> {
        self.clients.iter().position(|c| c.session == session)
    }

    // --- Chat und Befehle ---

    fn say(&mut self, session: u32, text: &str, now: Instant) {
        let Some(ci) = self.find_session(session) else { return };
        let Some(text) = clean_chat(text) else { return };
        if let Some(cmd) = text.strip_prefix('/') {
            let cmd = cmd.to_string();
            return self.command(Who::Client(session), &cmd);
        }
        if !self.clients[ci].guard.chat_ok(now) {
            return self.tell(session, "Nicht so schnell! (max. 5 Nachrichten in 5 s)");
        }
        if self.clients[ci].muted {
            return self.tell(session, "Du bist stummgeschaltet.");
        }
        let (slot, name) = (self.clients[ci].slot, self.clients[ci].name.clone());
        self.log(&format!("<{name}> {text}"));
        let msg = Msg::Chat { from: Some(slot as u8), text }.encode();
        for c in self.clients.iter_mut() {
            c.rel_out.push(msg.clone());
        }
    }

    /// Systemnachricht an alle
    fn system(&mut self, text: &str) {
        let msg = Msg::Chat { from: None, text: text.to_string() }.encode();
        for c in self.clients.iter_mut() {
            c.rel_out.push(msg.clone());
        }
    }

    /// Systemnachricht an einen
    fn tell(&mut self, session: u32, text: &str) {
        if let Some(ci) = self.find_session(session) {
            self.clients[ci].rel_out.push(Msg::Chat { from: None, text: text.to_string() }.encode());
        }
    }

    fn reply(&mut self, who: Who, text: &str) {
        match who {
            Who::Console => println!("{text}"),
            Who::Client(s) => self.tell(s, text),
        }
    }

    fn console_commands(&mut self) {
        let mut lines = Vec::new();
        if let Some(rx) = &self.console {
            while let Ok(l) = rx.try_recv() {
                lines.push(l);
            }
        }
        for l in lines {
            let l = l.trim().trim_start_matches('/').to_string();
            if !l.is_empty() {
                self.command(Who::Console, &l);
            }
        }
    }

    /// Spieler über Nummer (#3 / 3) oder Name(nsanfang) finden
    fn target(&self, what: &str) -> Result<usize, String> {
        let w = what.trim().trim_start_matches('#');
        if let Ok(n) = w.parse::<usize>() {
            if let Some(ci) = self.clients.iter().position(|c| c.slot + 1 == n) {
                return Ok(ci);
            }
        }
        let lower = w.to_lowercase();
        if let Some(ci) = self.clients.iter().position(|c| c.name.to_lowercase() == lower) {
            return Ok(ci);
        }
        let hits: Vec<usize> =
            (0..self.clients.len()).filter(|&i| self.clients[i].name.to_lowercase().starts_with(&lower)).collect();
        match hits.len() {
            1 => Ok(hits[0]),
            0 => Err(format!("Kein Spieler „{w}“ gefunden.")),
            _ => Err(format!("„{w}“ ist nicht eindeutig.")),
        }
    }

    fn command(&mut self, who: Who, line: &str) {
        let mut parts = line.splitn(2, ' ');
        let cmd = parts.next().unwrap_or("").to_lowercase();
        let arg = parts.next().unwrap_or("").trim().to_string();
        let admin = match who {
            Who::Console => true,
            Who::Client(s) => self.find_session(s).is_some_and(|ci| self.clients[ci].admin),
        };
        let me = match who {
            Who::Client(s) => self.find_session(s),
            Who::Console => None,
        };
        let need_admin = ["kick", "ban", "unban", "bans", "bots", "mute", "unmute", "say"];
        if need_admin.contains(&cmd.as_str()) && !admin {
            return self.reply(who, "Das darf nur der Host bzw. ein Admin (/login <Passwort>).");
        }
        match cmd.as_str() {
            "help" | "hilfe" | "?" => {
                let mut t = String::from("Befehle: /players, /login <pw>");
                if admin {
                    t.push_str(", /kick <spieler> [grund], /ban <spieler> [grund], /unban <name|ip|nr>, /bans, /mute <spieler>, /unmute <spieler>, /bots <0-7>");
                }
                self.reply(who, &t);
            }
            "players" | "spieler" | "list" => {
                let lines: Vec<String> = self
                    .clients
                    .iter()
                    .map(|c| {
                        let mut s = format!("#{} {} – {:.0} ms", c.slot + 1, c.name, c.ping);
                        if c.admin {
                            s.push_str(" (Admin)");
                        }
                        if admin {
                            s.push_str(&format!(" – {}", c.addr.ip()));
                        }
                        s
                    })
                    .collect();
                let bots = self.bots.len();
                self.reply(who, &format!("{} Spieler, {bots} Bots", lines.len()));
                for l in lines {
                    self.reply(who, &l);
                }
            }
            "login" => {
                let Some(ci) = me else { return };
                match &self.cfg.admin_pass {
                    Some(p) if !p.is_empty() && *p == arg => {
                        self.clients[ci].admin = true;
                        let name = self.clients[ci].name.clone();
                        self.log(&format!("{name} ist jetzt Admin"));
                        self.reply(who, "Du bist jetzt Admin.");
                        self.send_players();
                    }
                    _ => {
                        let name = self.clients[ci].name.clone();
                        self.log(&format!("Falsches Admin-Passwort von {name}"));
                        self.clients[ci].guard.strikes += 0.5;
                        self.reply(who, "Falsches Passwort.");
                    }
                }
            }
            "kick" | "ban" => {
                let mut ap = arg.splitn(2, ' ');
                let target = ap.next().unwrap_or("");
                let reason = ap.next().unwrap_or("").trim().to_string();
                if target.is_empty() {
                    return self.reply(who, &format!("/{cmd} <spieler> [grund]"));
                }
                match self.target(target) {
                    Ok(ci) if Some(ci) == me => self.reply(who, "Dich selbst kannst du nicht rauswerfen."),
                    Ok(ci) => {
                        let s = self.clients[ci].session;
                        let why = if reason.is_empty() {
                            if cmd == "ban" {
                                "vom Host gebannt".to_string()
                            } else {
                                "vom Host gekickt".to_string()
                            }
                        } else {
                            reason
                        };
                        self.remove(s, &why, cmd == "ban");
                    }
                    Err(e) => self.reply(who, &e),
                }
            }
            "unban" => {
                let gone = self.bans.remove(&arg);
                if gone.is_empty() {
                    self.reply(who, "Nichts gefunden. /bans zeigt die Liste.");
                } else {
                    for b in gone {
                        self.log(&format!("Entbannt: {} ({})", b.name, b.ip));
                        self.reply(who, &format!("Entbannt: {} ({})", b.name, b.ip));
                    }
                }
            }
            "bans" => {
                if self.bans.list.is_empty() {
                    return self.reply(who, "Niemand gebannt.");
                }
                let lines: Vec<String> = self
                    .bans
                    .list
                    .iter()
                    .enumerate()
                    .map(|(i, b)| format!("{}. {} ({}) {}", i + 1, b.name, b.ip, b.reason))
                    .collect();
                for l in lines {
                    self.reply(who, &l);
                }
            }
            "mute" | "unmute" => match self.target(&arg) {
                Ok(ci) => {
                    self.clients[ci].muted = cmd == "mute";
                    let name = self.clients[ci].name.clone();
                    let t = if cmd == "mute" { "stummgeschaltet" } else { "darf wieder schreiben" };
                    self.system(&format!("{name} {t}"));
                }
                Err(e) => self.reply(who, &e),
            },
            "bots" => match arg.parse::<usize>() {
                Ok(n) if n < MAX_PLAYERS => {
                    self.cfg.bots = n;
                    self.roster_dirty = true;
                    self.system(&format!("Bots: {n}"));
                }
                _ => self.reply(who, "/bots <0-7>"),
            },
            "say" => {
                if !arg.is_empty() {
                    self.system(&format!("[Server] {arg}"));
                }
            }
            "" => {}
            _ => self.reply(who, "Unbekannter Befehl. /help zeigt alle."),
        }
    }

    /// Client entfernen (Kick, Bann, Verlassen, Zeitüberschreitung)
    fn remove(&mut self, session: u32, reason: &str, ban: bool) {
        let Some(ci) = self.find_session(session) else { return };
        let c = self.clients.remove(ci);
        if c.in_game {
            if let Some(g) = self.game.as_mut() {
                g.remove_player(c.slot); // die Runde läuft weiter
            }
        }
        let packet = encode_reason(S_KICK, reason);
        if reason != "hat das Spiel verlassen" {
            for _ in 0..3 {
                let _ = self.sock.send_to(&packet, c.addr);
            }
        }
        if ban {
            self.bans.add(Ban { ip: c.addr.ip(), token: c.token, name: c.name.clone(), reason: reason.to_string() });
            self.log(&format!("{} gebannt ({}): {reason}", c.name, c.addr.ip()));
            self.system(&format!("{} wurde gebannt ({reason})", c.name));
        } else if reason == "hat das Spiel verlassen" {
            self.log(&format!("{} hat das Spiel verlassen", c.name));
            self.system(&format!("{} hat das Spiel verlassen", c.name));
        } else {
            self.log(&format!("{} entfernt: {reason}", c.name));
            self.system(&format!("{} ist raus ({reason})", c.name));
        }
        self.roster_dirty = true;
    }

    fn review(&mut self, now: Instant) {
        let mut kick = Vec::new();
        for c in self.clients.iter_mut() {
            if let Verdict::Warn(w) = c.guard.review(now) {
                if self.cfg.verbose {
                    println!("Anti-Cheat: {} ({}): {w} – Verwarnung {}", c.name, c.addr.ip(), c.guard.strikes);
                }
            }
            if c.guard.should_kick() {
                kick.push(c.session);
            }
        }
        for s in kick {
            self.remove(s, "Anti-Cheat: verdächtiges Verhalten", false);
        }
    }

    // --- Spielerliste ---

    /// Wer spielt mit? Beitreten und Verlassen starten keine neue Runde: Neue warten in
    /// der Warteschlange, bis die nächste Runde beginnt.
    fn update_roster(&mut self) {
        self.roster_dirty = false;
        let used: Vec<usize> = self.clients.iter().map(|c| c.slot).collect();
        // Bots nehmen die hohen freien Plätze
        let want_bots: Vec<usize> = (0..MAX_PLAYERS).rev().filter(|s| !used.contains(s)).take(self.cfg.bots).collect();
        let mut fresh: Vec<usize> = Vec::new();
        for &s in &want_bots {
            if !self.bots.contains_key(&s) {
                self.bots.insert(s, Bot::new(s, None));
                fresh.push(s);
            }
        }
        for c in self.clients.iter_mut() {
            if c.fresh {
                c.fresh = false;
                fresh.push(c.slot);
            }
        }
        let mut players: Vec<usize> = used.iter().copied().chain(want_bots.iter().copied()).collect();
        players.sort();
        if players.len() < 2 {
            // allein: zurück in die Lobby
            self.game = None;
            for c in self.clients.iter_mut() {
                c.in_game = false;
            }
            self.bots.retain(|s, _| want_bots.contains(s));
        } else {
            match self.game.as_mut() {
                None => {
                    let mut g = Game::with_rule(&players, self.cfg.rule);
                    g.time = self.start.elapsed().as_secs_f64(); // Serverzeit läuft durch
                    self.game = Some(g);
                    self.apply_round();
                }
                Some(g) if g.players.len() < 2 => {
                    // es lief nur noch einer: sofort mit allen weiter
                    g.set_players(&players, &fresh);
                    self.apply_round();
                }
                Some(g) => {
                    g.queue_players(&players, &fresh);
                }
            }
        }
        for c in self.clients.iter_mut() {
            c.maze_sent = None;
        }
        self.send_players();
    }

    /// Neue Runde hat begonnen: Warteschlange rein, alte Bots raus
    fn apply_round(&mut self) {
        let Some(g) = self.game.as_mut() else { return };
        self.round_seen = g.maze_id;
        for c in self.clients.iter_mut() {
            c.in_game = g.players.contains(&c.slot);
        }
        let humans: Vec<usize> = self.clients.iter().map(|c| c.slot).collect();
        self.bots.retain(|s, _| g.players.contains(s) && !humans.contains(s));
        g.bots = self.bots.keys().copied().collect();
        self.send_players();
    }

    fn send_players(&mut self) {
        let mut list: Vec<PlayerInfo> = self
            .clients
            .iter()
            .map(|c| PlayerInfo {
                slot: c.slot as u8,
                name: c.name.clone(),
                bot: false,
                admin: c.admin,
                queued: !c.in_game,
                ping: c.ping.round().min(9999.0) as u16,
            })
            .collect();
        for &s in self.bots.keys() {
            if !list.iter().any(|p| p.slot as usize == s) {
                list.push(PlayerInfo {
                    slot: s as u8,
                    name: format!("{} (Bot)", COLOR_NAMES[s]),
                    bot: true,
                    admin: false,
                    queued: false,
                    ping: 0,
                });
            }
        }
        list.sort_by_key(|p| p.slot);
        let msg = Msg::Players(list).encode();
        for c in self.clients.iter_mut() {
            c.rel_out.push(msg.clone());
        }
    }

    // --- Senden ---

    fn send_snapshots(&mut self, now: Instant) {
        let lobby_time = (self.start.elapsed().as_secs_f64() * 1000.0) as u32;
        // Labyrinth nur bei Änderung neu kodieren (früher: jeder Tick Klonen + Encodieren)
        let key = self.game.as_ref().map(|g| (g.maze_id, g.maze.rev));
        if key != self.maze_msg.as_ref().map(|m| m.0) {
            self.maze_msg = match key {
                Some(k) => {
                    let g = self.game.as_ref().unwrap();
                    Some((k, Msg::Maze(g.maze.clone()).encode()))
                }
                None => None,
            };
        }
        // Ohne Nebel und Tarnkappen ist der Snapshot für alle gleich: einmal bauen und
        // kopieren statt pro Client neu rechnen (mit Verstecktem gilt das nicht, dort
        // wie bisher pro Client)
        let shared = self.game.as_ref().and_then(|g| {
            let hides = g.mutator == Some(Mutator::Nebel)
                || g.tanks.iter().any(|t| t.alive && t.has(Eff::Invis));
            (!hides).then(|| g.net_snap(None, true))
        });
        for ci in 0..self.clients.len() {
            let c = &mut self.clients[ci];
            if let Some((key, data)) = &self.maze_msg {
                if c.maze_sent != Some(*key) {
                    c.maze_sent = Some(*key);
                    c.rel_out.push(data.clone());
                }
            }
            let mut snap = match (&self.game, &shared) {
                // Wartende schauen zu, sehen aber auch nichts Verstecktes
                (Some(_), Some(s)) if c.in_game => s.clone(),
                (Some(_), Some(s)) => NetSnap { queued: true, ..s.clone() },
                (Some(g), None) if c.in_game => g.net_snap(Some(c.slot), true),
                (Some(g), None) => NetSnap { queued: true, ..g.net_snap(None, true) },
                (None, _) => NetSnap { time: lobby_time, et: u16::MAX, ..Default::default() },
            };
            snap.ack = c.guard.done_seq;
            c.snap_seq += 1;
            let seq = c.snap_seq;
            let base = c.history.iter().find(|h| h.0 == c.acked).map(|h| (h.0, &h.1));
            let head = SnapHead {
                seq,
                base: base.map_or(0, |b| b.0),
                rel_ack: c.rel_in.ack(),
                rel: c.rel_out.collect(now, 900),
            };
            let w = &mut self.wbuf;
            w.buf.clear();
            w.u8(S_SNAP);
            head.write(w);
            write_snap(w, &snap, base.map(|b| b.1));
            if w.len() <= MAX_PAYLOAD + 64 {
                let _ = self.sock.send_to(&w.buf, c.addr);
            } else {
                // zu groß für ein Paket: in Stücke
                let body = &w.buf[1..];
                let parts = split(body);
                for (i, p) in parts.iter().enumerate() {
                    let f = &mut self.fbuf;
                    f.buf.clear();
                    f.u8(S_FRAG);
                    f.var(seq as u64);
                    f.u8(i as u8);
                    f.u8(parts.len() as u8);
                    f.raw(p);
                    let _ = self.sock.send_to(&f.buf, c.addr);
                }
            }
            c.history.push_back((seq, snap, now));
            while c.history.len() > HISTORY {
                c.history.pop_front();
            }
        }
    }
}
