//! Batch 3 der neuen Waffen: Geschossweiche, Nachbrennbild, Richtungsdorn, Trittplatte,
//! Zangenmarkierung, Fadenkreuzfluch, Kreuzpendel.

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
    g.tanks[0].eff.set(Eff::Inv, 1e6);
    g.tanks[1].eff.remove(Eff::Inv);
    g.box_timer = 1e9;
    g.mutator = None;
    g.arena.mutator = None;
    g
}

fn duel(g: &mut Game, gap: f64) -> (f64, f64) {
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

fn find(g: &Game, kind: Kind) -> (f64, f64) {
    let s = g.shots.iter().find(|s| s.kind == kind && !s.dead).expect("Objekt fehlt");
    (s.x, s.y)
}

#[test]
fn geschossweiche_dreht_gegnerkugel_um_90_grad() {
    let mut g = game(1);
    duel(&mut g, 80.0);
    give(&mut g, 0, Item::Switch);
    press(&mut g, 0);
    let (zx, zy) = find(&g, Kind::Switch);
    // Gegner steht auf der Weiche und feuert nach Osten
    g.tanks[1].x = zx;
    g.tanks[1].y = zy;
    g.tanks[1].a = 0.0;
    g.tanks[1].cooldown = 0.0;
    let seen = g.tanks[1].fire_seen.unwrap_or(0);
    g.apply_input(1, 0, seen, TICK);
    g.apply_input(1, 0, seen.wrapping_add(1), TICK);
    run(&mut g, TICK);
    let b = g.shots.iter().find(|s| s.kind == Kind::B && s.owner == 1 && !s.dead).expect("Kugel fehlt");
    assert!((b.heading() + FRAC_PI_2).abs() < 1e-6, "Kurs ist {}", b.heading());
}

#[test]
fn nachbrennbild_trifft_wer_auf_der_linie_steht() {
    let mut g = game(2);
    let (x, y) = duel(&mut g, 80.0);
    let _ = (x, y);
    give(&mut g, 0, Item::Afterburn);
    press(&mut g, 0);
    let (lx, ly) = find(&g, Kind::Afterburn);
    g.tanks[1].x = lx + 100.0;
    g.tanks[1].y = ly;
    run(&mut g, AFTERBURN_FUSE + 0.2);
    assert!(!g.tanks[1].alive, "Linie trifft den Gegner nicht");
}

#[test]
fn richtungsdorn_nur_von_der_pfeilseite() {
    let mut g = game(3);
    duel(&mut g, 80.0);
    give(&mut g, 0, Item::Thorn);
    press(&mut g, 0);
    let (tx, ty) = find(&g, Kind::Thorn);
    // hinter der Leiste: harmlos
    g.tanks[1].x = tx - 12.0;
    g.tanks[1].y = ty;
    run(&mut g, THORN_FUSE + 0.2);
    assert!(g.tanks[1].alive, "Richtungsdorn trifft von hinten");
    // von der Pfeilseite: trifft
    g.tanks[1].x = tx + 12.0;
    g.tanks[1].y = ty;
    run(&mut g, 0.1);
    assert!(!g.tanks[1].alive, "Richtungsdorn trifft nicht von vorn");
}

#[test]
fn trittplatte_zaehlt_betritte_und_warnt() {
    let mut g = game(4);
    duel(&mut g, 80.0);
    give(&mut g, 0, Item::Plate);
    press(&mut g, 0);
    let (px, py) = find(&g, Kind::Plate);
    for _ in 0..PLATE_COUNT {
        g.tanks[1].x = px;
        g.tanks[1].y = py;
        run(&mut g, TICK);
        g.tanks[1].x = px + 200.0;
        g.tanks[1].y = py;
        run(&mut g, TICK);
    }
    let plate = g.shots.iter().find(|s| s.kind == Kind::Plate && !s.dead).expect("Platte fehlt");
    assert!(plate.cool > 0.0, "nach dem dritten Betritt keine Warnung");
    assert!(g.tanks[1].alive, "Platte hat zu früh gewirkt");
}

#[test]
fn zangenmarkierung_klemmt_das_ziel_ein() {
    let mut g = game(5);
    duel(&mut g, 80.0);
    give(&mut g, 0, Item::Pincer);
    press(&mut g, 0);
    let pincers: Vec<(f64, f64)> = g.shots.iter().filter(|s| s.kind == Kind::Pincer).map(|s| (s.x, s.y)).collect();
    assert_eq!(pincers.len(), 2, "zwei Zangen erwartet");
    // Mitte zwischen den beiden Zangen
    let cx = (pincers[0].0 + pincers[1].0) / 2.0;
    let cy = (pincers[0].1 + pincers[1].1) / 2.0;
    g.tanks[1].x = cx;
    g.tanks[1].y = cy;
    run(&mut g, PINCER_FUSE + PINCER_LAT / PINCER_SPEED + 0.3);
    assert!(!g.tanks[1].alive, "Zangen treffen die Mitte nicht");
}

#[test]
fn fadenkreuzfluch_folgt_dann_friert_und_trifft() {
    let mut g = game(6);
    duel(&mut g, 80.0);
    give(&mut g, 0, Item::Cross);
    press(&mut g, 0);
    run(&mut g, CROSS_FOLLOW + CROSS_FREEZE + 0.2);
    assert!(!g.tanks[1].alive, "Fadenkreuz trifft das Ziel nicht");
}

#[test]
fn kreuzpendel_trifft_den_arm() {
    let mut g = game(7);
    duel(&mut g, 80.0);
    give(&mut g, 0, Item::Pendel);
    press(&mut g, 0);
    // der Gegner steht 30 vom Sockel entfernt und der Arm streicht über ihn hinweg
    let (px, py, ang) = {
        let s = g.shots.iter().find(|s| s.kind == Kind::Pendel && !s.dead).expect("Pendel fehlt");
        (s.x, s.y, s.ang)
    };
    let _ = ang;
    let mut hit = false;
    for _ in 0..((PENDEL_WARN + 1.0) / TICK) as usize {
        let a = g.shots.iter().find(|s| s.kind == Kind::Pendel && !s.dead).map(|s| s.ang).unwrap_or(0.0);
        g.tanks[1].x = px + 30.0 * a.cos();
        g.tanks[1].y = py + 30.0 * a.sin();
        g.step(TICK);
        if !g.tanks[1].alive {
            hit = true;
            break;
        }
    }
    assert!(hit, "Arm trifft den Gegner nie");
}
