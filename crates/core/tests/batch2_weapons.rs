//! Batch 2 der neuen Waffen: Zweitaktgeschoss, Eckenschuss, Kettenzange, Schwenkhammer,
//! Zündpause, Köderladung, Schattensalve, Rohrverdreher.

use panzer_core::consts::*;
use panzer_core::items::{Eff, Item, Kind};
use panzer_core::rng::Rng;
use panzer_core::Game;
use std::f64::consts::{FRAC_PI_2, PI};

fn game(seed: u64) -> Game {
    let mut g = Game::with_rng(&[0, 1], Rng::new(seed));
    for t in g.tanks.iter_mut() {
        t.eff.remove(Eff::Spawn);
    }
    g.tanks[0].eff.set(Eff::Inv, 1e6); // Schütze stirbt nicht durch eigene Waffen
    g.tanks[1].eff.remove(Eff::Inv);
    g.box_timer = 1e9;
    g.mutator = None;
    g.arena.mutator = None;
    g
}

/// Panzer 0 links, Panzer 1 rechts auf freier Linie (Abstand `gap`)
fn duel(g: &mut Game, gap: f64) {
    let (x, y) = g
        .arena
        .cells()
        .into_iter()
        .map(|c| g.arena.center(c))
        .find(|&(x, y)| g.arena.clear(x - 60.0, y, x + 60.0, y))
        .expect("keine freie Zelle");
    g.tanks[0].x = x - gap / 2.0;
    g.tanks[0].y = y;
    g.tanks[0].a = 0.0;
    g.tanks[1].x = x + gap / 2.0;
    g.tanks[1].y = y;
    g.tanks[1].a = PI;
}

fn run(g: &mut Game, seconds: f64) {
    for _ in 0..(seconds / TICK).round() as usize {
        g.step(TICK);
    }
}

/// eine Waffe-Taste drücken (ein Schuss)
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

fn count(g: &Game, kind: Kind) -> usize {
    g.shots.iter().filter(|s| s.kind == kind && !s.dead).count()
}

#[test]
fn zweitakt_erst_markierung_dann_treffer() {
    let mut g = game(1);
    duel(&mut g, 80.0);
    give(&mut g, 0, Item::Double);
    press(&mut g, 0);
    run(&mut g, 0.5);
    assert!(g.tanks[1].alive, "erster Treffer darf nicht töten");
    assert!(g.tanks[1].mark.is_some(), "erster Treffer hat nicht markiert");
    give(&mut g, 0, Item::Double);
    press(&mut g, 0);
    run(&mut g, 0.5);
    assert!(!g.tanks[1].alive, "zweiter Treffer hat nicht getroffen");
}

#[test]
fn eckenschuss_biegt_einmal_rechts_ab() {
    let mut g = game(2);
    duel(&mut g, 80.0);
    give(&mut g, 0, Item::Corner);
    press(&mut g, 0);
    run(&mut g, 0.1);
    assert_eq!(count(&g, Kind::Corner), 1);
    press(&mut g, 0); // zweiter Druck: rechts abbiegen
    let s = g.shots.iter().find(|s| s.kind == Kind::Corner && !s.dead).expect("Eckenschuss fehlt");
    assert!((s.heading() - FRAC_PI_2).abs() < 1e-6, "Kurs ist {}", s.heading());
    assert_eq!(count(&g, Kind::Corner), 1, "zweiter Druck darf keinen neuen Schuss abgeben");
}

#[test]
fn kettenzange_friert_beide_und_trifft_danach() {
    let mut g = game(3);
    duel(&mut g, 30.0); // Gegner in Greifweite
    give(&mut g, 0, Item::Chain);
    press(&mut g, 0);
    run(&mut g, 0.3);
    assert!(g.tanks[1].alive, "Zange trifft vor der Haltezeit");
    assert!(g.tanks[1].has(Eff::Frozen) && g.tanks[0].has(Eff::Frozen), "beide müssen stehen");
    run(&mut g, 1.0);
    assert!(!g.tanks[1].alive, "Zange hat nach der Haltezeit nicht getroffen");
}

#[test]
fn schwenkhammer_trifft_vorn_nicht_hinten() {
    let mut g = game(4);
    duel(&mut g, 30.0);
    give(&mut g, 0, Item::Hammer);
    press(&mut g, 0);
    run(&mut g, 0.8);
    assert!(!g.tanks[1].alive, "Hammer trifft den Gegner vor sich nicht");

    let mut g = game(5);
    duel(&mut g, 30.0);
    g.tanks[1].x = g.tanks[0].x - 30.0; // hinter dem Schützen
    g.tanks[1].a = 0.0;
    give(&mut g, 0, Item::Hammer);
    press(&mut g, 0);
    run(&mut g, 0.8);
    assert!(g.tanks[1].alive, "Hammer trifft hinter dem Panzer");
}

#[test]
fn zuendpause_friert_kugeln_ein() {
    let mut g = game(6);
    duel(&mut g, 80.0);
    // Gegner schießt eine Kugel auf den Schützen
    let seen = g.tanks[1].fire_seen.unwrap_or(0);
    g.tanks[1].cooldown = 0.0;
    g.apply_input(1, 0, seen, TICK);
    g.apply_input(1, 0, seen.wrapping_add(1), TICK);
    run(&mut g, 0.05);
    let bullet = g.shots.iter().find(|s| s.kind == Kind::B && !s.dead).expect("Kugel fehlt");
    let (bx, by) = (bullet.x, bullet.y);
    give(&mut g, 0, Item::Pause);
    press(&mut g, 0);
    run(&mut g, 0.5);
    let bullet = g.shots.iter().find(|s| s.kind == Kind::B && !s.dead).expect("Kugel ist verschwunden");
    assert!((bullet.x - bx).abs() < 1e-6 && (bullet.y - by).abs() < 1e-6, "Kugel bewegt sich in der Pause");
}

#[test]
fn koeder_wartet_ohne_ausloeser_und_verschwindet_still() {
    let mut g = game(7);
    duel(&mut g, 80.0);
    give(&mut g, 0, Item::Decoy);
    press(&mut g, 0);
    // Gegner steht genau auf dem Köder, es fliegt aber nichts dagegen
    let (dx, dy) = {
        let s = g.shots.iter().find(|s| s.kind == Kind::Decoy).expect("Köder fehlt");
        (s.x, s.y)
    };
    g.tanks[1].x = dx;
    g.tanks[1].y = dy;
    run(&mut g, DECOY_LIFE + 0.5);
    assert!(g.tanks[1].alive, "Köder explodiert ohne Auslöser");
    assert_eq!(count(&g, Kind::Decoy), 0);
}

#[test]
fn schattensalve_feuert_drei_kugeln() {
    let mut g = game(8);
    duel(&mut g, 80.0);
    give(&mut g, 0, Item::Shadow);
    press(&mut g, 0);
    // direkt nach der Vorwarnzeit: alle drei entstehen, bevor eine gegen eine Wand fliegt
    run(&mut g, SHADOW_FUSE + 2.0 * TICK);
    assert_eq!(count(&g, Kind::B), 3, "Schattensalve feuert nicht drei Kugeln");
}

#[test]
fn rohrverdreher_dreht_panzer_im_umkreis() {
    let mut g = game(9);
    duel(&mut g, 60.0);
    give(&mut g, 0, Item::Twist);
    press(&mut g, 0);
    run(&mut g, TWIST_FUSE + 0.2);
    assert!((g.tanks[1].a - (PI + FRAC_PI_2)).abs() < 1e-9, "Gegner wurde nicht gedreht: {}", g.tanks[1].a);
}
