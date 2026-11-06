//! Power-ups (Item), Geschoss- bzw. Trefferarten (Kind) und Panzer-Effekte (Eff).
//! Alles als kompakte Enums, damit es billig übers Netz geht.

/// Herkunft eines Power-ups
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    /// Waffe aus der Kiste
    Weapon,
    /// Effekt aus der Kiste (wirkt sofort beim Aufheben)
    Buff,
    /// Waffe nur aus der Wundertüte
    MysteryWeapon,
    /// Effekt nur aus der Wundertüte
    MysteryBuff,
    /// nur für die Anzeige
    Display,
}

macro_rules! items {
    ($($v:ident = $name:expr, $class:ident, $num:expr;)*) => {
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum Item { $($v),* }

        pub const ALL_ITEMS: &[Item] = &[$(Item::$v),*];

        impl Item {
            pub fn name(self) -> &'static str {
                match self { $(Item::$v => $name),* }
            }
            pub fn class(self) -> Class {
                match self { $(Item::$v => Class::$class),* }
            }
            /// Schuss bei Waffen, Sekunden bei Effekten (0 = keine Angabe)
            pub fn amount(self) -> f64 {
                match self { $(Item::$v => $num as f64),* }
            }
        }
    };
}

// Alle Power-ups an einer Stelle: Name, Art und Zahl (Schuss bzw. Sekunden).
items! {
    Frag = "Splitterbombe", Weapon, 1;
    Mg = "MG", Weapon, 25;
    Laser = "Laser", Weapon, 1;
    Rocket = "Lenkrakete", Weapon, 1;
    Shotgun = "Schrotflinte", Weapon, 2;
    Bouncy = "Flummi", Weapon, 1;
    Mine = "Minen", Weapon, 3;
    Swap = "Tauschkugel", Weapon, 1;
    Rang = "Bumerang", Weapon, 1;
    Ice = "Eisstrahl", Weapon, 1;
    Hole = "Schwarzes Loch", Weapon, 1;
    Strike = "Luftschlag", Weapon, 1;
    Flame = "Flammenwerfer", Weapon, 70;
    Zap = "Blitz", Weapon, 1;
    Rail = "Railgun", Weapon, 1;
    Rc = "Fernlenkrakete", Weapon, 1;
    Sentry = "Geschützturm", Weapon, 1;
    Wall = "Wandbauer", Weapon, 3;
    Oil = "Ölfass", Weapon, 2;
    Sticky = "Klebebombe", Weapon, 2;
    Gl = "Granatwerfer", Weapon, 3;
    Swarm = "Raketenschwarm", Weapon, 1;
    Saw = "Sägeblatt", Weapon, 1;
    Plasma = "Plasmakugel", Weapon, 1;
    Fw = "Feuerwerk", Weapon, 2;
    Push = "Druckwelle", Weapon, 2;
    Spider = "Spinnenmine", Weapon, 2;
    Ram = "Rammbock", Weapon, 1;
    Nova = "Kugelring", Weapon, 1;
    Seek = "Suchkugel", Weapon, 3;
    Phase = "Phasenkugel", Weapon, 2;
    Cannon = "Kanonenkugel", Weapon, 1;
    Arty = "Artillerie", Weapon, 1;
    Trap = "Bärenfalle", Weapon, 2;
    Jump = "Sprungdüse", Weapon, 3;
    Frost = "Frostnova", Weapon, 1;
    Orbit = "Orbitallaser", Weapon, 1;
    Prism = "Prisma", Weapon, 1;
    Split = "Querschläger", Weapon, 1;
    Wave = "Schlangenkugel", Weapon, 3;
    Timebomb = "Zeitbombe", Weapon, 2;
    Napalm = "Napalm", Weapon, 2;
    Fence = "Laserschranke", Weapon, 2;
    Sprinkler = "Kugelsprinkler", Weapon, 1;
    Hook = "Enterhaken", Weapon, 3;
    Destruct = "Selbstzerstörung", Weapon, 1;
    Shear = "Geschossschere", Weapon, 2;
    Ring = "Hohlringgranate", Weapon, 2;
    Dist = "Distanzzünder", Weapon, 3;
    Eater = "Kugelfresser", Weapon, 1;
    Disarm = "Entwaffnungsfächer", Weapon, 1;
    Double = "Zweitaktgeschoss", Weapon, 4;
    Corner = "Eckenschuss", Weapon, 3;
    Chain = "Kettenzange", Weapon, 1;
    Hammer = "Schwenkhammer", Weapon, 2;
    Pause = "Zündpause", Weapon, 1;
    Decoy = "Köderladung", Weapon, 2;
    Shadow = "Schattensalve", Weapon, 1;
    Twist = "Rohrverdreher", Weapon, 1;
    Switch = "Geschossweiche", Weapon, 2;
    Afterburn = "Nachbrennbild", Weapon, 2;
    Thorn = "Richtungsdorn", Weapon, 2;
    Plate = "Trittplatte", Weapon, 2;
    Pincer = "Zangenmarkierung", Weapon, 1;
    Cross = "Fadenkreuzfluch", Weapon, 1;
    Pendel = "Kreuzpendel", Weapon, 1;
    Verdict = "Stillstandsurteil", Weapon, 1;
    Catcher = "Geschossbrüter", Weapon, 1;
    Parry = "Konterklinge", Weapon, 2;
    Rust = "Rostwolke", Weapon, 2;
    Cushion = "Prallkissen", Weapon, 2;
    Cracker = "Kanonenschlag", Weapon, 2;
    Shield = "Schild", Buff, 0;
    Turbo = "Turbo", Buff, 6;
    Mini = "Schrumpfpilz", Buff, 10;
    Ghost = "Geist", Buff, 5;
    Invis = "Tarnkappe", Buff, 8;
    Confuse = "Verwirrung", Buff, 6;
    Mirror = "Spiegelschild", Buff, 7;
    Star = "Superstern", Buff, 6;
    Giant = "Riesenpilz", Buff, 9;
    Rapid = "Schnellfeuer", Buff, 8;
    Extra = "Ersatzteil", Buff, 999;
    Slow = "Lähmung", Buff, 5;
    Twin = "Doppellauf", Buff, 10;
    Tp = "Teleporter", Buff, 0;
    Disco = "Discokugel", Buff, 0;
    Radar = "Radar", Buff, 12;
    Nuke = "Atombombe", MysteryWeapon, 1;
    Gold = "Goldene Kugel", MysteryWeapon, 1;
    Potato = "Heiße Kartoffel", MysteryBuff, 0;
    Meteor = "Meteoritenschauer", MysteryBuff, 0;
    Mystery = "Wundertüte", Display, 0;
    Frozen = "Eingefroren", Display, 0;
}

impl Item {
    pub fn from_u8(v: u8) -> Option<Item> {
        ALL_ITEMS.get(v as usize).copied()
    }

    /// belegt den Waffenplatz (auch die aus der Wundertüte)
    pub fn is_weapon(self) -> bool {
        matches!(self.class(), Class::Weapon | Class::MysteryWeapon)
    }

    /// Waffe, die es in Kisten gibt
    pub fn is_box_weapon(self) -> bool {
        self.class() == Class::Weapon
    }

    pub fn is_buff(self) -> bool {
        self.class() == Class::Buff
    }

    pub fn mystery_only(self) -> bool {
        matches!(self.class(), Class::MysteryWeapon | Class::MysteryBuff)
    }

    /// Waffen, bei denen man Feuer gedrückt hält
    pub fn is_hold(self) -> bool {
        matches!(self, Item::Mg | Item::Flame | Item::Rail)
    }

    /// Munition der Waffe (1 = Einzelschuss)
    pub fn ammo(self) -> i32 {
        self.amount() as i32
    }

    /// hat die Waffe eine Munitionsanzeige (mehr als ein Schuss)?
    pub fn has_ammo(self) -> bool {
        self.is_weapon() && self.amount() > 1.0
    }

    /// Effekt, den das Power-up auf dem Panzer setzt
    pub fn effect(self) -> Option<Eff> {
        Some(match self {
            Item::Turbo => Eff::Turbo,
            Item::Mini => Eff::Mini,
            Item::Ghost => Eff::Ghost,
            Item::Invis => Eff::Invis,
            Item::Confuse => Eff::Confuse,
            Item::Mirror => Eff::Mirror,
            Item::Star => Eff::Star,
            Item::Giant => Eff::Giant,
            Item::Rapid => Eff::Rapid,
            Item::Extra => Eff::Extra,
            Item::Slow => Eff::Slow,
            Item::Twin => Eff::Twin,
            Item::Radar => Eff::Radar,
            _ => return None,
        })
    }
}

pub fn box_weapons() -> Vec<Item> {
    ALL_ITEMS.iter().copied().filter(|i| i.is_box_weapon()).collect()
}

pub fn buffs() -> Vec<Item> {
    ALL_ITEMS.iter().copied().filter(|i| i.is_buff()).collect()
}

/// Was in Kisten liegen kann (die Wundertüte doppelt, wie im Original)
pub fn box_pool() -> Vec<Item> {
    let mut v = box_weapons();
    v.extend(buffs());
    v.push(Item::Mystery);
    v.push(Item::Mystery);
    v
}

macro_rules! kinds {
    ($($v:ident = $code:expr;)*) => {
        /// Geschossart und gleichzeitig Trefferursache (für Killfeed und Ansagen)
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
        #[repr(u8)]
        pub enum Kind { $($v),* }

        pub const ALL_KINDS: &[Kind] = &[$(Kind::$v),*];

        impl Kind {
            pub fn code(self) -> &'static str {
                match self { $(Kind::$v => $code),* }
            }
        }
    };
}

kinds! {
    B = "b"; Mg = "mg"; Sg = "sg"; Frag = "frag"; Frg = "frg"; Rocket = "rocket";
    Bnc = "bnc"; Swap = "swap"; Rang = "rang"; Ice = "ice"; Hole = "hole"; Strike = "strike";
    Fl = "fl"; Nuke = "nuke"; Rc = "rc"; Mrk = "mrk"; Sentry = "sentry"; Sb = "sb";
    Oil = "oil"; Oilf = "oilf"; Sticky = "sticky"; Gl = "gl"; Saw = "saw"; Plasma = "plasma";
    Fw = "fw"; Spk = "spk"; Spider = "spider"; Mine = "mine"; Trap = "trap"; Nv = "nv";
    Seek = "seek"; Phase = "phase"; Cannon = "cannon"; Arty = "arty"; Orbit = "orbit";
    Meteor = "meteor"; Gold = "gold"; Split = "split"; Wave = "wave"; Timebomb = "timebomb";
    Napalm = "napalm"; Burn = "burn"; Fence = "fence"; Sprinkler = "sprinkler"; Spb = "spb";
    Hook = "hook"; Destruct = "destruct";
    Ring = "ring"; Dist = "dist"; Eater = "eater";
    Double = "double"; Corner = "corner"; Chain = "chain"; Hammer = "hammer"; Decoy = "decoy";
    Shadow = "shadow"; Twist = "twist";
    Switch = "switch"; Afterburn = "afterburn"; Thorn = "thorn"; Plate = "plate";
    Pincer = "pincer"; Cross = "cross"; Pendel = "pendel";
    Verdict = "verdict"; Catcher = "catcher"; Parry = "parry"; Rust = "rust"; Cushion = "cushion"; Cracker = "cracker";
    // nur Trefferursachen
    Laser = "laser"; Zap = "zap"; Rail = "rail"; Prism = "prism"; Ram = "ram";
    Star = "star"; Giant = "giant"; Potato = "potato";
}

impl Kind {
    pub fn from_u8(v: u8) -> Option<Kind> {
        ALL_KINDS.get(v as usize).copied()
    }

    /// Abpraller zählen für Trickshots
    pub fn trick(self) -> bool {
        use Kind::*;
        matches!(self, B | Bnc | Mg | Sg | Laser | Saw | Cannon | Gold | Nv | Prism | Split | Wave)
    }

    /// Snapshot enthält den Flugwinkel
    pub fn angled(self) -> bool {
        matches!(self, Kind::Rocket | Kind::Rc | Kind::Mrk)
    }

    /// fliegen durch Panzer hindurch
    pub fn pierce(self) -> bool {
        matches!(self, Kind::Saw | Kind::Plasma | Kind::Cannon | Kind::Gold)
    }

    /// Liegende Objekte (und die Granate, die drüberfliegt): werden nicht angezogen,
    /// zünden keine Minen, Bots weichen ihnen nicht aus
    pub fn is_static(self) -> bool {
        use Kind::*;
        matches!(
            self,
            Mine | Strike
                | Hole
                | Nuke
                | Sentry
                | Oil
                | Oilf
                | Trap
                | Arty
                | Orbit
                | Meteor
                | Gl
                | Timebomb
                | Burn
                | Fence
                | Sprinkler
                | Destruct
                | Ring
                | Chain
                | Hammer
                | Decoy
                | Shadow
                | Twist
                | Afterburn
                | Thorn
                | Plate
                | Cross
                | Pendel
                | Verdict
                | Catcher
                | Rust
                | Cushion
                | Cracker
        )
    }

    /// Kugeln, die ein Spiegelschild zurückwirft
    pub fn reflect(self) -> bool {
        use Kind::*;
        matches!(self, B | Mg | Sg | Frg | Bnc | Nv | Sb | Spk | Seek | Ice | Mrk | Rc | Split | Wave | Spb)
    }

    /// Explosionsradius der Geschosse, die beim Treffer oder am Ende hochgehen
    pub fn blast(self) -> Option<f64> {
        use crate::consts::*;
        Some(match self {
            Kind::Rc => RC_BLAST,
            Kind::Spider => SPIDER_BLAST,
            Kind::Sticky => STICKY_BLAST,
            Kind::Gl => GL_BLAST,
            Kind::Arty => ARTY_BLAST,
            Kind::Meteor => METEOR_BLAST,
            Kind::Timebomb => TIMEBOMB_BLAST,
            Kind::Destruct => DESTRUCT_BLAST,
            Kind::Ring => RING_OUT,
            Kind::Decoy => DECOY_BLAST,
            Kind::Plate => PLATE_BLAST,
            _ => return None,
        })
    }

    /// Power-up-Symbol, das im Killfeed für diese Ursache steht
    pub fn icon(self) -> Option<Item> {
        use Kind::*;
        Some(match self {
            Frg | Frag => Item::Frag,
            Sg => Item::Shotgun,
            Bnc => Item::Bouncy,
            Fl => Item::Flame,
            Sb | Sentry => Item::Sentry,
            Mrk => Item::Swarm,
            Spk | Fw => Item::Fw,
            Nv => Item::Nova,
            Oilf | Oil => Item::Oil,
            Mg => Item::Mg,
            Rocket => Item::Rocket,
            Swap => Item::Swap,
            Rang => Item::Rang,
            Ice => Item::Ice,
            Hole => Item::Hole,
            Strike => Item::Strike,
            Nuke => Item::Nuke,
            Rc => Item::Rc,
            Sticky => Item::Sticky,
            Gl => Item::Gl,
            Saw => Item::Saw,
            Plasma => Item::Plasma,
            Spider => Item::Spider,
            Mine => Item::Mine,
            Trap => Item::Trap,
            Seek => Item::Seek,
            Phase => Item::Phase,
            Cannon => Item::Cannon,
            Arty => Item::Arty,
            Orbit => Item::Orbit,
            Meteor => Item::Meteor,
            Gold => Item::Gold,
            Split => Item::Split,
            Wave => Item::Wave,
            Timebomb => Item::Timebomb,
            Napalm | Burn => Item::Napalm,
            Fence => Item::Fence,
            Sprinkler | Spb => Item::Sprinkler,
            Hook => Item::Hook,
            Destruct => Item::Destruct,
            Ring => Item::Ring,
            Dist => Item::Dist,
            Eater => Item::Eater,
            Double => Item::Double,
            Corner => Item::Corner,
            Chain => Item::Chain,
            Hammer => Item::Hammer,
            Decoy => Item::Decoy,
            Shadow => Item::Shadow,
            Twist => Item::Twist,
            Switch => Item::Switch,
            Afterburn => Item::Afterburn,
            Thorn => Item::Thorn,
            Plate => Item::Plate,
            Pincer => Item::Pincer,
            Cross => Item::Cross,
            Pendel => Item::Pendel,
            Verdict => Item::Verdict,
            Catcher => Item::Catcher,
            Parry => Item::Parry,
            Rust => Item::Rust,
            Cushion => Item::Cushion,
            Cracker => Item::Cracker,
            Laser => Item::Laser,
            Zap => Item::Zap,
            Rail => Item::Rail,
            Prism => Item::Prism,
            Ram => Item::Ram,
            Star => Item::Star,
            Giant => Item::Giant,
            Potato => Item::Potato,
            B => return None,
        })
    }
}

macro_rules! effs {
    ($($v:ident,)*) => {
        /// Effekte auf einem Panzer (Restzeit in Tank::eff)
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
        #[repr(u8)]
        pub enum Eff { $($v),* }

        pub const ALL_EFFS: &[Eff] = &[$(Eff::$v),*];
    };
}

effs! {
    Turbo, Mini, Ghost, Invis, Confuse, Mirror, Star, Giant, Rapid, Extra, Slow, Twin, Radar,
    Ram, Potato, Frozen, Steer, Slip, Spawn, Inv, NoPass,
}

pub const EFF_N: usize = 21;

impl Eff {
    pub fn from_u8(v: u8) -> Option<Eff> {
        ALL_EFFS.get(v as usize).copied()
    }

    /// bleibt beim Server (geht nicht übers Netz)
    pub fn hidden(self) -> bool {
        matches!(self, Eff::Inv | Eff::NoPass)
    }

    /// ohne eigenes Symbol im HUD
    pub fn no_icon(self) -> bool {
        matches!(self, Eff::Steer | Eff::Slip | Eff::Spawn) || self.hidden()
    }

    /// Symbol im HUD
    pub fn icon(self) -> Item {
        match self {
            Eff::Turbo => Item::Turbo,
            Eff::Mini => Item::Mini,
            Eff::Ghost => Item::Ghost,
            Eff::Invis => Item::Invis,
            Eff::Confuse => Item::Confuse,
            Eff::Mirror => Item::Mirror,
            Eff::Star => Item::Star,
            Eff::Giant => Item::Giant,
            Eff::Rapid => Item::Rapid,
            Eff::Extra => Item::Extra,
            Eff::Slow => Item::Slow,
            Eff::Twin => Item::Twin,
            Eff::Radar => Item::Radar,
            Eff::Ram => Item::Ram,
            Eff::Potato => Item::Potato,
            Eff::Frozen => Item::Ice,
            _ => Item::Mystery,
        }
    }

    /// volle Laufzeit (für die Ringe im HUD)
    pub fn full_time(self) -> f64 {
        use crate::consts::*;
        match self {
            Eff::Ram => RAM_TIME,
            Eff::Potato => POTATO_TIME,
            Eff::Frozen => FREEZE_TIME,
            _ => {
                let item = self.icon();
                if item.amount() > 0.0 {
                    item.amount()
                } else {
                    FREEZE_TIME
                }
            }
        }
    }
}
