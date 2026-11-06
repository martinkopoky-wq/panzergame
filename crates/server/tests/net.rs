//! Echter Server über UDP-Loopback mit Test-Clients: Handshake, Warteschlange beim
//! Beitreten, Verlassen ohne Neustart, Chat, Kick/Bann, Speedhack-Bremse.

use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use panzer_core::codec::{read_snap, Reader, Writer};
use panzer_core::consts::*;
use panzer_core::net::{Reassembly, RelIn, RelOut};
use panzer_core::proto::*;
use panzer_core::snap::NetSnap;
use panzer_core::Maze;
use panzer_server::{Config, Server};

struct TestClient {
    sock: UdpSocket,
    session: u32,
    slot: usize,
    name: String,
    snaps: Vec<(u32, NetSnap)>,
    latest: u32,
    rel_in: RelIn,
    rel_out: RelOut,
    reasm: Reassembly,
    seq: u32,
    chat: Vec<String>,
    /// Sonderregel jedes empfangenen Labyrinths
    mutators: Vec<Option<Mutator>>,
    mazes: Vec<Maze>,
    kicked: Option<String>,
    bytes: usize,
    packets: usize,
}

impl TestClient {
    fn connect(port: u16, name: &str, token: u64, host_key: u64) -> Result<TestClient, String> {
        let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
        sock.connect(("127.0.0.1", port)).unwrap();
        sock.set_read_timeout(Some(Duration::from_millis(50))).unwrap();
        let hello = Hello { proto: PROTO, name: name.into(), token, host_key }.encode();
        let start = Instant::now();
        let mut buf = [0u8; 2048];
        while start.elapsed() < Duration::from_secs(3) {
            sock.send(&hello).unwrap();
            if let Ok(n) = sock.recv(&mut buf) {
                let mut r = Reader::new(&buf[..n]);
                match r.u8().unwrap() {
                    S_WELCOME => {
                        let w = Welcome::decode(&mut r).unwrap().unwrap();
                        return Ok(TestClient {
                            sock,
                            session: w.session,
                            slot: w.slot as usize,
                            name: w.name,
                            snaps: Vec::new(),
                            latest: 0,
                            rel_in: RelIn::default(),
                            rel_out: RelOut::new(),
                            reasm: Reassembly::default(),
                            seq: 0,
                            chat: Vec::new(),
                            mutators: Vec::new(),
                            mazes: Vec::new(),
                            kicked: None,
                            bytes: 0,
                            packets: 0,
                        });
                    }
                    S_REJECT => return Err(r.str(400).unwrap()),
                    _ => {}
                }
            }
        }
        Err("timeout".into())
    }

    /// n Eingaben mit dt_ms schicken und alles empfangen, was da ist
    fn pump(&mut self, keys: u8, n: usize, dt_ms: u8) {
        let cmds: Vec<Cmd> = (0..n).map(|_| Cmd { keys, fires: 0, dt_ms }).collect();
        let first = self.seq + 1;
        self.seq += n as u32;
        let rel = self.rel_out.collect(Instant::now(), 600);
        let inp = Input {
            session: self.session,
            snap_ack: self.latest,
            rel_ack: self.rel_in.ack(),
            first_seq: first,
            cmds,
            rel,
        };
        let _ = self.sock.send(&inp.encode());
        self.sock.set_nonblocking(true).unwrap();
        let mut buf = [0u8; 2048];
        while let Ok(k) = self.sock.recv(&mut buf) {
            self.bytes += k;
            self.packets += 1;
            let data = buf[..k].to_vec();
            let mut r = Reader::new(&data);
            match r.u8().unwrap() {
                S_SNAP => self.body(r.rest()),
                S_FRAG => {
                    let (seq, idx, cnt) = (r.var32().unwrap(), r.u8().unwrap(), r.u8().unwrap());
                    if let Some(b) = self.reasm.add(seq, idx, cnt, r.rest()) {
                        self.body(&b);
                    }
                }
                S_KICK => {
                    if self.kicked.is_none() {
                        self.kicked = Some(r.str(400).unwrap());
                    }
                }
                _ => {}
            }
        }
        self.sock.set_nonblocking(false).unwrap();
    }

    fn body(&mut self, b: &[u8]) {
        let mut r = Reader::new(b);
        let head = SnapHead::read(&mut r).unwrap();
        self.rel_out.ack(head.rel_ack);
        for (id, d) in head.rel {
            for m in self.rel_in.receive(id, d).unwrap() {
                match Msg::decode(&m) {
                    Ok(Msg::Chat { text, .. }) => self.chat.push(text),
                    Ok(Msg::Maze(maze)) => {
                        self.mutators.push(maze.mutator);
                        self.mazes.push(maze);
                    }
                    _ => {}
                }
            }
        }
        if head.seq <= self.latest {
            return;
        }
        let base = if head.base == 0 { None } else { self.snaps.iter().find(|s| s.0 == head.base).map(|s| &s.1) };
        if head.base != 0 && base.is_none() {
            return;
        }
        let snap = read_snap(&mut r, base).expect("Snapshot kaputt");
        self.latest = head.seq;
        self.snaps.push((head.seq, snap));
        if self.snaps.len() > 128 {
            self.snaps.remove(0);
        }
    }

    fn snap(&self) -> &NetSnap {
        &self.snaps.last().expect("noch kein Snapshot").1
    }

    fn say(&mut self, text: &str) {
        self.rel_out.push(Msg::Say(text.into()).encode());
    }

    fn bye(&self) {
        let mut w = Writer::new();
        w.u8(C_BYE);
        w.u32(self.session);
        let _ = self.sock.send(&w.buf);
    }
}

/// alle Clients ein paar Ticks lang pumpen
fn run(clients: &mut [&mut TestClient], ms: u64) {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        for c in clients.iter_mut() {
            c.pump(0, 1, 16);
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}

/// Regel aus dem Menü: Der Host stellt sie ein, die Clients bekommen sie mit dem Labyrinth.
#[test]
fn regel_kommt_ueber_den_server() {
    let cfg = Config {
        port: 0,
        bots: 1,
        rule: RoundRule::Only(Mutator::Kisten),
        bans_file: None,
        verbose: false,
        ..Default::default()
    };
    let mut server = Server::bind(cfg).unwrap();
    let port = server.local_port();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let th = std::thread::spawn(move || server.run(&flag));

    let mut c = TestClient::connect(port, "Regel", 77, 0).unwrap();
    let start = Instant::now();
    while c.mutators.is_empty() && start.elapsed() < Duration::from_secs(5) {
        c.pump(0, 1, 16);
        std::thread::sleep(Duration::from_millis(16));
    }
    stop.store(true, Ordering::Relaxed);
    th.join().unwrap();
    assert!(!c.mutators.is_empty(), "kein Labyrinth empfangen");
    assert!(c.mutators.iter().all(|&m| m == Some(Mutator::Kisten)), "{:?}", c.mutators);
}

/// Volles Spiel mit Bots: Geht ein Spieler, läuft der Server weiter und Bots rücken nach.
#[test]
fn spieler_geht_waehrend_bots_spielen() {
    for _ in 0..10 {
        let cfg = Config { port: 0, bots: 7, bans_file: None, verbose: false, ..Default::default() };
        let mut server = Server::bind(cfg).unwrap();
        let port = server.local_port();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let th = std::thread::spawn(move || server.run(&flag));

        let mut c = TestClient::connect(port, "Geher", 78, 0).unwrap();
        run(&mut [&mut c], 500);
        c.bye();
        std::thread::sleep(Duration::from_millis(100));
        stop.store(true, Ordering::Relaxed);
        th.join().expect("Server ist abgestürzt");
    }
}

#[test]
fn multiplayer_over_udp() {
    let dir = std::env::temp_dir().join(format!("panzer-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let host_key = 0xABCDEF;
    let cfg = Config {
        port: 0,
        bots: 0,
        host_key,
        bans_file: Some(dir.join("bans.txt")),
        verbose: false,
        ..Default::default()
    };
    let mut server = Server::bind(cfg).unwrap();
    let port = server.local_port();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let th = std::thread::spawn(move || server.run(&flag));

    // Ohne Namen kommt niemand rein
    assert!(TestClient::connect(port, "   ", 1, 0).is_err());

    let mut host = TestClient::connect(port, "Martin", 11, host_key).unwrap();
    let mut b = TestClient::connect(port, "martin", 22, 0).unwrap();
    assert_eq!(b.name, "martin2", "doppelte Namen bekommen eine Nummer");
    run(&mut [&mut host, &mut b], 600);
    let round = host.snap().maze_id;
    assert!(round > 0, "Spiel sollte mit 2 Spielern laufen");
    assert!(!host.snap().queued && !b.snap().queued);

    // Dritter tritt bei: keine neue Runde, er wartet
    let mut c = TestClient::connect(port, "Gast", 33, 0).unwrap();
    run(&mut [&mut host, &mut b, &mut c], 500);
    assert_eq!(host.snap().maze_id, round, "Beitritt darf die Runde nicht neu starten");
    assert!(c.snap().queued, "Neuer Spieler muss in der Warteschlange sein");
    let c_maze = c.mazes.last().expect("neuer Spieler muss das Labyrinth erhalten");
    assert!(host.mazes.iter().any(|maze| maze == c_maze), "Spieler müssen dasselbe Labyrinth erhalten");
    assert_eq!(host.snap().tanks.len(), 2);
    assert!(c.chat.iter().any(|t| t.contains("Warteschlange")));

    // Chat kommt bei allen an
    b.say("hallo zusammen");
    run(&mut [&mut host, &mut b, &mut c], 300);
    assert!(host.chat.iter().any(|t| t == "hallo zusammen"));
    assert!(c.chat.iter().any(|t| t == "hallo zusammen"));

    // Nicht-Admins dürfen nicht kicken
    b.say("/kick Gast");
    run(&mut [&mut host, &mut b, &mut c], 300);
    assert!(c.kicked.is_none());

    // Speedhack: 10 Eingaben à 50 ms pro Paket = 30-fache Geschwindigkeit
    let start = b.snap().tanks.iter().find(|t| t.slot as usize == b.slot).map(|t| (t.x, t.y)).unwrap();
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_millis(1000) {
        b.pump(UP, 10, 50);
        host.pump(0, 1, 16);
        c.pump(0, 1, 16);
        std::thread::sleep(Duration::from_millis(16));
    }
    if b.snap().maze_id == round {
        if let Some(t) = b.snap().tanks.iter().find(|t| t.slot as usize == b.slot) {
            let moved = ((t.x as f64 - start.0 as f64).hypot(t.y as f64 - start.1 as f64)) / 4.0;
            // ehrlich wären höchstens ~130 px in 1 s (+ Budget)
            assert!(moved < 200.0, "Speedhack nicht gebremst: {moved} px");
        }
    }

    // Admin kickt den Wartenden
    host.say("/kick Gast nervt");
    run(&mut [&mut host, &mut b, &mut c], 400);
    assert!(c.kicked.as_deref().is_some_and(|k| k.contains("nervt")), "{:?}", c.kicked);

    // Bann gilt auch beim Wiederkommen
    let mut d = TestClient::connect(port, "Dieter", 44, 0).unwrap();
    run(&mut [&mut host, &mut b, &mut d], 200);
    host.say("/ban Dieter");
    run(&mut [&mut host, &mut b, &mut d], 400);
    assert!(d.kicked.is_some());
    assert!(TestClient::connect(port, "Dieter", 44, 0).err().unwrap_or_default().contains("gebannt"));

    // Verlassen: der Rest spielt weiter, keine neue Runde (außer sie war eh vorbei)
    let r_before = host.snap().maze_id;
    let mut e = TestClient::connect(port, "Emil", 55, 0).unwrap();
    run(&mut [&mut host, &mut b, &mut e], 300);
    e.bye();
    run(&mut [&mut host, &mut b], 300);
    assert!(host.snap().maze_id <= r_before + 1);
    assert!(host.chat.iter().any(|t| t.contains("Emil hat das Spiel verlassen")));

    // Bandbreite: Ø Bytes pro Snapshot-Paket
    println!("Host: {} Pakete, Ø {} Byte", host.packets, host.bytes / host.packets.max(1));
    assert!(host.bytes / host.packets.max(1) < 400);

    stop.store(true, Ordering::Relaxed);
    th.join().unwrap();
    let _ = std::fs::remove_dir_all(dir);
}
