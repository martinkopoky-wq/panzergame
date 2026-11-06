//! Alle Spielkonstanten an einer Stelle (Werte wie in der Python-Version).

use std::f64::consts::PI;

pub const PORT: u16 = 5555;
/// Protokollversion, Server und Client müssen übereinstimmen
pub const PROTO: u16 = 11;
pub const MAX_PLAYERS: usize = 8;
/// mehr passen nicht sinnvoll auf eine Tastatur
pub const MAX_LOCAL: usize = 4;
/// ab so vielen Spielern wird das Labyrinth größer
pub const BIG_FROM: usize = 5;
pub const W: f64 = 1000.0;
pub const H: f64 = 680.0;
pub const ARENA_TOP: f64 = 15.0;
pub const ARENA_H: f64 = 560.0;
/// Höhe des Spielfelds auf dem Bildschirm (darunter HUD)
pub const VIEW_H: f64 = ARENA_TOP + ARENA_H + 10.0;
pub const TICK: f64 = 1.0 / 60.0;
/// Snapshots pro Sekunde vom Server
pub const SNAP_RATE: u32 = 60;
/// so weit in der Vergangenheit rendert der Client die anderen
pub const INTERP_DELAY: f64 = 0.075;

pub const CELL: f64 = 90.0;
pub const WALL: f64 = 8.0;
pub const TANK_R: f64 = 15.0;
pub const TANK_SPEED: f64 = 130.0;
pub const TANK_ROT: f64 = 200.0 * PI / 180.0;
/// Abstand Mitte -> Rohrende
pub const MUZZLE: f64 = 20.0;
pub const BULLET_R: f64 = 4.0;
pub const BULLET_SPEED: f64 = 220.0;
pub const BULLET_LIFE: f64 = 10.0;
pub const MAX_BULLETS: usize = 5;
pub const FIRE_COOLDOWN: f64 = 0.12;
/// so lange kann man sich nicht selbst treffen
pub const OWNER_GRACE: f64 = 0.1;
pub const ROUND_END_DELAY: f64 = 3.0;
/// so lange ist man nach dem Erscheinen unverwundbar
pub const SPAWN_SAFE: f64 = 2.0;

// Power-ups
pub const BOX_FIRST: f64 = 4.0;
pub const BOX_EVERY: (f64, f64) = (6.0, 11.0);
pub const BOX_MAX: usize = 3;

pub const FRAG_R: f64 = 7.0;
pub const FRAG_SPEED: f64 = 170.0;
pub const FRAG_FUSE: f64 = 5.0;
pub const FRAG_PIECES: usize = 26;
pub const MG_RATE: f64 = 0.07;
pub const MG_SPREAD: f64 = 0.12;
pub const MG_R: f64 = 3.0;
pub const MG_LIFE: f64 = 4.0;
pub const LASER_LEN: f64 = 1400.0;
pub const LASER_SHOW: f64 = 0.45;
pub const ROCKET_R: f64 = 5.0;
pub const ROCKET_SPEED: f64 = 170.0;
pub const ROCKET_TURN: f64 = 4.0;
pub const ROCKET_LIFE: f64 = 10.0;
pub const ROCKET_DELAY: f64 = 0.4;
pub const ROCKET_GRACE: f64 = 0.8;
pub const SMOKE_LIFE: f64 = 0.7;
pub const SHIELD_TIME: f64 = 8.0;
pub const SG_PELLETS: usize = 7;
pub const SG_SPREAD: f64 = 0.32;
pub const SG_LIFE: f64 = 1.1;
pub const BOUNCY_R: f64 = 6.0;
pub const BOUNCY_SPEED: f64 = 150.0;
pub const BOUNCY_GAIN: f64 = 1.15;
pub const BOUNCY_MAX: f64 = 560.0;
pub const BOUNCY_LIFE: f64 = 7.0;
pub const MINE_R: f64 = 7.0;
pub const MINE_ARM: f64 = 1.0;
pub const MINE_BLAST: f64 = 44.0;
pub const SWAP_R: f64 = 6.0;
pub const SWAP_SPEED: f64 = 300.0;
pub const SWAP_LIFE: f64 = 2.5;
pub const RANG_R: f64 = 6.0;
pub const RANG_SPEED: f64 = 250.0;
pub const RANG_CURVE: f64 = 2.0;
pub const RANG_TURN: f64 = 5.0;
pub const RANG_BACK: f64 = 0.8;
pub const RANG_LIFE: f64 = 5.0;
pub const ICE_R: f64 = 5.0;
pub const ICE_SPEED: f64 = 290.0;
pub const ICE_LIFE: f64 = 3.0;
pub const FREEZE_TIME: f64 = 3.0;
pub const HOLE_R: f64 = 7.0;
pub const HOLE_SPEED: f64 = 130.0;
pub const HOLE_TRAVEL: f64 = 1.2;
pub const HOLE_TIME: f64 = 4.5;
pub const HOLE_RANGE: f64 = 210.0;
pub const HOLE_PULL: f64 = 175.0;
pub const HOLE_KILL: f64 = 16.0;
pub const HOLE_SHOT_PULL: f64 = 650.0;
pub const STRIKE_DIST: f64 = 280.0;
pub const STRIKE_DELAY: f64 = 1.5;
pub const STRIKE_BLAST: f64 = 62.0;
// Flammenwerfer: Feuer halten, jede Flamme wird schnell langsamer und verglüht
pub const FLAME_RATE: f64 = 0.03;
pub const FLAME_SPREAD: f64 = 0.16;
pub const FLAME_SPEED: f64 = 340.0;
pub const FLAME_LIFE: f64 = 0.42;
pub const FLAME_R: f64 = 7.0;
pub const FLAME_DRAG: f64 = 2.2;
// Blitz: springt auf den nächsten Panzer vor einem und von dort weiter
pub const ZAP_RANGE: f64 = 250.0;
pub const ZAP_CHAIN: f64 = 200.0;
pub const ZAP_JUMPS: usize = 3;
pub const ZAP_CONE: f64 = 1.0;
pub const ZAP_SHOW: f64 = 0.45;
// Atombombe (nur aus der Wundertüte): rollt aus und geht nach NUKE_FUSE s hoch
pub const NUKE_R: f64 = 10.0;
pub const NUKE_SPEED: f64 = 150.0;
pub const NUKE_DRAG: f64 = 1.3;
pub const NUKE_FUSE: f64 = 4.0;
pub const NUKE_BLAST: f64 = 230.0;
pub const NUKE_CHANCE: f64 = 0.15;
// Railgun: Feuer halten zum Aufladen, loslassen feuert einen Strahl durch bis zu 2 Wände
pub const RAIL_CHARGE: f64 = 0.9;
pub const RAIL_LEN: f64 = 1500.0;
pub const RAIL_WALLS: usize = 2;
pub const RAIL_SHOW: f64 = 0.5;
// Fernlenkrakete: der Panzer steht, Links/Rechts lenken die Rakete, 2. Druck zündet
pub const RC_SPEED: f64 = 185.0;
pub const RC_TURN: f64 = 3.4;
pub const RC_LIFE: f64 = 6.0;
pub const RC_BLAST: f64 = 36.0;
pub const SENTRY_LIFE: f64 = 9.0;
pub const SENTRY_RANGE: f64 = 380.0;
pub const SENTRY_RATE: f64 = 0.55;
pub const SENTRY_TURN: f64 = 5.0;
pub const SENTRY_R: f64 = 10.0;
/// Wandbauer: Länge und Haltbarkeit der Wand
pub const BUILD_LEN: f64 = 66.0;
pub const BUILD_TIME: f64 = 10.0;
/// Öl brennt, wenn Feuer drankommt
pub const OIL_R: f64 = 34.0;
pub const OIL_LIFE: f64 = 14.0;
pub const OIL_BURN: f64 = 2.5;
pub const STICKY_SPEED: f64 = 260.0;
pub const STICKY_FUSE: f64 = 1.5;
pub const STICKY_BLAST: f64 = 40.0;
/// Granate fliegt über Wände
pub const GL_SPEED: f64 = 320.0;
pub const GL_DIST: f64 = 240.0;
pub const GL_BLAST: f64 = 38.0;
pub const SWARM_N: usize = 5;
pub const SWARM_SPEED: f64 = 210.0;
pub const SWARM_TURN: f64 = 3.0;
pub const SWARM_LIFE: f64 = 3.2;
pub const SAW_R: f64 = 9.0;
pub const SAW_SPEED: f64 = 175.0;
pub const SAW_BOUNCES: u32 = 7;
pub const SAW_LIFE: f64 = 8.0;
pub const PLASMA_R: f64 = 8.0;
pub const PLASMA_SPEED: f64 = 150.0;
pub const PLASMA_WALLS: u32 = 3;
pub const PLASMA_LIFE: f64 = 6.0;
pub const FW_SPEED: f64 = 260.0;
pub const FW_FUSE: f64 = 0.7;
pub const FW_SPARKS: usize = 16;
pub const PUSH_RANGE: f64 = 170.0;
pub const PUSH_CONE: f64 = 0.65;
pub const PUSH_DIST: f64 = 120.0;
pub const SPIDER_SENSE: f64 = 230.0;
pub const SPIDER_SPEED: f64 = 85.0;
pub const SPIDER_BLAST: f64 = 42.0;
pub const SPIDER_LIFE: f64 = 20.0;
pub const RAM_TIME: f64 = 4.0;
pub const NOVA_N: usize = 12;
pub const SEEK_SPEED: f64 = 200.0;
pub const SEEK_TURN: f64 = 2.4;
pub const SEEK_LIFE: f64 = 6.0;
pub const PHASE_SPEED: f64 = 300.0;
pub const PHASE_LIFE: f64 = 2.2;
pub const CANNON_R: f64 = 11.0;
pub const CANNON_SPEED: f64 = 160.0;
pub const CANNON_BOUNCES: u32 = 2;
pub const ARTY_DIST: [f64; 3] = [110.0, 190.0, 270.0];
pub const ARTY_BLAST: f64 = 44.0;
pub const TRAP_TIME: f64 = 4.0;
pub const JUMP_DIST: f64 = 95.0;
pub const FROST_R: f64 = 130.0;
pub const ORBIT_DIST: f64 = 240.0;
pub const ORBIT_DELAY: f64 = 1.6;
pub const ORBIT_TIME: f64 = 1.6;
pub const ORBIT_SPEED: f64 = 120.0;
pub const ORBIT_R: f64 = 24.0;
pub const PRISM_SPLIT: f64 = 0.42;
pub const PRISM_LEN: f64 = 900.0;
pub const METEOR_N: usize = 9;
pub const METEOR_BLAST: f64 = 42.0;
pub const GOLD_SPEED: f64 = 330.0;
pub const GOLD_LIFE: f64 = 6.0;
pub const SPLIT_SPEED: f64 = 250.0;
pub const SPLIT_LIFE: f64 = 5.0;
/// so oft teilt sich ein Querschläger (3 = höchstens 8 Kugeln)
pub const SPLIT_GENS: u8 = 3;
pub const SPLIT_ANGLE: f64 = 0.3;
pub const WAVE_SPEED: f64 = 240.0;
pub const WAVE_LIFE: f64 = 5.0;
pub const WAVE_FREQ: f64 = 9.0;
pub const WAVE_SWING: f64 = 5.5;
pub const TIMEBOMB_FUSE: f64 = 3.5;
pub const TIMEBOMB_BLAST: f64 = 95.0;
pub const NAPALM_SPEED: f64 = 270.0;
pub const NAPALM_FUSE: f64 = 1.3;
pub const NAPALM_R: f64 = 62.0;
pub const NAPALM_BURN: f64 = 4.5;
pub const FENCE_ARM: f64 = 0.8;
pub const FENCE_TIME: f64 = 6.0;
/// reicht in jede Richtung bis zur nächsten Wand, höchstens so weit
pub const FENCE_LEN: f64 = 260.0;
pub const SPRINKLER_ARM: f64 = 0.6;
pub const SPRINKLER_TIME: f64 = 3.2;
pub const SPRINKLER_RATE: f64 = 0.14;
pub const SPRINKLER_TURN: f64 = 3.2;
pub const SPRINKLER_SPEED: f64 = 230.0;
pub const SPRINKLER_LIFE: f64 = 2.0;
pub const HOOK_SPEED: f64 = 560.0;
pub const HOOK_LIFE: f64 = 0.9;
pub const HOOK_PULL: f64 = 520.0;
pub const HOOK_HOLD: f64 = 1.2;
pub const HOOK_STUN: f64 = 1.4;
pub const DESTRUCT_FUSE: f64 = 1.2;
pub const DESTRUCT_BLAST: f64 = 120.0;
pub const GIANT_SCALE: f64 = 1.5;
pub const TURBO_SPEED: f64 = 1.65;
pub const MINI_SCALE: f64 = 0.55;
pub const MINI_SPEED: f64 = 1.15;
pub const POTATO_TIME: f64 = 10.0;
pub const POTATO_MIN: f64 = 3.0;
pub const POTATO_BLAST: f64 = 55.0;
pub const POTATO_CHANCE: f64 = 0.25;
// Zeitlupe, sobald die Runde entschieden ist (Panzer fahren normal, alles andere langsam)
pub const SLOWMO_TIME: f64 = 1.5;
pub const SLOWMO_SCALE: f64 = 0.25;
// Ansagen: Mehrfachkill innerhalb dieser Zeit, Trickshot ab so vielen Abprallern
pub const MULTI_WINDOW: f64 = 1.6;
pub const TRICK_BOUNCES: u32 = 3;
pub const ANN_TIME: f64 = 1.9;
pub const MUTATOR_CHANCE: f64 = 0.35;
pub const FOG_R: f64 = 165.0;
pub const TAUNT_TIME: f64 = 2.5;
pub const FEED_TIME: f64 = 5.0;
pub const FEED_MAX: usize = 4;
/// Abtastschritt, mit dem ein Strahl die Wände seiner Zellen sammelt
pub const RAY_STEP: f64 = 20.0;

// Tasten-Bits. Nicht FIRE nennen: so heißt auch die Partikelart Feuer
pub const UP: u8 = 1;
pub const DOWN: u8 = 2;
pub const LEFT: u8 = 4;
pub const RIGHT: u8 = 8;
pub const SHOOT: u8 = 16;
/// Bits 5-7 der Tasten: Nummer des Spruchs (1-7), 0 = keiner
pub const TAUNT_SHIFT: u8 = 5;

pub type Rgb = [u8; 3];

pub const COLORS: [Rgb; MAX_PLAYERS] = [
    [220, 35, 35],
    [35, 190, 45],
    [45, 95, 225],
    [240, 140, 15],
    [150, 60, 200],
    [20, 175, 185],
    [235, 85, 170],
    [130, 85, 40],
];
/// Farbnamen: für lokale Spieler und Bots
pub const COLOR_NAMES: [&str; MAX_PLAYERS] = ["Rot", "Grün", "Blau", "Orange", "Lila", "Türkis", "Pink", "Braun"];

pub const BG: Rgb = [11, 12, 17];
pub const TEXT_C: Rgb = [232, 236, 244];
pub const DIM_C: Rgb = [138, 146, 164];
/// dunkle Schrift auf hellem Grund
pub const INK_C: Rgb = [28, 30, 38];
pub const SHOT_C: Rgb = [30, 30, 30];
pub const ACCENT_C: Rgb = [255, 196, 60];

/// Sprüche über die Tasten 1–7
pub const TAUNTS: [&str; 7] = ["Hehe!", "Ups …", "GG", "Zu langsam!", "Komm her!", "JUlIAN SPRING", "SCHEIß JUNKY"];

/// Sonderregel einer Runde
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Mutator {
    Kisten,
    Hyper,
    Riesen,
    Turbo,
    Nebel,
    Sniper,
    Dunkel,
}

pub const MUTATORS: [Mutator; 7] = [
    Mutator::Kisten,
    Mutator::Hyper,
    Mutator::Riesen,
    Mutator::Turbo,
    Mutator::Nebel,
    Mutator::Sniper,
    Mutator::Dunkel,
];

impl Mutator {
    pub fn from_u8(v: u8) -> Option<Mutator> {
        MUTATORS.get(v as usize).copied()
    }

    pub fn name(self) -> &'static str {
        match self {
            Mutator::Kisten => "Kistenregen",
            Mutator::Hyper => "Hyperkugeln",
            Mutator::Riesen => "Riesenkugeln",
            Mutator::Turbo => "Alle auf Turbo",
            Mutator::Nebel => "Nebel",
            Mutator::Sniper => "Scharfschützen",
            Mutator::Dunkel => "Stromausfall",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            Mutator::Kisten => "Power-ups ohne Ende",
            Mutator::Hyper => "Kugeln fast doppelt so schnell",
            Mutator::Riesen => "dicke, langsame Kugeln",
            Mutator::Turbo => "alle Panzer fahren schneller",
            Mutator::Nebel => "man sieht nur, was direkt vor einem liegt",
            Mutator::Sniper => "nur 1 Kugel, aber pfeilschnell",
            Mutator::Dunkel => "Licht gibt es nur von Scheinwerfern und Feuer",
        }
    }

    /// Kurzname für die gespeicherten Einstellungen (bleibt stabil, auch wenn sich Texte ändern)
    pub fn key(self) -> &'static str {
        match self {
            Mutator::Kisten => "kisten",
            Mutator::Hyper => "hyper",
            Mutator::Riesen => "riesen",
            Mutator::Turbo => "turbo",
            Mutator::Nebel => "nebel",
            Mutator::Sniper => "sniper",
            Mutator::Dunkel => "dunkel",
        }
    }
}

/// Welche Sonderregeln ein Spiel benutzt
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RoundRule {
    /// manche Runden bekommen eine zufällige Sonderregel
    #[default]
    Random,
    /// nie eine Sonderregel
    Off,
    /// jede Runde dieselbe
    Only(Mutator),
}

impl RoundRule {
    /// Alle Möglichkeiten in der Reihenfolge der Menü-Auswahl
    pub fn all() -> Vec<RoundRule> {
        let mut v = vec![RoundRule::Random, RoundRule::Off];
        v.extend(MUTATORS.iter().map(|&m| RoundRule::Only(m)));
        v
    }

    pub fn key(self) -> &'static str {
        match self {
            RoundRule::Random => "zufall",
            RoundRule::Off => "keine",
            RoundRule::Only(m) => m.key(),
        }
    }

    /// Unbekannte Werte (z. B. aus einer neueren Version) ergeben die Standard-Regel
    pub fn from_key(key: &str) -> RoundRule {
        RoundRule::all().into_iter().find(|r| r.key() == key).unwrap_or_default()
    }

    pub fn name(self) -> &'static str {
        match self {
            RoundRule::Random => "Zufall",
            RoundRule::Off => "Keine",
            RoundRule::Only(m) => m.name(),
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            RoundRule::Random => "manche Runden bekommen eine zufällige Sonderregel",
            RoundRule::Off => "immer normale Runden, ohne Sonderregel",
            RoundRule::Only(m) => m.desc(),
        }
    }
}

/// Umgebungspartikel je Look
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ambient {
    Dust,
    Sand,
    Ember,
    Snow,
    Firefly,
    Neon,
}

/// Arenen-Look: der Server würfelt pro Runde einen aus (rein optisch)
pub struct Theme {
    pub name: &'static str,
    pub floor: Rgb,
    pub line: Rgb,
    pub wall: Rgb,
    pub hi: Rgb,
    pub lo: Rgb,
    pub track: Rgb,
    pub dust: Rgb,
    pub accent: Rgb,
    pub glow: Option<Rgb>,
    pub cracks: bool,
    pub amb: Ambient,
}

pub const THEMES: [Theme; 6] = [
    Theme {
        name: "Nachtbasis",
        floor: [34, 38, 49],
        line: [45, 51, 65],
        wall: [84, 96, 120],
        hi: [152, 170, 204],
        lo: [36, 42, 56],
        track: [25, 28, 37],
        dust: [128, 138, 160],
        accent: [110, 180, 255],
        glow: None,
        cracks: false,
        amb: Ambient::Dust,
    },
    Theme {
        name: "Wüste",
        floor: [70, 55, 40],
        line: [79, 63, 46],
        wall: [156, 122, 82],
        hi: [220, 186, 132],
        lo: [86, 64, 42],
        track: [54, 42, 30],
        dust: [196, 166, 120],
        accent: [255, 170, 80],
        glow: None,
        cracks: false,
        amb: Ambient::Sand,
    },
    Theme {
        name: "Vulkan",
        floor: [36, 27, 27],
        line: [47, 34, 33],
        wall: [72, 54, 50],
        hi: [150, 104, 84],
        lo: [30, 21, 21],
        track: [24, 17, 17],
        dust: [96, 74, 68],
        accent: [255, 110, 40],
        glow: Some([255, 80, 15]),
        cracks: true,
        amb: Ambient::Ember,
    },
    Theme {
        name: "Gletscher",
        floor: [30, 45, 62],
        line: [40, 58, 78],
        wall: [122, 172, 212],
        hi: [226, 244, 255],
        lo: [60, 96, 134],
        track: [22, 34, 48],
        dust: [205, 228, 248],
        accent: [150, 220, 255],
        glow: None,
        cracks: false,
        amb: Ambient::Snow,
    },
    Theme {
        name: "Dschungel",
        floor: [30, 43, 31],
        line: [38, 53, 38],
        wall: [72, 100, 60],
        hi: [132, 172, 104],
        lo: [34, 50, 29],
        track: [21, 31, 22],
        dust: [112, 144, 92],
        accent: [170, 240, 110],
        glow: None,
        cracks: false,
        amb: Ambient::Firefly,
    },
    Theme {
        name: "Neon-City",
        floor: [19, 16, 33],
        line: [52, 28, 82],
        wall: [34, 26, 64],
        hi: [90, 235, 255],
        lo: [255, 60, 200],
        track: [13, 11, 24],
        dust: [150, 100, 230],
        accent: [255, 80, 210],
        glow: Some([60, 150, 255]),
        cracks: false,
        amb: Ambient::Neon,
    },
];

// Batch 1 (Entwürfe von Codex, Werte als Startwerte für Spieltests)
/// Geschossschere: Reichweite, voller Öffnungswinkel, höchstens so viele Kugeln
pub const SHEAR_RANGE: f64 = 150.0;
pub const SHEAR_ARC: f64 = PI / 3.0;
pub const SHEAR_MAX: usize = 6;
/// Hohlringgranate: Wurfweite, Zündzeit, innerer und äußerer Radius des Rings
pub const RING_THROW: f64 = 200.0;
pub const RING_FUSE: f64 = 1.1;
pub const RING_IN: f64 = 35.0;
pub const RING_OUT: f64 = 85.0;
/// Distanzzünder: Tempo, Radius, Lebenszeit, Strecke bis zur Scharfschaltung
pub const DIST_SPEED: f64 = 420.0;
pub const DIST_R: f64 = 4.0;
pub const DIST_LIFE: f64 = 0.95;
pub const DIST_ARM: f64 = 140.0;
/// Kugelfresser: Tempo, Radius, Lebenszeit, Anzahl Kugeln, die er schluckt
pub const EATER_SPEED: f64 = 150.0;
pub const EATER_R: f64 = 8.0;
pub const EATER_LIFE: f64 = 4.0;
pub const EATER_MAX: u8 = 3;
/// Entwaffnungsfächer: Reichweite und halber Öffnungswinkel
pub const DISARM_RANGE: f64 = 110.0;
pub const DISARM_HALF: f64 = PI / 8.0;
/// Zeit, in der der Entwaffnete seine Waffe nicht wieder aufnehmen kann
pub const DISARM_PICK_WAIT: f64 = 0.7;

// Batch 2
/// Zweitaktgeschoss: Tempo, Radius, Lebenszeit, Dauer der Markierung
pub const DOUBLE_SPEED: f64 = 300.0;
pub const DOUBLE_R: f64 = 4.0;
pub const DOUBLE_LIFE: f64 = 3.0;
pub const DOUBLE_MARK: f64 = 2.0;
/// Eckenschuss: Tempo, Radius, Lebenszeit
pub const CORNER_SPEED: f64 = 230.0;
pub const CORNER_R: f64 = 4.0;
pub const CORNER_LIFE: f64 = 3.0;
/// Kettenzange: Reichweite, halber Winkel, Haltezeit
pub const CHAIN_RANGE: f64 = 38.0;
pub const CHAIN_HALF: f64 = PI / 6.0;
pub const CHAIN_HOLD: f64 = 0.9;
/// Schwenkhammer: Ausholzeit, Reichweite, halber Sektor
pub const HAMMER_WINDUP: f64 = 0.45;
pub const HAMMER_RANGE: f64 = 48.0;
pub const HAMMER_HALF: f64 = PI / 3.0;
/// Zündpause: Reichweite und Dauer des Einfrierens für normale Kugeln
pub const PAUSE_RANGE: f64 = 110.0;
pub const PAUSE_TIME: f64 = 0.9;
/// Köderladung: wie weit geworfen, scharf nach, Lebenszeit, Zündzeit, Auslöse-Abstand, Radius
pub const DECOY_THROW: f64 = 160.0;
pub const DECOY_ARM: f64 = 0.8;
pub const DECOY_LIFE: f64 = 7.0;
pub const DECOY_FUSE: f64 = 0.35;
pub const DECOY_SENSE: f64 = 14.0;
pub const DECOY_BLAST: f64 = 55.0;
/// Schattensalve: Vorwarnzeit, Tempo, Lebenszeit, Winkelabstand der drei Kugeln
pub const SHADOW_FUSE: f64 = 0.9;
pub const SHADOW_SPEED: f64 = 220.0;
pub const SHADOW_LIFE: f64 = 1.5;
pub const SHADOW_SPREAD: f64 = 0.26;
/// Rohrverdreher: Vorwarnzeit und Reichweite
pub const TWIST_FUSE: f64 = 0.5;
pub const TWIST_RANGE: f64 = 100.0;

// Batch 3
/// Geschossweiche: Abstand vor dem Panzer, Dauer, Radius, höchstens so viele Kugeln
pub const SWITCH_DIST: f64 = 120.0;
pub const SWITCH_LIFE: f64 = 2.0;
pub const SWITCH_R: f64 = 40.0;
pub const SWITCH_MAX: u8 = 5;
/// Nachbrennbild: Vorschau, Länge und halbe Breite der Linie
pub const AFTERBURN_FUSE: f64 = 0.9;
pub const AFTERBURN_LEN: f64 = 320.0;
pub const AFTERBURN_HALF_W: f64 = 8.0;
/// Richtungsdorn: Scharfschaltung, Lebenszeit, Reichweite über der Leiste
pub const THORN_FUSE: f64 = 0.7;
pub const THORN_LIFE: f64 = 8.0;
pub const THORN_REACH: f64 = 25.0;
/// Trittplatte: Radius, Lebenszeit, Anzahl Betritte bis zur Warnung, Vorwarnung, Explosionsradius
pub const PLATE_R: f64 = 24.0;
pub const PLATE_LIFE: f64 = 8.0;
pub const PLATE_COUNT: u8 = 3;
pub const PLATE_WARN: f64 = 0.7;
pub const PLATE_BLAST: f64 = 65.0;
/// Zangenmarkierung: Zielabstand, seitlicher Abstand der Zangen, Tempo, Radius, Vorlauf, Lebenszeit
pub const PINCER_DIST: f64 = 240.0;
pub const PINCER_LAT: f64 = 80.0;
pub const PINCER_SPEED: f64 = 100.0;
pub const PINCER_R: f64 = 25.0;
pub const PINCER_FUSE: f64 = 0.7;
pub const PINCER_LIFE: f64 = 6.0;
/// Fadenkreuzfluch: Reichweite, Verfolgung, Einfrieren, Radius
pub const CROSS_RANGE: f64 = 260.0;
pub const CROSS_FOLLOW: f64 = 0.8;
pub const CROSS_FREEZE: f64 = 0.65;
pub const CROSS_R: f64 = 38.0;
/// Kreuzpendel: Armlänge, Drehtempo, Vorwarnung, Lebenszeit, Trefferradius
pub const PENDEL_LEN: f64 = 65.0;
pub const PENDEL_SPEED: f64 = std::f64::consts::PI / 1.4;
pub const PENDEL_WARN: f64 = 0.8;
pub const PENDEL_LIFE: f64 = 5.8;
pub const PENDEL_R: f64 = 10.0;

// Batch 4
/// Stillstandsurteil: Reichweite, Markierungsdauer, Stillstandszeit, Toleranz
pub const VERDICT_RANGE: f64 = 220.0;
pub const VERDICT_LIFE: f64 = 2.5;
pub const VERDICT_STILL: f64 = 1.1;
pub const VERDICT_TOL: f64 = 12.0;
/// Geschossbrüter: Lebenszeit, Fangradius, Kugeln bis zum Zerbrechen
pub const CATCHER_LIFE: f64 = 4.0;
pub const CATCHER_R: f64 = 28.0;
pub const CATCHER_MAX: u8 = 3;
/// Konterklinge: Abwehrwinkel halb, Abwehrreichweite, Stoß
pub const PARRY_HALF: f64 = std::f64::consts::PI * 35.0 / 180.0;
pub const PARRY_REACH: f64 = 60.0;
pub const LUNGE_REACH: f64 = 45.0;
/// Rostwolke: Wurfweite, Lebenszeit, Radius, Feuerpause in der Wolke
pub const RUST_THROW: f64 = 180.0;
pub const RUST_LIFE: f64 = 3.0;
pub const RUST_R: f64 = 55.0;
pub const RUST_PAUSE: f64 = 1.2;
/// Prallkissen: Wurfweite, Scharfschaltung, Lebenszeit, Radius, Stoß
pub const CUSHION_THROW: f64 = 180.0;
pub const CUSHION_ARM: f64 = 0.6;
pub const CUSHION_LIFE: f64 = 5.6;
pub const CUSHION_R: f64 = 28.0;
pub const CUSHION_PUSH: f64 = 90.0;
/// Kanonenschlag: Zündzeit, Splitter, Tempo
pub const CRACKER_FUSE: f64 = 1.0;
pub const CRACKER_N: usize = 3;
pub const CRACKER_SPEED: f64 = 170.0;
