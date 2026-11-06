//! Texte für Killfeed, Ansagen und Banner. Der Server schickt nur Slots und Ursachen,
//! der Client setzt mit den Spielernamen den Text zusammen.

use crate::items::Kind;
use crate::snap::{Ann, PopText};

pub const SELF_GENERIC: [&str; 3] =
    ["{v} hat sich selbst erledigt", "{v}: Eigentor!", "{v} wurde von der eigenen Kugel überrascht"];

fn kill_template(how: Kind) -> &'static str {
    use Kind::*;
    match how {
        Mg => "{k} hat {v} durchlöchert",
        Sg => "{k} hat {v} mit Schrot durchsiebt",
        Frag => "{k} hat {v} die Bombe direkt reingedrückt",
        Frg => "{k} hat {v} zerbröselt",
        Laser => "{k} hat {v} gegrillt",
        Rocket => "{k}s Rakete hat {v} gefunden",
        Bnc => "{k}s Flummi hat {v} erwischt",
        Rang => "{k} hat {v} mit dem Bumerang umgehauen",
        Mine => "{v} ist auf {k}s Mine getreten",
        Hole => "{k}s Schwarzes Loch hat {v} verschluckt",
        Strike => "{k} hat {v} aus der Luft erwischt",
        Potato => "{v} stand zu nah an {k}s Kartoffel",
        Fl => "{k} hat {v} flambiert",
        Zap => "{k} hat {v} unter Strom gesetzt",
        Nuke => "{k} hat {v} atomisiert",
        Rail => "{k} hat {v} durch die Wand gerailt",
        Rc => "{k} hat {v} ferngesteuert erwischt",
        Sb => "{k}s Geschützturm hat {v} erwischt",
        Sticky => "{k} hat {v} eine Klebebombe verpasst",
        Gl => "{k} hat {v} eine Granate über die Wand geworfen",
        Mrk => "{k}s Raketenschwarm hat {v} gefunden",
        Saw => "{k} hat {v} zersägt",
        Plasma => "{k} hat {v} mit Plasma geschmolzen",
        Spk => "{k}s Feuerwerk hat {v} erwischt",
        Spider => "{k}s Spinne hat {v} erwischt",
        Ram => "{k} hat {v} plattgewalzt",
        Star => "{k} ist als Superstern durch {v} gerauscht",
        Giant => "{k} hat {v} zertrampelt",
        Nv => "{k} hat {v} mit dem Kugelring erwischt",
        Seek => "{k}s Suchkugel hat {v} gefunden",
        Phase => "{k} hat {v} durch die Wand erschossen",
        Cannon => "{k} hat {v} weggeballert",
        Arty => "{k}s Artillerie hat {v} getroffen",
        Oil | Oilf => "{k} hat {v} im Öl angezündet",
        Orbit => "{k} hat {v} aus dem Orbit gegrillt",
        Prism => "{k} hat {v} durchs Prisma erwischt",
        Meteor => "{v} wurde von einem Meteoriten getroffen",
        Gold => "{k} hat {v} vergoldet",
        Split => "{k}s Querschläger hat {v} erwischt",
        Wave => "{k}s Schlangenkugel hat {v} erwischt",
        Timebomb => "{k}s Zeitbombe hat {v} erwischt",
        Napalm | Burn => "{k} hat {v} mit Napalm angezündet",
        Fence => "{v} ist in {k}s Laserschranke gefahren",
        Spb | Sprinkler => "{k}s Kugelsprinkler hat {v} erwischt",
        Destruct => "{k} hat {v} mit in die Luft gerissen",
        Ring => "{k}s Hohlringgranate hat {v} erwischt",
        Dist => "{k} hat {v} mit dem Distanzzünder getroffen",
        Eater => "{k}s Kugelfresser hat {v} erwischt",
        Double => "{k} hat {v} mit dem Zweitaktgeschoss erwischt",
        Corner => "{k}s Eckenschuss hat {v} um die Ecke getroffen",
        Chain => "{k} hat {v} mit der Kettenzange gepackt",
        Hammer => "{k}s Schwenkhammer hat {v} erwischt",
        Decoy => "{k}s Köder hat {v} in die Falle gelockt",
        Shadow => "{k}s Schattensalve hat {v} erwischt",
        Twist => "{k} hat {v} den Rohrverdreher verpasst",
        Switch => "{k}s Geschossweiche hat {v} erwischt",
        Afterburn => "{k}s Nachbrennbild hat {v} verbrannt",
        Thorn => "{k} hat {v} auf den Richtungsdorn gelockt",
        Plate => "{k}s Trittplatte hat {v} erwischt",
        Pincer => "{k}s Zangen haben {v} eingeklemmt",
        Cross => "{k} hat {v} mit dem Fadenkreuz festgenagelt",
        Pendel => "{k}s Kreuzpendel hat {v} erwischt",
        Verdict => "{k}s Stillstandsurteil hat {v} erwischt",
        Catcher => "{k}s Geschossbrüter hat {v} zurückgeworfen",
        Parry => "{k} hat {v} mit der Konterklinge aufgespießt",
        Rust => "{k}s Rostwolke hat {v} eingerostet",
        Cushion => "{k}s Prallkissen hat {v} weggeschleudert",
        Cracker => "{k}s Kanonenschlag hat {v} erwischt",
        _ => "{k} hat {v} abgeschossen",
    }
}

fn self_template(how: Kind) -> Option<&'static str> {
    use Kind::*;
    Some(match how {
        Laser => "{v} hat sich selbst gegrillt",
        Mine => "{v} ist auf die eigene Mine getreten",
        Hole => "{v} wurde vom eigenen Schwarzen Loch verschluckt",
        Strike => "{v} hat den Luftschlag auf sich selbst bestellt",
        Potato => "Die Kartoffel ist bei {v} explodiert",
        Bnc => "{v} hat den eigenen Flummi unterschätzt",
        Nuke => "{v} stand zu nah an der eigenen Atombombe",
        Rc => "{v} hat die eigene Rakete heimgelenkt",
        Sticky => "{v} klebt an der eigenen Bombe",
        Gl => "{v} hat die Granate zu kurz geworfen",
        Saw => "{v} wurde vom eigenen Sägeblatt erwischt",
        Sb => "{v} stand vor dem eigenen Geschützturm",
        Spider => "{v} wurde von der eigenen Spinne erwischt",
        Arty => "{v} stand im eigenen Artilleriefeuer",
        Oil | Oilf => "{v} ist im eigenen Öl verbrannt",
        Orbit => "{v} hat den Orbitallaser auf sich selbst gerichtet",
        Meteor => "{v} wurde von einem Meteoriten getroffen",
        Seek => "{v} wurde von der eigenen Suchkugel gefunden",
        Split => "{v} wurde vom eigenen Querschläger erwischt",
        Wave => "{v} wurde von der eigenen Schlangenkugel erwischt",
        Timebomb => "{v} hat die Zeitbombe unterschätzt",
        Napalm | Burn => "{v} ist im eigenen Napalm verbrannt",
        Fence => "{v} ist in die eigene Laserschranke gefahren",
        Spb | Sprinkler => "{v} stand im eigenen Kugelsprinkler",
        Destruct => "{v} hat sich selbst gesprengt",
        Ring => "{v} stand im eigenen Ring der Hohlringgranate",
        Dist => "{v} hat sich mit dem eigenen Distanzzünder getroffen",
        Eater => "{v} hat den eigenen Kugelfresser gefüttert",
        Double => "{v} hat sich mit dem eigenen Zweitaktgeschoss getroffen",
        Corner => "{v} ist im eigenen Eckenschuss hängen geblieben",
        Chain => "{v} hat sich mit der eigenen Kettenzange erwischt",
        Hammer => "{v} hat den eigenen Schwenkhammer abbekommen",
        Decoy => "{v} ist auf den eigenen Köder reingefallen",
        Shadow => "{v} wurde von der eigenen Schattensalve getroffen",
        Twist => "{v} hat den eigenen Rohrverdreher abbekommen",
        Switch => "{v} hat die eigene Geschossweiche erwischt",
        Afterburn => "{v} stand im eigenen Nachbrennbild",
        Thorn => "{v} ist auf den eigenen Richtungsdorn gefahren",
        Plate => "{v} ist auf die eigene Trittplatte getreten",
        Pincer => "{v} wurde von den eigenen Zangen eingeklemmt",
        Cross => "{v} wurde vom eigenen Fadenkreuz festgenagelt",
        Pendel => "{v} wurde vom eigenen Kreuzpendel erwischt",
        Verdict => "{v} hat das eigene Stillstandsurteil gesprochen bekommen",
        Catcher => "{v} hat den eigenen Geschossbrüter gefüttert",
        Parry => "{v} hat sich an der eigenen Konterklinge verletzt",
        Rust => "{v} ist in der eigenen Rostwolke stehen geblieben",
        Cushion => "{v} ist gegen das eigene Prallkissen geprallt",
        Cracker => "{v} stand im eigenen Kanonenschlag",
        _ => return None,
    })
}

/// Killfeed-Text. name(slot) liefert den Spielernamen.
pub fn kill_text(by: Option<usize>, victim: usize, how: Kind, variant: u8, name: &dyn Fn(usize) -> String) -> String {
    let v = name(victim);
    match by {
        Some(k) if k != victim => kill_template(how).replace("{k}", &name(k)).replace("{v}", &v),
        _ => self_template(how).unwrap_or(SELF_GENERIC[variant as usize % SELF_GENERIC.len()]).replace("{v}", &v),
    }
}

/// Untertitel zum Rundensieg, je nachdem wie viele Runden in Folge es waren.
pub fn streak_text(n: u32) -> String {
    match n {
        0 | 1 => String::new(),
        2 => "Zweiter Sieg in Folge".into(),
        3 => "HATTRICK!".into(),
        4 => "Nicht zu stoppen!".into(),
        n => format!("{n} Siege in Folge – UNAUFHALTSAM!"),
    }
}

pub fn win_text(slot: Option<usize>, name: &dyn Fn(usize) -> String) -> String {
    match slot {
        Some(s) => format!("{} gewinnt die Runde!", name(s)),
        None => "Unentschieden!".into(),
    }
}

/// (Haupttext, Untertitel) einer Ansage
pub fn ann_text(ann: Ann, slot: usize, name: &dyn Fn(usize) -> String) -> (String, String) {
    match ann {
        Ann::Multi(n) => (
            match n {
                2 => "DOPPELKILL!",
                3 => "DREIFACHKILL!",
                _ => "MEGAKILL!",
            }
            .into(),
            name(slot),
        ),
        Ann::Trick(b) => ("TRICKSHOT!".into(), format!("{} · {b} Abpraller", name(slot))),
        Ann::FirstBlood => ("ERSTES BLUT!".into(), name(slot)),
        Ann::Disco => ("DISCO!".into(), "Alle Plätze getauscht".into()),
        Ann::Meteors => ("METEORITEN!".into(), "Sucht Deckung!".into()),
    }
}

pub fn pop_text(p: PopText) -> &'static str {
    match p {
        PopText::OwnGoal => "EIGENTOR!",
        PopText::Spare => "ERSATZTEIL!",
    }
}

/// Spielername prüfen und säubern. None = unbrauchbar.
pub fn clean_name(raw: &str) -> Option<String> {
    let mut out = String::new();
    for ch in raw.trim().chars() {
        if ch.is_alphanumeric() || " _-.!?'".contains(ch) {
            out.push(ch);
        }
        if out.chars().count() >= 16 {
            break;
        }
    }
    let out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = out.to_lowercase();
    if out.is_empty() || ["server", "system", "admin", "host"].contains(&lower.as_str()) {
        return None;
    }
    Some(out)
}

/// Chatnachricht säubern (Steuerzeichen raus, Länge begrenzen)
pub fn clean_chat(raw: &str) -> Option<String> {
    let s: String = raw.chars().filter(|c| !c.is_control()).take(120).collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}
