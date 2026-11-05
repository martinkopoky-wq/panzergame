//! Dedizierter Panzer-Server.
//!
//!     panzer-server [--port 5555] [--bots 0] [--admin-pass PW] [--bans bans.txt] [--quiet]
//!
//! Jede Option lässt sich auch per Umgebungsvariable setzen (PANZER_PORT, PANZER_BOTS,
//! PANZER_ADMIN_PASS, PANZER_BANS, PANZER_QUIET). Kommandozeile geht vor Umgebung.
//!
//! Befehle auf der Konsole (ohne /): players, kick, ban, unban, bans, mute, unmute, bots, say

use std::io::BufRead;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

use panzer_server::{Config, Server};

fn usage() -> ! {
    eprintln!(
        "panzer-server – dedizierter Server für Panzer\n\n\
         Optionen (in Klammern die Umgebungsvariable, die Kommandozeile hat Vorrang):\n  \
           --port N          UDP-Port, Standard 5555 (PANZER_PORT)\n  \
           --bots N          freie Plätze mit N Bots füllen, 0-7 (PANZER_BOTS)\n  \
           --admin-pass PW   Passwort für /login im Spiel, Admin: kick, ban, bots … (PANZER_ADMIN_PASS)\n  \
           --bans DATEI      Bannliste, Standard bans.txt (PANZER_BANS)\n  \
           --quiet           keine Logausgabe (PANZER_QUIET=1)\n\n\
         Konsolenbefehle: players, kick <spieler> [grund], ban <spieler> [grund],\n\
         unban <name|ip|nr>, bans, mute/unmute <spieler>, bots <n>, say <text>, help"
    );
    std::process::exit(2)
}

/// Liest die PANZER_*-Variablen. Leere Werte zählen als nicht gesetzt, damit etwa
/// `PANZER_ADMIN_PASS=` in einer Compose-Datei nicht zu einem leeren Passwort wird.
fn apply_env(cfg: &mut Config, env: impl Fn(&str) -> Option<String>) -> Result<(), String> {
    let get = |name: &str| env(name).filter(|v| !v.is_empty());
    if let Some(v) = get("PANZER_PORT") {
        cfg.port = v.parse().map_err(|_| format!("PANZER_PORT ist kein gültiger Port: {v}"))?;
    }
    if let Some(v) = get("PANZER_BOTS") {
        cfg.bots = v.parse::<usize>().map_err(|_| format!("PANZER_BOTS ist keine Zahl: {v}"))?.min(7);
    }
    if let Some(v) = get("PANZER_ADMIN_PASS") {
        cfg.admin_pass = Some(v);
    }
    if let Some(v) = get("PANZER_BANS") {
        cfg.bans_file = Some(PathBuf::from(v));
    }
    if let Some(v) = get("PANZER_QUIET") {
        cfg.verbose = match v.to_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => false,
            "0" | "false" | "no" | "off" => true,
            _ => return Err(format!("PANZER_QUIET muss 1 oder 0 sein: {v}")),
        };
    }
    Ok(())
}

fn main() {
    let mut cfg = Config::default();
    if let Err(e) = apply_env(&mut cfg, |name| std::env::var(name).ok()) {
        eprintln!("{e}");
        std::process::exit(2);
    }
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut val = || args.next().unwrap_or_else(|| usage());
        match a.as_str() {
            "--port" | "-p" => cfg.port = val().parse().unwrap_or_else(|_| usage()),
            "--bots" | "-b" => cfg.bots = val().parse::<usize>().unwrap_or_else(|_| usage()).min(7),
            "--admin-pass" => cfg.admin_pass = Some(val()),
            "--bans" => cfg.bans_file = Some(PathBuf::from(val())),
            "--quiet" | "-q" => cfg.verbose = false,
            _ => usage(),
        }
    }
    if cfg.admin_pass.is_none() {
        println!("Hinweis: ohne --admin-pass kann nur die Konsole kicken/bannen.");
    }
    let mut server = match Server::bind(cfg) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Port kann nicht geöffnet werden: {e}");
            std::process::exit(1);
        }
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    server.set_console(rx);
    let stop = Arc::new(AtomicBool::new(false));
    server.run(&stop);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_env(vars: &[(&str, &str)]) -> Result<Config, String> {
        let mut cfg = Config::default();
        apply_env(&mut cfg, |name| vars.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string()))?;
        Ok(cfg)
    }

    #[test]
    fn alle_variablen_werden_gelesen() {
        let cfg = from_env(&[
            ("PANZER_PORT", "6000"),
            ("PANZER_BOTS", "3"),
            ("PANZER_ADMIN_PASS", "geheim"),
            ("PANZER_BANS", "/data/bans.txt"),
            ("PANZER_QUIET", "1"),
        ])
        .unwrap();
        assert_eq!(cfg.port, 6000);
        assert_eq!(cfg.bots, 3);
        assert_eq!(cfg.admin_pass.as_deref(), Some("geheim"));
        assert_eq!(cfg.bans_file, Some(PathBuf::from("/data/bans.txt")));
        assert!(!cfg.verbose);
    }

    #[test]
    fn leere_werte_zaehlen_als_nicht_gesetzt() {
        let cfg = from_env(&[("PANZER_PORT", ""), ("PANZER_ADMIN_PASS", ""), ("PANZER_QUIET", "")]).unwrap();
        let default = Config::default();
        assert_eq!(cfg.port, default.port);
        assert_eq!(cfg.admin_pass, None);
        assert!(cfg.verbose);
    }

    #[test]
    fn bots_werden_wie_auf_der_kommandozeile_begrenzt() {
        assert_eq!(from_env(&[("PANZER_BOTS", "99")]).unwrap().bots, 7);
    }

    #[test]
    fn ungueltige_werte_sind_ein_fehler() {
        assert!(from_env(&[("PANZER_PORT", "99999")]).is_err());
        assert!(from_env(&[("PANZER_BOTS", "viele")]).is_err());
        assert!(from_env(&[("PANZER_QUIET", "vielleicht")]).is_err());
    }

    #[test]
    fn quiet_laesst_sich_ausdruecklich_abschalten() {
        assert!(from_env(&[("PANZER_QUIET", "0")]).unwrap().verbose);
    }
}
