//! Netzwerk-Client: UDP, Delta-Snapshots, Prediction des eigenen Panzers und
//! Interpolation der anderen.

use std::collections::VecDeque;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::time::Instant;

use panzer_core::codec::{read_snap, Reader};
use panzer_core::consts::*;
use panzer_core::net::{Reassembly, RelIn, RelOut};
use panzer_core::physics::{move_tank, Body};
use panzer_core::proto::*;
use panzer_core::snap::{NetSnap, View};
use panzer_core::{Arena, Maze};

use crate::render::Roster;

pub enum NetState {
    Connecting,
    Connected,
    /// getrennt, mit Grund
    Closed(String),
}

pub struct ChatLine {
    pub from: Option<u8>,
    pub text: String,
    pub time: f64,
}

pub struct NetClient {
    sock: UdpSocket,
    server: SocketAddr,
    pub state: NetState,
    hello: Hello,
    last_hello: f64,
    started: Instant,
    session: u32,
    pub slot: usize,
    pub admin: bool,
    pub name: String,
    snaps: VecDeque<(u32, NetSnap)>,
    views: VecDeque<View>,
    latest: u32,
    pub maze: Option<Maze>,
    pub arena: Option<Arena>,
    pub roster: Roster,
    pub queued_list: [bool; MAX_PLAYERS],
    pub chat: Vec<ChatLine>,
    rel_out: RelOut,
    rel_in: RelIn,
    reasm: Reassembly,
    offset: Option<f64>,
    seq: u32,
    pending: VecDeque<(u32, Cmd, f64)>,
    pub ping: Option<f64>,
    last_recv: f64,
    last_send: f64,
    /// Statistik für F3: Bytes pro Sekunde rein/raus
    pub rate_in: f64,
    pub rate_out: f64,
    bytes_in: usize,
    bytes_out: usize,
    rate_t: f64,
}

impl NetClient {
    /// addr = host[:port]
    pub fn connect(addr: &str, name: &str, token: u64, host_key: u64) -> Result<NetClient, String> {
        let addr = addr.trim();
        let with_port =
            if addr.rsplit_once(':').is_some_and(|(_, p)| p.parse::<u16>().is_ok()) && addr.matches(':').count() == 1 {
                addr.to_string()
            } else {
                format!("{addr}:{PORT}")
            };
        let mut addrs: Vec<SocketAddr> =
            with_port.to_socket_addrs().map_err(|e| format!("Adresse unbekannt: {e}"))?.collect();
        addrs.sort_by_key(|a| !a.is_ipv4());
        let server = *addrs.first().ok_or("Adresse unbekannt")?;
        let bind = if server.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
        let sock = UdpSocket::bind(bind).map_err(|e| e.to_string())?;
        sock.set_nonblocking(true).map_err(|e| e.to_string())?;
        let mut c = NetClient {
            sock,
            server,
            state: NetState::Connecting,
            hello: Hello { proto: PROTO, name: name.to_string(), token, host_key },
            last_hello: -1.0,
            started: Instant::now(),
            session: 0,
            slot: 0,
            admin: false,
            name: name.to_string(),
            snaps: VecDeque::new(),
            views: VecDeque::new(),
            latest: 0,
            maze: None,
            arena: None,
            roster: Roster::local(),
            queued_list: [false; MAX_PLAYERS],
            chat: Vec::new(),
            rel_out: RelOut::new(),
            rel_in: RelIn::default(),
            reasm: Reassembly::default(),
            offset: None,
            seq: 0,
            pending: VecDeque::new(),
            ping: None,
            last_recv: 0.0,
            last_send: -1.0,
            rate_in: 0.0,
            rate_out: 0.0,
            bytes_in: 0,
            bytes_out: 0,
            rate_t: 0.0,
        };
        c.roster.names = Default::default();
        c.update();
        Ok(c)
    }

    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// Uhr des Clients (für Chat-Zeitstempel)
    pub fn clock(&self) -> f64 {
        self.now()
    }

    fn send(&mut self, data: &[u8]) {
        self.bytes_out += data.len() + 28;
        let _ = self.sock.send_to(data, self.server);
    }

    pub fn bye(&mut self) {
        if self.session != 0 {
            let mut w = panzer_core::codec::Writer::new();
            w.u8(C_BYE);
            w.u32(self.session);
            for _ in 0..3 {
                self.send(&w.buf.clone());
            }
        }
    }

    /// Pakete abholen, Verbindung pflegen. Jeden Frame aufrufen.
    pub fn update(&mut self) {
        let now = self.now();
        if let NetState::Connecting = self.state {
            if now - self.last_hello > 0.25 {
                self.last_hello = now;
                let h = self.hello.encode();
                self.send(&h);
            }
            if now > 6.0 {
                self.state = NetState::Closed("Keine Antwort vom Server. IP und Port richtig? Läuft er?".into());
            }
        }
        let mut buf = [0u8; 2048];
        loop {
            match self.sock.recv_from(&mut buf) {
                Ok((n, from)) => {
                    if from.ip() != self.server.ip() && !from.ip().is_loopback() {
                        continue;
                    }
                    self.bytes_in += n + 28;
                    self.last_recv = now;
                    let data = buf[..n].to_vec();
                    self.handle(&data, now);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => continue,
            }
        }
        if matches!(self.state, NetState::Connected) && now - self.last_recv > 6.0 {
            self.state = NetState::Closed("Verbindung zum Server verloren.".into());
        }
        if now - self.rate_t >= 1.0 {
            let span = now - self.rate_t;
            self.rate_in = self.bytes_in as f64 / span;
            self.rate_out = self.bytes_out as f64 / span;
            self.bytes_in = 0;
            self.bytes_out = 0;
            self.rate_t = now;
        }
        self.chat.retain(|c| now - c.time < 60.0);
        if self.chat.len() > 40 {
            let cut = self.chat.len() - 40;
            self.chat.drain(..cut);
        }
    }

    fn handle(&mut self, data: &[u8], now: f64) {
        let mut r = Reader::new(data);
        let Ok(kind) = r.u8() else { return };
        match kind {
            S_WELCOME => match Welcome::decode(&mut r) {
                Ok(Ok(w)) => {
                    if matches!(self.state, NetState::Connecting) {
                        self.session = w.session;
                        self.slot = w.slot as usize;
                        self.admin = w.admin;
                        self.name = w.name;
                        self.state = NetState::Connected;
                    }
                }
                Ok(Err(msg)) => self.state = NetState::Closed(msg),
                Err(_) => {}
            },
            S_REJECT | S_KICK => {
                let reason = r.str(400).unwrap_or_default();
                if kind == S_KICK && matches!(self.state, NetState::Connecting) {
                    return; // altes Kick-Paket einer früheren Sitzung
                }
                if matches!(self.state, NetState::Closed(_)) {
                    return; // der erste Grund zählt
                }
                self.state =
                    NetState::Closed(if kind == S_KICK { format!("Vom Server getrennt: {reason}") } else { reason });
            }
            S_SNAP => self.snap_body(r.rest(), now),
            S_FRAG => {
                let (Ok(seq), Ok(idx), Ok(count)) = (r.var32(), r.u8(), r.u8()) else { return };
                if let Some(body) = self.reasm.add(seq, idx, count, r.rest()) {
                    self.snap_body(&body, now);
                }
            }
            _ => {}
        }
    }

    fn snap_body(&mut self, body: &[u8], now: f64) {
        let mut r = Reader::new(body);
        let Ok(head) = SnapHead::read(&mut r) else { return };
        self.rel_out.ack(head.rel_ack);
        for (id, data) in head.rel {
            if let Ok(msgs) = self.rel_in.receive(id, data) {
                for m in msgs {
                    if let Ok(msg) = Msg::decode(&m) {
                        self.message(msg, now);
                    }
                }
            }
        }
        if !matches!(self.state, NetState::Connected) {
            return;
        }
        if head.seq <= self.latest {
            return; // veraltet oder doppelt
        }
        let base = if head.base == 0 {
            None
        } else {
            match self.snaps.iter().find(|s| s.0 == head.base) {
                Some(b) => Some(&b.1),
                None => return, // Basis fehlt: nächster Snapshot kommt mit passender Basis
            }
        };
        let Ok(snap) = read_snap(&mut r, base) else { return };
        self.latest = head.seq;
        let view = snap.view();
        // Früh ankommende Pakete bestimmen den Zeitversatz, langsam nachdriften
        let off = view.time - now;
        match self.offset {
            Some(o) if off <= o && off > o - 1.0 => self.offset = Some(o + (off - o) * 0.02),
            _ => self.offset = Some(off),
        }
        // Ping über die bestätigte Eingabe
        while let Some(&(seq, _, sent)) = self.pending.front() {
            if seq > snap.ack {
                break;
            }
            if seq == snap.ack {
                let sample = (now - sent) * 1000.0;
                self.ping = Some(self.ping.map_or(sample, |p| p * 0.9 + sample * 0.1));
            }
            self.pending.pop_front();
        }
        self.snaps.push_back((head.seq, snap));
        while self.snaps.len() > 128 {
            self.snaps.pop_front();
        }
        if self.views.back().is_some_and(|v| v.maze_id != view.maze_id) {
            self.views.clear();
        }
        self.views.push_back(view);
        while self.views.len() > 40 {
            self.views.pop_front();
        }
    }

    fn message(&mut self, msg: Msg, now: f64) {
        match msg {
            Msg::Maze(m) => {
                self.arena = Some(Arena::new(&m));
                self.maze = Some(m);
            }
            Msg::Players(list) => {
                let mut r = Roster::local();
                r.names = Default::default();
                self.queued_list = [false; MAX_PLAYERS];
                for p in list {
                    let s = (p.slot as usize).min(MAX_PLAYERS - 1);
                    r.names[s] = p.name;
                    r.ping[s] = p.ping;
                    r.admin[s] = p.admin;
                    self.queued_list[s] = p.queued;
                    if p.bot {
                        r.bots |= 1 << s;
                    }
                }
                self.roster = r;
            }
            Msg::Chat { from, text } => self.chat.push(ChatLine { from, text, time: now }),
            Msg::Say(_) => {}
        }
    }

    /// Chatzeile oder /Befehl an den Server
    pub fn say(&mut self, text: &str) {
        self.rel_out.push(Msg::Say(text.to_string()).encode());
    }

    /// Eingabe dieses Frames verschicken (mit allen unbestätigten davor, falls Pakete fehlen)
    pub fn send_input(&mut self, keys: u8, fires: u16, dt: f64) {
        if !matches!(self.state, NetState::Connected) {
            return;
        }
        let now = self.now();
        let dt_ms = (dt * 1000.0).round().clamp(1.0, 50.0) as u8;
        self.seq += 1;
        self.pending.push_back((self.seq, Cmd { keys, fires, dt_ms }, now));
        while self.pending.len() > 240 {
            self.pending.pop_front();
        }
        // höchstens 125 Pakete/s, die Eingaben gehen trotzdem alle mit
        if now - self.last_send < 0.008 {
            return;
        }
        self.last_send = now;
        let from = self.pending.len().saturating_sub(MAX_CMDS);
        let cmds: Vec<Cmd> = self.pending.iter().skip(from).map(|p| p.1).collect();
        let first_seq = self.pending.get(from).map_or(self.seq, |p| p.0);
        let rel = self.rel_out.collect(Instant::now(), 600);
        let inp =
            Input { session: self.session, snap_ack: self.latest, rel_ack: self.rel_in.ack(), first_seq, cmds, rel };
        let data = inp.encode();
        self.send(&data);
    }

    /// Spielzustand zum Zeichnen: andere interpoliert, der eigene Panzer vorhergesagt
    pub fn view(&self) -> Option<View> {
        let latest = self.views.back()?;
        let arena = self.arena.as_ref()?;
        let render_t = self.now() + self.offset.unwrap_or(0.0) - INTERP_DELAY;
        let mut view = latest.clone();
        for w in self.views.iter().collect::<Vec<_>>().windows(2) {
            if w[0].time <= render_t && render_t <= w[1].time {
                view = View::interpolate(w[0], w[1], render_t);
                break;
            }
        }
        if latest.queued {
            return Some(view);
        }
        // Eigener Panzer: letzter Serverzustand + noch unbestätigte Eingaben -> sofortige Reaktion
        if let Some(me) = latest.tank(self.slot) {
            if me.alive && latest.maze_id == arena.id {
                let mut body = Body { x: me.x, y: me.y, a: me.a };
                for (_, c, _) in &self.pending {
                    move_tank(&mut body, &me.eff, me.charge > 0.0, c.keys, c.dt_ms as f64 / 1000.0, arena);
                }
                let mut mine = me.clone();
                mine.x = body.x;
                mine.y = body.y;
                mine.a = body.a;
                match view.tank_mut(self.slot) {
                    Some(t) => *t = mine,
                    None => view.tanks.push(mine),
                }
            }
        }
        Some(view)
    }

    pub fn queued(&self) -> bool {
        self.views.back().is_some_and(|v| v.queued)
    }
}
