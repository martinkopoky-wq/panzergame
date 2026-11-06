//! Balance-Messung: `cargo test --release --test balance -- --ignored --nocapture`
//!
//! Pro Waffe läuft eine Reihe Bot-Matches, in denen nur diese Waffe in Kisten liegt.
//! Die Bots fahren normal, feuern aber zusätzlich zufällig (Kollisionen sind dann
//! dieselben wie beim Spielen). Ausgabe: abgefeuert, Abschüsse, Abschüsse je Schuss.
//! Das ist ein grober Vergleich zwischen Waffen, keine Aussage über echte Spieler.

use panzer_core::bot::Bot;
use panzer_core::consts::*;
use panzer_core::items::{Item, Kind, ALL_ITEMS};
use panzer_core::rng::Rng;
use panzer_core::Game;
use std::collections::{HashMap, HashSet};

fn env_num(name: &str, default: f64) -> f64 {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// Ein Match mit nur `item` in den Kisten. Liefert (abgefeuert je Art, Abschüsse je Art).
fn run_match(item: Item, seed: u64, secs: f64) -> (HashMap<Kind, u32>, HashMap<Kind, u32>) {
    let slots: Vec<usize> = (0..4).collect();
    let mut g = Game::with_rng(&slots, Rng::new(seed));
    g.bots = slots.clone();
    g.box_timer = 1e9; // keine weiteren Kisten
    g.mutator = None;
    g.arena.mutator = None;
    let mut rng = Rng::new(seed ^ 0x5151);
    let cells = g.arena.cells();
    for _ in 0..6 {
        let c = cells[rng.below(cells.len())];
        let (x, y) = g.arena.center(c);
        g.boxes.push((x, y, item));
    }
    let mut bots: Vec<Bot> = slots.iter().map(|&s| Bot::new(s, Some(0.85))).collect();
    let mut counter = [0u16; 4];
    let mut seen_shots: HashSet<u32> = HashSet::new();
    let mut fired: HashMap<Kind, u32> = HashMap::new();
    let mut kills: HashMap<Kind, u32> = HashMap::new();
    for _ in 0..(secs / TICK) as usize {
        for b in bots.iter_mut() {
            let (k, _) = b.think(&g);
            let s = b.slot;
            let mut keys = k & !SHOOT;
            // zufällig abdrücken, wenn eine Waffe im Spiel ist (Halten wird nicht simuliert)
            if g.tanks[s].alive && g.tanks[s].weapon.is_some() && rng.random() < 0.06 {
                counter[s] = counter[s].wrapping_add(1);
            } else if counter[s] == 0 {
                counter[s] = g.tanks[s].fire_seen.unwrap_or(0);
            }
            keys &= !0; // Bewegung unverändert
            g.apply_input(s, keys, counter[s], TICK);
        }
        g.step(TICK);
        for s in g.shots.iter() {
            if seen_shots.insert(s.id) {
                *fired.entry(s.kind).or_default() += 1;
            }
        }
        for f in g.feed.iter() {
            if f.age <= TICK + 1e-9 && f.by.is_some() && f.by != Some(f.victim) {
                *kills.entry(f.how).or_default() += 1;
            }
        }
        if g.alive_slots().len() <= 1 && g.end_timer.is_none() {
            g.new_round();
        }
    }
    (fired, kills)
}

#[test]
#[ignore]
fn balance_pro_waffe() {
    let matches = env_num("BALANCE_MATCHES", 3.0) as u64;
    let secs = env_num("BALANCE_SECS", 60.0);
    let only: Option<String> = std::env::var("BALANCE_ONLY").ok();
    eprintln!("{:<22}{:>8}{:>8}{:>10}", "waffe", "feuer", "kills", "quote");
    for &item in ALL_ITEMS {
        if !item.is_box_weapon() || item.is_hold() {
            continue;
        }
        if let Some(o) = &only {
            if !format!("{item:?}").eq_ignore_ascii_case(o) {
                continue;
            }
        }
        let (mut fired, mut kills) = (0u32, 0u32);
        for m in 0..matches {
            let (f, k) = run_match(item, 500 + m, secs);
            fired += f.values().sum::<u32>();
            kills += k.values().sum::<u32>();
        }
        let q = if fired > 0 { kills as f64 / fired as f64 } else { 0.0 };
        eprintln!("{:<22}{:>8}{:>8}{:>10.2}", item.name(), fired, kills, q);
    }
}
