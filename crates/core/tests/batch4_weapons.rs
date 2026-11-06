//! Batch 4 der neuen Waffen: Stillstandsurteil, Geschossbrüter, Konterklinge, Rostwolke,
//! Prallkissen, Kanonenschlag.

use panzer_core::consts::*;
use panzer_core::items::{Eff, Item, Kind};
use panzer_core::rng::Rng;
use panzer_core::Game;
use std::f64::consts::PI;

fn game(seed: u64) -> Game {
    let mut g = Game::with_rng(&[0, 1], Rng::new(seed));
    for t in g.tanks.iter_mut() {
        t.eff.remove(Eff::Spawn);
    }
    g.tanks[0].eff.set(Eff::Inv, 1e6);
    g.tanks[1].eff.remove(Eff::Inv);
    g.box_timer = 1e9;
    g.mutator = None;
    g.arena.mutator = None;
    g
}

/// Panzer 0 bei (x,y) mit Blick `a0`, Panzer 1 `gap` Einheiten davor
fn duel(g: &mut Game, gap: f64, a0: f64) -> (f64, f64) {
    let (x, y) = g
        .arena
        .cells()
        .into_iter()
        .map(|c| g.arena.center(c))
        .find(|&(x, y)| g.arena.clear(x - 60.0, y, x + 60.0, y))
        .expect("keine freie Zelle");
    g.tanks[0].x = x - gap / 2.0;
    g.tanks[0].y = y;
    g.tanks[0].a = a0;
    g.tanks[1].x = x + gap / 2.0;
    g.tanks[1].y = y;
    g.tanks[1].a = PI;
    (x, y)
}

fn run(g: &mut Game, seconds: f64) {
    for _ in 0..(seconds / TICK).round() as usize {
        g.step(TICK);
    }
}

fn press(g: &mut Game, slot: usize) {
    let t = &mut g.tanks[slot];
    t.cooldown = 0.0;
    let seen = t.fire_seen.unwrap_or(0);
    g.apply_input(slot, 0, seen, TICK);
    g.apply_input(slot, 0, seen.wrapping_add(1), TICK);
}

fn give(g: &mut Game, slot: usize, item: Item) {
    g.tanks[slot].weapon = Some(item);
    g.tanks[slot].ammo = item.ammo();
}

fn fire_bullet(g: &mut Game, slot: usize) {
    g.tanks[slot].weapon = None;
    press(g, slot);
}

#[test]
fn stillstandsurteil_trifft_nur_den_Stehenden() {
    let mut g = game(1);
    duel(&mut g, 80.0, 0.0);
    give(&mut g, 0, Item::Verdict);
    press(&mut g, 0);
    run(&mut g, VERDICT_STILL + 0.2);
    assert!(!g.tanks[1].alive, "Urteil trifft den stehenden Gegner nicht");

    let mut g = game(2);
    duel(&mut g, 80.0, 0.0);
    give(&mut g, 0, Item::Verdict);
    press(&mut g, 0);
    for _ in 0..3 {
        run(&mut g, 0.5);
        g.tanks[1].y += 30.0; // bewegt sich ständig
    }
    assert!(g.tanks[1].alive, "Urteil trifft den Bewegten");
}

#[test]
fn geschossbrueter_wirft_kugel_zurueck() {
    let mut g = game(3);
    duel(&mut g, 80.0, PI); // Schütze blickt nach Westen, Fangzone liegt im Osten
    give(&mut g, 0, Item::Catcher);
    press(&mut g, 0);
    let (cx, cy) = {
        let s = g.shots.iter().find(|s| s.kind == Kind::Catcher).expect("Fänger fehlt");
        (s.x, s.y)
    };
    g.tanks[1].x = cx + 40.0;
    g.tanks[1].y = cy;
    g.tanks[1].a = PI; // schießt nach Westen auf den Fänger
    fire_bullet(&mut g, 1);
    run(&mut g, 0.2);
    // die Kugel wird im Fangradius umgekehrt und fliegt dem Schützen (Panzer 1) entgegen
    assert!(!g.tanks[1].alive, "zurückgeworfene Kugel hat den Schützen nicht getroffen");
    assert_eq!(g.tanks[0].slot, 0);
}

#[test]
fn konterklinge_nur_mit_abgewehrter_kugel() {
    // ohne Kugel: nichts passiert
    let mut g = game(4);
    duel(&mut g, 35.0, 0.0);
    give(&mut g, 0, Item::Parry);
    press(&mut g, 0);
    assert!(g.tanks[1].alive, "Konterklinge stößt ohne Kugel zu");

    // mit Kugel vor dem Panzer und Gegner in Reichweite: Kugel weg, Gegner getroffen
    let mut g = game(5);
    duel(&mut g, 35.0, 0.0);
    fire_bullet(&mut g, 1);
    give(&mut g, 0, Item::Parry);
    press(&mut g, 0);
    assert_eq!(g.shots.iter().filter(|s| s.kind == Kind::B && !s.dead).count(), 0, "Kugel nicht abgewehrt");
    assert!(!g.tanks[1].alive, "kein Stoß nach dem Abwehren");
}

#[test]
fn rostwolke_blockiert_das_feuer() {
    let mut g = game(6);
    duel(&mut g, 80.0, 0.0);
    give(&mut g, 0, Item::Rust);
    press(&mut g, 0);
    let (rx, ry) = {
        let s = g.shots.iter().find(|s| s.kind == Kind::Rust).expect("Wolke fehlt");
        (s.x, s.y)
    };
    g.tanks[1].x = rx;
    g.tanks[1].y = ry;
    run(&mut g, TICK * 2.0);
    assert!(g.tanks[1].cooldown >= RUST_PAUSE - 1e-6, "Wolke sperrt das Feuer nicht");
    // Schuss ohne Feuerpause zurücksetzen: die Wolke allein muss sperren
    let seen = g.tanks[1].fire_seen.unwrap_or(0);
    g.apply_input(1, 0, seen, TICK);
    g.apply_input(1, 0, seen.wrapping_add(1), TICK);
    assert_eq!(g.shots.iter().filter(|s| s.kind == Kind::B && s.owner == 1 && !s.dead).count(), 0);
}

#[test]
fn prallkissen_stoesst_den_einfahrenden_weg() {
    let mut g = game(7);
    duel(&mut g, 80.0, 0.0);
    give(&mut g, 0, Item::Cushion);
    press(&mut g, 0);
    let (kx, ky) = {
        let s = g.shots.iter().find(|s| s.kind == Kind::Cushion).expect("Kissen fehlt");
        (s.x, s.y)
    };
    run(&mut g, CUSHION_ARM + 0.1); // scharf
    g.tanks[1].x = kx + 5.0;
    g.tanks[1].y = ky;
    run(&mut g, TICK * 2.0);
    let d = ((g.tanks[1].x - kx).powi(2) + (g.tanks[1].y - ky).powi(2)).sqrt();
    assert!(d > CUSHION_PUSH * 0.6, "Kissen stößt nicht weg, Abstand {d}");
}

#[test]
fn kanonenschlag_zuendet_splitter() {
    let mut g = game(8);
    duel(&mut g, 80.0, 0.0);
    give(&mut g, 0, Item::Cracker);
    press(&mut g, 0);
    // bis zur Zündung laufen lassen, dann direkt nach dem Knall zählen
    for _ in 0..((CRACKER_FUSE + 1.0) / TICK) as usize {
        g.step(TICK);
        if !g.shots.iter().any(|s| s.kind == Kind::Cracker && !s.dead) {
            break;
        }
    }
    let n = g.shots.iter().filter(|s| s.kind == Kind::Frg && s.owner == 0 && !s.dead).count();
    assert_eq!(n, CRACKER_N, "Kanonenschlag wirft keine {CRACKER_N} Splitter");
}
