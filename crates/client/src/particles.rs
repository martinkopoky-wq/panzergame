//! Rein optische Partikel (Funken, Feuer, Rauch, Trümmer ...). Jeder Client hat seine
//! eigenen, nichts davon geht übers Netz.

use std::f32::consts::{PI, TAU};

use panzer_core::geom::circle_hits_walls;
use panzer_core::rng::Rng;
use panzer_core::Arena;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum P {
    Spark,
    Fire,
    Smoke,
    Dust,
    Debris,
    Ember,
    Shard,
    Confetti,
    Flash,
    Star,
    Ring,
    /// geformtes Mündungsfeuer: angle = Schussrichtung, size = Länge
    Muzzle,
}

pub const FIRE_GRAD: [[u8; 3]; 7] =
    [[255, 248, 222], [255, 222, 130], [255, 172, 64], [242, 112, 32], [205, 64, 22], [130, 34, 12], [60, 16, 6]];

#[derive(Clone, Debug)]
pub struct Part {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub age: f32,
    pub life: f32,
    pub kind: P,
    pub size: f32,
    pub color: [u8; 3],
    pub drag: f32,
    pub grav: f32,
    pub angle: f32,
    pub spin: f32,
}

pub struct Particles {
    pub items: Vec<Part>,
    pub rng: Rng,
}

const MAX: usize = 2400;

/// Optionen für add/burst (Standardwerte wie früher die Schlüsselwort-Argumente)
#[derive(Clone, Copy)]
pub struct Opt {
    pub color: [u8; 3],
    pub drag: f32,
    pub grav: f32,
    pub spin: f32,
    pub angle: f32,
    pub spread: f32,
}

impl Default for Opt {
    fn default() -> Self {
        Opt { color: [255, 255, 255], drag: 0.0, grav: 0.0, spin: 0.0, angle: 0.0, spread: PI }
    }
}

pub fn o(color: [u8; 3]) -> Opt {
    Opt { color, ..Default::default() }
}

impl Opt {
    pub fn drag(mut self, d: f32) -> Self {
        self.drag = d;
        self
    }
    pub fn grav(mut self, g: f32) -> Self {
        self.grav = g;
        self
    }
    pub fn spin(mut self, s: f32) -> Self {
        self.spin = s;
        self
    }
    pub fn fan(mut self, angle: f32, spread: f32) -> Self {
        self.angle = angle;
        self.spread = spread;
        self
    }
}

impl Particles {
    pub fn new() -> Particles {
        Particles { items: Vec::with_capacity(MAX), rng: Rng::from_entropy() }
    }

    pub fn u(&mut self, a: f32, b: f32) -> f32 {
        self.rng.uniform(a as f64, b as f64) as f32
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add(&mut self, kind: P, x: f32, y: f32, vx: f32, vy: f32, life: f32, size: f32, opt: Opt) {
        if self.items.len() < MAX {
            let angle = self.u(0.0, TAU);
            self.items.push(Part {
                x,
                y,
                vx,
                vy,
                age: 0.0,
                life,
                kind,
                size,
                color: opt.color,
                drag: opt.drag,
                grav: opt.grav,
                angle,
                spin: opt.spin,
            });
        }
    }

    /// Partikel mit fester Ausrichtung (Mündungsfeuer)
    #[allow(clippy::too_many_arguments)]
    pub fn add_dir(&mut self, kind: P, x: f32, y: f32, angle: f32, life: f32, size: f32, opt: Opt) {
        self.add(kind, x, y, 0.0, 0.0, life, size, opt);
        if let Some(p) = self.items.last_mut().filter(|p| p.kind == kind && p.age == 0.0) {
            p.angle = angle;
        }
    }

    /// n Partikel rundherum bzw. im Fächer angle ± spread
    #[allow(clippy::too_many_arguments)]
    pub fn burst(&mut self, kind: P, x: f32, y: f32, n: f32, speed: f32, life: f32, size: f32, opt: Opt) {
        for _ in 0..n as usize {
            let a = opt.angle + self.u(-opt.spread, opt.spread);
            let v = speed * self.u(0.3, 1.0);
            let lk = self.u(0.6, 1.25);
            let sk = self.u(0.7, 1.3);
            let spin = self.u(-opt.spin, opt.spin);
            self.add(kind, x, y, a.cos() * v, a.sin() * v, life * lk, size * sk, Opt { spin, ..opt });
        }
    }

    /// Bewegt alles. Gibt die Trümmer zurück, die liegen geblieben sind.
    pub fn update(&mut self, dt: f32, arena: Option<&Arena>) -> Vec<Part> {
        let mut landed = Vec::new();
        self.items.retain_mut(|p| {
            p.age += dt;
            if p.age >= p.life {
                if p.kind == P::Debris {
                    landed.push(p.clone());
                }
                return false;
            }
            if p.drag != 0.0 {
                let f = (1.0 - p.drag * dt).max(0.0);
                p.vx *= f;
                p.vy *= f;
            }
            p.vy += p.grav * dt;
            let (mut nx, mut ny) = (p.x + p.vx * dt, p.y + p.vy * dt);
            if let Some(a) = arena {
                if matches!(p.kind, P::Debris | P::Shard) {
                    // prallen an Wänden ab
                    let walls = a.near(nx as f64, ny as f64);
                    if circle_hits_walls(nx as f64, p.y as f64, 2.0, walls.clone()) {
                        p.vx = -p.vx * 0.4;
                        nx = p.x;
                    }
                    if circle_hits_walls(nx as f64, ny as f64, 2.0, walls) {
                        p.vy = -p.vy * 0.4;
                        ny = p.y;
                    }
                }
            }
            p.x = nx;
            p.y = ny;
            p.angle += p.spin * dt;
            true
        });
        landed
    }
}
