//! Batch 1 der neuen Waffen: Geschossschere, Hohlringgranate, Distanzzünder,
//! Kugelfresser, Entwaffnungsfächer.

use panzer_core::consts::*;
use panzer_core::items::{Eff, Item, Kind};
use panzer_core::rng::Rng;
use panzer_core::Game;

fn game(seed: u64) -> Game {
    let mut g = Game::with_rng(&[0, 1], Rng::new(seed));
    for t in g.tanks.iter_mut() {
        t.eff.remove(Eff::Spawn);
        t.eff.set(Eff::Inv, 1e6); // niemand stirbt aus Versehen
    }
    g.box_timer = 1e9;
    g
}

/// Zwei Panzer in derselben Zelle, 80 Einheiten auseinander, Blick aufeinander zu
fn duel(g: &mut Game) -> (f64, f64) {
    // Zelle, in der die Linie zwischen den Panzern frei von Wänden ist
    let (x, y) = g
        .arena
        .cells()
        .into_iter()
        .map(|c| g.arena.center(c))
        .find(|&(x, y)| g.arena.clear(x - 60.0, y, x + 60.0, y))
        .expect("keine freie Zelle");
    g.tanks[0].x = x - 40.0;
    g.tanks[0].y = y;
    g.tanks[0].a = 0.0;
    g.tanks[1].x = x + 40.0;
    g.tanks[1].y = y;
    g.tanks[1].a = std::f64::consts::PI;
    (x, y)
}

fn run(g: &mut Game, seconds: f64) {
    for _ in 0..(seconds / TICK).round() as usize {
        g.step(TICK);
    }
}

fn give_and_fire(g: &mut Game, slot: usize, item: Item) {
    let t = &mut g.tanks[slot];
    t.weapon = Some(item);
    t.ammo = item.ammo();
    t.cooldown = 0.0;
    let seen = t.fire_seen.unwrap_or(0);
    g.apply_input(slot, 0, seen, TICK);
    g.apply_input(slot, 0, seen.wrapping_add(1), TICK);
}

fn alive_count(g: &Game, kind: Kind) -> usize {
    g.shots.iter().filter(|s| s.kind == kind && !s.dead).count()
}

#[test]
fn geschossschere_schneidet_gegnerkugeln_im_kegel() {
    let mut g = game(1);
    duel(&mut g);
    // Gegner schießt eine normale Kugel auf den Schützen zu
    g.tanks[1].weapon = None;
    g.tanks[1].cooldown = 0.0;
    let seen = g.tanks[1].fire_seen.unwrap_or(0);
    g.apply_input(1, 0, seen, TICK);
    g.apply_input(1, 0, seen.wrapping_add(1), TICK);
    assert_eq!(alive_count(&g, Kind::B), 1, "Gegnerkugel fehlt");

    give_and_fire(&mut g, 0, Item::Shear);
    assert_eq!(alive_count(&g, Kind::B), 0, "Schere hat die Kugel nicht geschnitten");
    assert_eq!(g.tanks[0].weapon, Some(Item::Shear));
}

#[test]
fn geschossschere_laesst_eigene_kugeln_in_ruhe() {
    let mut g = game(2);
    duel(&mut g);
    g.tanks[0].weapon = None;
    g.tanks[0].cooldown = 0.0;
    let seen = g.tanks[0].fire_seen.unwrap_or(0);
    g.apply_input(0, 0, seen, TICK);
    g.apply_input(0, 0, seen.wrapping_add(1), TICK);
    assert_eq!(alive_count(&g, Kind::B), 1);
    give_and_fire(&mut g, 0, Item::Shear);
    assert_eq!(alive_count(&g, Kind::B), 1, "eigene Kugel wurde geschnitten");
}

#[test]
fn kugelfresser_schluckt_hoechstens_drei_kugeln() {
    let mut g = game(3);
    duel(&mut g);
    // Gegner feuert vier Kugeln auf den Schützen, dann folgt der Kugelfresser
    for _ in 0..4 {
        let seen = g.tanks[1].fire_seen.unwrap_or(0);
        g.tanks[1].cooldown = 0.0;
        g.apply_input(1, 0, seen, TICK);
        g.apply_input(1, 0, seen.wrapping_add(1), TICK);
    }
    assert_eq!(alive_count(&g, Kind::B), 4, "vier Kugeln fehlen");
    give_and_fire(&mut g, 0, Item::Eater);
    run(&mut g, 0.12);
    let eater = g.shots.iter().find(|s| s.kind == Kind::Eater && !s.dead).expect("Kugelfresser fehlt");
    assert_eq!(eater.eaten, EATER_MAX, "Kugelfresser sollte genau 3 Kugeln fressen");
    assert!(eater.r > EATER_R, "Radius wächst nicht");
    assert_eq!(alive_count(&g, Kind::B), 1, "genau eine Kugel muss übrig bleiben");
}

#[test]
fn distanzzuender_ist_vor_der_scharfschaltung_harmlos() {
    let mut g = game(4);
    duel(&mut g);
    // Gegner steht direkt vor der Mündung: Schuss ist noch nicht scharf
    g.tanks[1].x = g.tanks[0].x + 40.0;
    g.tanks[1].y = g.tanks[0].y;
    g.tanks[1].eff.set(Eff::Inv, 0.0);
    g.tanks[1].eff.remove(Eff::Inv);
    give_and_fire(&mut g, 0, Item::Dist);
    run(&mut g, 0.15); // 0.15 s * 420 = 63 < 140
    assert!(g.tanks[1].alive, "unscharfer Distanzzünder hat getroffen");
}

#[test]
fn hohlringgranate_trifft_den_ring_nicht_das_zentrum() {
    let mut g = game(5);
    duel(&mut g);
    give_and_fire(&mut g, 0, Item::Ring);
    let ring = g.shots.iter().find(|s| s.kind == Kind::Ring && !s.dead).expect("Ring fehlt");
    let (cx, cy) = (ring.x, ring.y);
    // Panzer 1 ins Zentrum, Panzer 0 in den Ring
    g.tanks[1].eff.remove(Eff::Inv);
    g.tanks[1].x = cx;
    g.tanks[1].y = cy;
    g.tanks[0].eff.remove(Eff::Inv);
    g.tanks[0].x = cx + (RING_IN + RING_OUT) / 2.0;
    g.tanks[0].y = cy;
    run(&mut g, RING_FUSE + 0.2);
    assert!(g.tanks[1].alive, "Zentrum des Rings wurde getroffen");
    assert!(!g.tanks[0].alive, "Ring-Treffer blieb aus");
}

#[test]
fn entwaffnungsfaecher_legt_die_waffe_als_kiste_ab() {
    let mut g = game(6);
    duel(&mut g);
    g.tanks[1].weapon = Some(Item::Rocket);
    g.tanks[1].ammo = 1;
    give_and_fire(&mut g, 0, Item::Disarm);
    assert_eq!(g.tanks[1].weapon, None, "Gegner behält seine Waffe");
    assert!(g.boxes.iter().any(|b| b.2 == Item::Rocket), "Waffe liegt nicht als Kiste am Boden");
    // der Entwaffnete kann sie in den ersten Sekunden nicht zurücknehmen
    run(&mut g, DISARM_PICK_WAIT * 0.5);
    assert_eq!(g.tanks[1].weapon, None, "Entwaffneter hat sofort zugegriffen");
}
