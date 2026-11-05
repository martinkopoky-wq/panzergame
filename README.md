# Panzer

Labyrinth-Panzerschlacht als Desktop-Spiel in **Rust** (macroquad), mit Multiplayer für bis zu 8 Spieler über das Netz, dediziertem Server mit Anti-Cheat, Chat, Kick/Bann, Computergegnern (Bots) und jeder Menge Krawall: Licht, Partikel, Zeitlupe, Ansagen und synthetisierter Musik. Es gibt keine Bild- oder Sounddateien, alles wird beim Start erzeugt (nur zwei Schriften liegen in `assets/fonts`).

Die alte Python-Version (`panzer.py`) liegt noch im Repo, ist aber nicht mehr kompatibel mit der neuen.

## Download

Fertige Installer für Windows, macOS und Linux gibt es auf der [Website](https://martinkopoky-wq.github.io/panzergame/) und unter [Releases](https://github.com/martinkopoky-wq/panzergame/releases). Installiert wird der Launcher, er lädt das Spiel beim ersten Start und hält es aktuell. Die Installer sind nicht signiert, Windows und macOS warnen deshalb beim ersten Start (Anleitung auf der Website).

## Aus dem Quellcode starten

Einfach das Startskript für dein System ausführen. Beim ersten Mal installiert es, was fehlt (Rust, unter Linux ein paar Bibliotheken, unter Windows die C++-Build-Tools), fragt vorher nach und baut das Spiel. Das dauert einmalig ein paar Minuten, danach startet es in Sekunden. Am einfachsten das Repo mit `git clone` holen, dann bleiben die Skripte ausführbar.

| System | Spiel | Server (ohne Grafik) |
|---|---|---|
| **Linux** | `./start.sh` | `./server_start.sh` |
| **macOS** | Doppelklick auf `start.command` (oder `./start.sh` im Terminal) | `server_start.command` bzw. `./server_start.sh` |
| **Windows** | Doppelklick auf `start.bat` | Doppelklick auf `server_start.bat` |

Argumente werden durchgereicht, zum Beispiel `./start.sh solo 3`, `start.bat join 100.64.0.5` oder `./server_start.sh --bots 3 --admin-pass geheim`.

Hinweise:

- **Linux:** Erkannt werden apt (Debian/Ubuntu/Mint), dnf (Fedora), pacman (Arch) und zypper (openSUSE). Für die Pakete wird `sudo` gebraucht. Der Server braucht nur einen C-Compiler, keine Grafik- oder Tonbibliotheken.
- **macOS:** Fehlen die Xcode-Kommandozeilen-Tools, öffnet sich ein Installationsfenster. Danach das Skript nochmal starten. Blockiert macOS die `.command`-Datei („nicht verifizierter Entwickler“), einmal Rechtsklick → *Öffnen*. Wurde das Repo als ZIP geladen und die Skripte sind nicht ausführbar: `chmod +x *.sh *.command scripts/*.sh`.
- **Windows:** Installiert wird über `winget` (Windows 10/11 hat das dabei): erst Rust, dann die „Build Tools für Visual Studio“ (C++). Nach der Rust-Installation muss das Skript eventuell einmal neu gestartet werden. Ohne winget öffnen sich die Download-Seiten. Beim ersten Hosten fragt die Windows-Firewall nach dem Netzwerkzugriff: zulassen, sonst kann niemand beitreten.
- Fertige Programme liegen danach in `target/release/` (`panzer`, `panzer-server`, unter Windows mit `.exe`). Die `panzer.exe` läuft ohne weitere Dateien und kann auch so an Freunde gehen.

## Aufbau

```
crates/core     Spiellogik, Labyrinth, Physik, Bots, Netzwerkprotokoll (ohne Grafik)
crates/server   Server: UDP, 60 Ticks/s, Anti-Cheat, Chat, Kick/Bann  -> panzer-server
crates/client   Spiel mit Grafik, Ton, Menü (hosten startet den Server im Spiel) -> panzer
crates/launcher Desktop-App: lädt das Spiel aus den GitHub-Releases, prüft es und startet es -> panzer-launcher
packaging/      Installer: AppImage und .deb, .dmg, Inno Setup
website/        Download-Seite (statisch, ohne Build-Schritt)
scripts/        gemeinsame Vorbereitung der Startskripte (setup.sh, setup.bat)
start.*, server_start.*   Startskripte für Linux (.sh), macOS (.command) und Windows (.bat)
```

Server und Client rechnen die Panzerbewegung mit demselben Code (`core/src/physics.rs`), deshalb stimmt die Vorhersage des eigenen Panzers exakt.

## Bauen von Hand

Voraussetzung: Rust (https://rustup.rs), unter Linux zusätzlich die Entwicklerpakete für ALSA, X11, Xi und OpenGL (bei Debian/Ubuntu: `sudo apt install libasound2-dev libx11-dev libxi-dev libgl1-mesa-dev`), unter Windows die C++-Build-Tools, unter macOS die Xcode-Kommandozeilen-Tools.

```bash
cargo build --release
./target/release/panzer              # Spiel
./target/release/panzer-server       # dedizierter Server
```

### Tests

```bash
cargo test --release
```

Bot-Matches mit 2–8 Spielern und ständig zufälligen Power-ups, verlustfreie Delta-Snapshots, und ein echter Server über UDP mit Test-Clients (Warteschlange, Chat, Kick/Bann, Speedhack-Bremse).

## Release

1. Version in `Cargo.toml` hochzählen (`[workspace.package]`), danach `cargo check` für die `Cargo.lock`.
2. `RELEASE_NOTES.md` schreiben. Der Text erscheint im Launcher unter den Neuigkeiten.
3. Committen, nach `main` bringen und den Tag pushen: `git tag v2.0.1 && git push origin v2.0.1`.

Der Workflow `release.yml` baut dann auf allen Systemen, testet die Pakete und veröffentlicht den Release samt `SHA256SUMS.txt`. Pushes auf andere Branches bauen nur Artefakte zum Prüfen, wenn sich die Pakete ändern. Die Website geht bei jeder Änderung in `website/` auf `main` über `pages.yml` online.

Die Dateinamen der Pakete enthalten absichtlich keine Version, sonst brechen die Links auf der Website (`releases/latest/download/...`).

Linux-Pakete lokal bauen: `packaging/linux/build.sh`, Ergebnis in `dist/`. Symbole neu erzeugen: `cargo test -p panzer --release export_icon -- --ignored` und `python3 packaging/make_icons.py`, für die Website `cargo test -p panzer --release export_items -- --ignored`.

## Spielen

- **Name:** Fürs Hosten und Beitreten muss oben im Menü ein Name stehen (1–16 Zeichen). Er wird gespeichert. Gibt es den Namen schon auf dem Server, bekommt man eine Nummer dazu.
- **Solo:** im Menü auf *Solo* klicken: du gegen Bots (Anzahl mit *Bots dazu* einstellen, mindestens 1).
- **Hosten:** *Spiel hosten* startet einen Server im Spiel (UDP-Port 5555). Deine IP wird angezeigt. Freie Plätze füllen die Bots aus *Bots dazu*. Du bist der Host und darfst kicken und bannen.
- **Beitreten:** IP des Hosts eingeben (optional mit `:port`) und *Beitreten* klicken oder Enter drücken.
- **Beitreten und Verlassen** starten keine neue Runde mehr. Wer während einer Runde beitritt, landet in der Warteschlange, schaut zu und spielt ab der nächsten Runde mit. Wer geht, dessen Panzer verschwindet, die Runde läuft weiter. Sind nur noch zwei da, geht es normal weiter, bleibt einer allein, wartet der Server auf Mitspieler.
- **Lokal:** 2 bis 4 Spieler an einer Tastatur (Rot: ESDF + Q, Grün: Pfeiltasten + M, Blau: IJKL + U, Orange: Numpad 8/4/5/6 + 0), auf Wunsch mit Bots dazu.
- **Große Maps:** Ab 5 Spielern wird das Labyrinth mit jedem Spieler größer und verkleinert angezeigt.

Direktstart: `panzer host [port] [bots]`, `panzer join 192.168.1.20[:5555]`, `panzer local [2|3|4] [bots]`, `panzer solo [bots]`.

Steuerung im Netzwerkspiel und solo: Pfeiltasten oder WASD fahren, Leertaste schießen, 1–7 Sprüche, T/Enter Chat, Tab Spielerliste, F2 Ton, F4 Musik, F3 FPS/Ping/Bandbreite, F11 Vollbild, Esc zurück ins Menü.

### Host- und Admin-Befehle (Chat)

| Befehl | Wirkung |
|---|---|
| `/kick <spieler> [grund]` | rauswerfen (Name, Namensanfang oder Nummer wie `#3`) |
| `/ban <spieler> [grund]` | rauswerfen und sperren (nach IP und Installation, steht in `bans.txt`) |
| `/unban <name\|ip\|nr>` | Sperre aufheben, `/bans` listet alle |
| `/mute`, `/unmute <spieler>` | Chat sperren/freigeben |
| `/bots <0-7>` | Anzahl Bots ändern (gilt ab der nächsten Runde) |
| `/players` | Spieler mit Ping (Admins sehen die IPs) |
| `/login <passwort>` | Admin werden (nur dedizierter Server mit `--admin-pass`) |

Kick und Bann gehen auch per Klick in der Spielerliste (Tab gedrückt halten).

## Regeln

- Kugeln prallen an Wänden ab und verschwinden nach 10 s. Jeder Panzer hat maximal 5 Kugeln gleichzeitig im Spiel (außer bei der Sonderregel Scharfschützen). Sind alle 5 unterwegs, lädt man nach: Ein Ring um den Panzer und das HUD zeigen, wie viele Sekunden es noch dauert, bis die älteste Kugel verschwindet und wieder eine frei ist. Die Punkte neben dem Spielstand zeigen die freien Kugeln.
- Eigene Kugeln können dich selbst treffen.
- Nach dem Erscheinen ist man 2 s unverwundbar (blinkender Ring), damit man nicht an der eigenen Kugel stirbt, wenn man an einer Wand erscheint und gleich schießt.
- Ist nur noch ein Panzer übrig, endet die Runde nach 3 s und der Überlebende bekommt einen Punkt. Danach gibt es ein neues, zufälliges Labyrinth.

## Power-ups

Alle 6–11 s erscheint eine Kiste mit lila Schimmer, maximal 3 gleichzeitig. Drüberfahren hebt sie auf. Man trägt immer nur eine Waffe; wer schon eine hat, lässt Waffenkisten liegen. Effekte (untere Tabelle) wirken sofort und gehen immer. Insgesamt gibt es 67 Power-ups: 46 Waffen, 16 Effekte, die Wundertüte und 4 Sachen, die es nur aus der Wundertüte gibt.

**Waffen**

| Kiste | Wirkung |
|---|---|
| Splitterbombe | Feuer schießt eine große Kugel, ein zweiter Druck zündet sie (oder sie zündet nach 5 s von selbst) und es fliegen 26 Splitter in alle Richtungen. Die Splitter treffen jeden, auch dich. |
| MG | Feuer gedrückt halten: 25 Schuss mit Streuung |
| Laser | sofortiger Strahl, der bis zu 1400 px weit von Wänden abprallt. Mit gepunkteter Zielhilfe. |
| Lenkrakete | Rakete mit Flamme und Rauchspur. Fliegt kurz geradeaus und sucht sich dann den Weg durchs Labyrinth zum nächsten Gegner. Getarnte Panzer findet sie nicht. Wenn nach 10 s der Treibstoff alle ist, explodiert sie. |
| Schrotflinte | 2 Schuss, jeder mit 7 Schrotkugeln im Fächer, die nach gut 1 s verschwinden |
| Flummi | pinker Ball, der mit jedem Abpraller schneller wird (bis fast 4×) |
| Minen | 3 Minen, die hinter den Panzer gelegt werden. Nach 1 s sind sie scharf und für die anderen fast unsichtbar. Wer drüberfährt oder draufschießt, löst sie aus. Die eigenen Minen gehen auch hoch. |
| Tauschkugel | Trifft sie einen Panzer, tauschst du mit ihm den Platz. Ein zweiter Druck teleportiert dich zur Kugel. |
| Bumerang | fliegt eine Kurve und kommt zurück. Fängst du ihn, kannst du ihn noch mal werfen. |
| Eisstrahl | friert den getroffenen Panzer 3 s ein (kein Fahren, kein Schießen). Ein Schild fängt ihn ab. |
| Schwarzes Loch | fliegt 1,2 s und öffnet sich dann für 4,5 s. Es zieht Panzer und Geschosse an und verschluckt alles in der Mitte, auch durch den Schild und auch dich. Wände schützen. |
| Luftschlag | markiert einen Punkt bis 280 px voraus (mit Zielhilfe). Ein Jet überfliegt das Ziel, 1,5 s später schlägt dort eine Bombe ein, die alles im Umkreis trifft. |
| Flammenwerfer | Feuer gedrückt halten: ein Flammenstrahl mit gut 100 px Reichweite, der an Wänden abprallt und Minen zündet. Der Tank reicht gut 2 s (Balken im HUD). Die eigenen Flammen verbrennen dich nicht. |
| Blitz | trifft sofort den nächsten Panzer vor dir (bis 250 px, mit freier Sicht) und springt von dort auf bis zu zwei weitere über. Ein Schild fängt ihn ab. Ohne Ziel schlägt er in die nächste Wand ein. |
| Railgun | Feuer gedrückt halten lädt (knapp 1 s, Ring um den Panzer), loslassen feuert einen Strahl geradeaus, der durch bis zu 2 Wände schlägt und alle Panzer in der Linie trifft. Beim Laden fährt man halb so schnell. |
| Fernlenkrakete | Der Panzer bleibt stehen, Links/Rechts lenken die Rakete (6 s Treibstoff). An einer Wand oder einem Panzer geht sie hoch, ein zweiter Druck zündet sie sofort. |
| Geschützturm | stellt einen Turm hinter den Panzer, der 9 s lang auf Gegner mit freier Sicht schießt. Seine Kugeln treffen jeden, auch dich. Ein Treffer zerstört ihn. |
| Wandbauer | 3 Wandstücke quer hinter den Panzer, die nach 10 s zerfallen. Blocken Kugeln und Wege. |
| Ölfass | 2 Ölflecken hinter den Panzer: Wer drüberfährt, rutscht (schneller, schwer zu lenken). Feuer oder eine Explosion zündet das Öl, dann brennt es 2,5 s und erwischt jeden darin. |
| Klebebombe | 2 Schuss. Bleibt an der Wand oder am Panzer kleben und geht 1,5 s später hoch. |
| Granatwerfer | 3 Granaten, die über Wände fliegen und 240 px voraus explodieren (mit Zielhilfe). |
| Raketenschwarm | 5 kleine Lenkraketen im Fächer |
| Sägeblatt | prallt bis zu 7-mal ab und sägt durch jeden Panzer auf dem Weg |
| Plasmakugel | schmilzt sich durch bis zu 3 Wände (die Wände bleiben weg) und trifft jeden auf dem Weg |
| Feuerwerk | 2 Raketen, die nach 0,7 s oder an einer Wand in 16 bunte Funken zerplatzen |
| Druckwelle | 2 Stöße: schiebt Panzer im Kegel vor dir weg und lenkt Geschosse um, die dann dir gehören |
| Spinnenmine | 2 Spinnen hinter den Panzer. Sobald ein Gegner in die Nähe kommt, krabbeln sie zu ihm und explodieren. |
| Rammbock | 4 s lang schneller, wer gerammt wird, ist erledigt |
| Kugelring | 12 Kugeln auf einmal, rundherum |
| Suchkugel | 3 Schuss, die zum nächsten sichtbaren Panzer lenken, nach 1 s auch zu dir |
| Phasenkugel | 2 Schuss geradeaus durch alle Wände |
| Kanonenkugel | dicke Kugel, die durch Schilde und Panzer geht und 2-mal abprallt |
| Artillerie | 3 Einschläge hintereinander vor dir, Wände egal |
| Bärenfalle | 2 Fallen hinter den Panzer. Wer reinfährt (auch du), hängt 4 s fest. |
| Sprungdüse | 3 Sprünge ein Stück nach vorne, auch über Wände |
| Frostnova | friert alle Panzer im Umkreis von 130 px 3 s ein |
| Orbitallaser | markiert einen Punkt 240 px voraus, 1,6 s später wandert dort ein Strahl von oben weiter und trifft alles, auch durch Schilde |
| Prisma | Laserstrahl, der sich an der ersten Wand in drei Strahlen aufspaltet |
| Querschläger | Kugel, die sich bei den ersten 3 Abprallern jeweils verdoppelt: bis zu 8 Kugeln, die auch dich treffen |
| Schlangenkugel | 3 Schuss, die in Wellenlinien fliegen und schwer vorherzusehen sind |
| Zeitbombe | 2 Bomben hinter den Panzer. Sie ticken 3,5 s lang (mit Countdown und Reichweiten-Ring) und reißen alles im Umkreis mit, auch dich. |
| Napalm | 2 Schuss. Platzt an Wand, Panzer oder nach gut 1 s und hinterlässt eine Feuerfläche, die 4,5 s brennt und jeden darin erwischt, auch dich. Wände schützen. |
| Laserschranke | 2 Schranken quer hinter den Panzer, von Wand zu Wand. Nach 0,8 s tödlich für jeden, auch für dich, und 6 s lang aktiv. |
| Kugelsprinkler | stellt ein Gerät hinter den Panzer, das sich dreht und gut 3 s lang Kugeln in zwei Richtungen spuckt |
| Enterhaken | 3 Schuss. In einer Wand zieht er dich hin, an einem Panzer zieht er den Gegner zu dir und friert ihn 1,4 s ein. |
| Selbstzerstörung | Nach 1,2 s fliegst du in die Luft und reißt alles im Umkreis von 120 px mit. Nur ein Schild oder der Superstern rettet dich. |

**Effekte**

| Kiste | Wirkung |
|---|---|
| Schild | fängt 8 s lang einen Treffer ab und blinkt kurz vor Ablauf |
| Turbo | 6 s lang schneller fahren und lenken |
| Schrumpfpilz | 10 s lang nur noch halb so groß (schwerer zu treffen, etwas schneller) |
| Geist | 5 s lang durch Wände fahren. Solange du in einer Wand steckst, kannst du nicht schießen. |
| Tarnkappe | 8 s lang für die anderen unsichtbar. Lenkraketen finden dich nicht. |
| Verwirrung | Alle **anderen** haben 6 s lang vertauschte Steuerung (vorne/hinten, links/rechts). |
| Spiegelschild | 7 s lang prallen Kugeln an dir ab und fliegen zum Schützen zurück |
| Superstern | 6 s lang unverwundbar (außer Schwarzes Loch und Atombombe) und schneller, wen du anfährst, ist erledigt |
| Riesenpilz | 9 s lang anderthalbmal so groß und etwas langsamer, kleinere Panzer werden zertrampelt |
| Schnellfeuer | 8 s lang doppelt so viele Kugeln und doppelt so schnell nachschießen |
| Ersatzteil | rettet einmal vor einem Treffer, sogar vor der Atombombe, und hält die ganze Runde |
| Lähmung | Alle **anderen** fahren 5 s lang nur halb so schnell. |
| Doppellauf | 10 s lang zwei Kugeln pro Schuss |
| Teleporter | bringt dich sofort an die Stelle, die am weitesten von allen anderen weg ist |
| Discokugel | Alle Panzer tauschen reihum die Plätze. |
| Radar | 12 s lang siehst du getarnte Panzer, Minen, Fallen und Spinnen |
| Wundertüte (goldene Kiste) | zufälliges Power-up (mit Glücksrad über dem Panzer), ersetzt auch eine Waffe, die du schon hast. Mit 25 % Chance ist es eine **Heiße Kartoffel**: Sie explodiert nach 10 s. Fährst du einen anderen Panzer an, hat er sie (mit mindestens 3 s Restzeit). Mit 15 % Chance ist es eine **Atombombe**: Sie rollt aus und geht nach 4 s (Countdown, Sirene, Warnkreis) mit 230 px Radius hoch, durch Wände und Schilde, reißt Minen und Bomben mit und sprengt die Wände in der Nähe weg. Die Außenwände bleiben. Mit je 8 % Chance gibt es einen **Meteoritenschauer** (9 Einschläge irgendwo im Labyrinth, auch bei dir) oder die **Goldene Kugel** (schnell, geht durch Schilde und durch jeden Panzer auf dem Weg). |

## Sonderregeln

Ungefähr jede dritte Runde bekommt eine Sonderregel, die am Anfang groß angezeigt wird und oben in der Mitte stehen bleibt. Im Menü lässt sich das unter **Regel** ändern: *Zufall* (Standard), *Keine* oder eine feste Regel, zum Beispiel nur Kistenregen. Die Auswahl wird gespeichert und gilt für Solo, Lokal und beim Hosten. Beim Beitreten gilt, was der Host eingestellt hat.

| Regel | Was passiert |
|---|---|
| Kistenregen | alle 1–2 s eine Kiste, bis zu 8 gleichzeitig |
| Hyperkugeln | normale Kugeln fast doppelt so schnell |
| Riesenkugeln | dicke, etwas langsamere Kugeln |
| Alle auf Turbo | alle Panzer fahren schneller |
| Nebel | Man sieht nur die Umgebung des eigenen Panzers, von den Wänden nur Umrisse. |
| Scharfschützen | nur 1 Kugel gleichzeitig, aber pfeilschnell und nach 3,5 s weg |
| Stromausfall | Alles ist dunkel. Licht gibt es nur von den Scheinwerfern der Panzer, von Kugeln, Feuer, Explosionen und Lasern. |

## Extras

- **Arenen:** Jede Runde hat einen anderen Look (Nachtbasis, Wüste, Vulkan mit glühenden Rissen, Gletscher, Dschungel, Neon-City) mit eigener Umgebung (Staub, Sand, Glut, Schnee, Glühwürmchen, Neon-Pixel). Wände haben Kanten und Schatten.
- **Licht und Partikel:** Kugeln leuchten in der Farbe des Schützen und ziehen einen Schweif, Mündungsfeuer, Funken beim Abprallen, Explosionen mit Feuerball, Druckwelle, Rauch und Glut, Brandflecken auf dem Boden. Zerstörte Panzer zerfallen in Trümmer, der Turm fliegt davon und das Wrack brennt.
- **Zeitlupe:** Ist die Runde entschieden, läuft alles außer den Panzern kurz in Zeitlupe, mit Kinobalken und Zoom auf den letzten Abschuss. Auch eine Atombombe bremst die Zeit.
- **Ansagen:** Erstes Blut, Doppel-/Dreifach-/Megakill, Trickshot (ab 3 Abprallern), Eigentor. Wer mehrere Runden in Folge gewinnt, bekommt das beim Rundensieg angezeigt (Hattrick, Nicht zu stoppen, …), dazu Konfetti.
- **Killfeed** oben rechts mit Waffensymbol und farbigen Namen: wer wen womit erwischt hat, inklusive Eigentore
- **Sprüche** im Netzwerkspiel und solo mit den Tasten 1–7 („Hehe!“, „Ups …“, „GG“, „Zu langsam!“, „Komm her!“, „JUlIAN SPRING“, „SCHEIß JUNKY“) als Sprechblase über dem Panzer. Bots sagen auch mal was.
- **Chat** im Netzwerkspiel: T oder Enter öffnet ihn, Enter schickt ab, Esc bricht ab. Tab zeigt die Spielerliste mit Ping.
- **Sound und Musik**: alle Effekte und ein Synthwave-Loop werden beim Start synthetisiert, links/rechts je nach Position. F2 schaltet den Ton aus und an, F4 nur die Musik.
- **Menü** mit Bots, die im Hintergrund gegeneinander kämpfen
- **Krone** über dem Panzer und im HUD für den, der alleine vorne liegt
- **Kettenspuren** auf dem Boden, Wackeln bei Explosionen

## Netzwerk

- **Dedizierter Server:** `./server_start.sh` bzw. `server_start.bat`, oder direkt `panzer-server [--port 5555] [--bots N] [--admin-pass PW] [--bans bans.txt]`. Läuft ohne Grafik, z. B. auf dem Homeserver. Auf der Konsole gehen dieselben Befehle ohne `/` (`kick`, `ban`, `bots 3`, `say hallo` …). Dann spielt niemand mit Heimvorteil, der Host-Rechner ist nicht mehr gleichzeitig Spieler und Schiedsrichter.
- **Docker:** `docker build -t panzer-server .` baut ein Image ohne Betriebssystem (`FROM scratch`), das nur die statische Binärdatei enthält. Gestartet wird es mit `docker run -dit --name panzer -p 5555:5555/udp -e PANZER_BOTS=3 -e PANZER_ADMIN_PASS=geheim -v panzer-data:/data panzer-server`. Die Bannliste liegt im Volume unter `/data`. Ein Bind-Mount muss für UID 65534 beschreibbar sein. Zur Konsole geht es mit `docker attach panzer`, zurück mit Strg+P, dann Strg+Q (Strg+C beendet den Server hier nicht). Beenden mit `docker stop panzer`. Wer `PANZER_PORT` ändert, muss auch die Portfreigabe `-p` anpassen.
- **Umgebungsvariablen:** Alle Optionen des dedizierten Servers gehen auch ohne Docker per Umgebung. Die Kommandozeile hat Vorrang, leere Werte zählen als nicht gesetzt.

  | Variable | Option | Standard |
  |---|---|---|
  | `PANZER_PORT` | `--port` | `5555` |
  | `PANZER_BOTS` | `--bots` | `0` (höchstens 7) |
  | `PANZER_ADMIN_PASS` | `--admin-pass` | keins |
  | `PANZER_BANS` | `--bans` | `bans.txt`, im Docker-Image `/data/bans.txt` |
  | `PANZER_QUIET` | `--quiet` | `0` (`1` schaltet das Log ab) |
- Der Server simuliert mit festen 60 Ticks/s und schickt jedem Spieler 60 Snapshots/s über **UDP**. Snapshots sind binär und als **Delta** gegen den letzten bestätigten Snapshot kodiert: typisch 30–100 Byte statt mehrerer KB JSON wie früher. Verlorene Pakete halten nichts auf (kein TCP-Stau mehr).
- Der Client schickt pro Frame seine Eingabe und alle noch unbestätigten mit (bis zu 24), damit verlorene Pakete nichts ausmachen. Chat, Labyrinth und Spielerliste gehen als zuverlässige Nachrichten.
- Der Client sagt seinen eigenen Panzer voraus (Prediction) und zeigt die anderen 75 ms verzögert und interpoliert.
- **Im selben WLAN/LAN** geht das direkt. Eventuell den **UDP**-Port 5555 in der Firewall des Hosts freigeben.
- **Übers Internet:** am einfachsten alle in Tailscale (oder ZeroTier), dann die Tailscale-IP des Hosts verwenden (`tailscale ip -4`). Alternativ Port-Forwarding von **UDP** 5555.

## Anti-Cheat

Der Server ist autoritativ: Clients schicken nur Tasten, alles andere rechnet der Server.

- **Speedhack:** Jeder Spieler darf pro Tick höchstens so viel Spielzeit verbrauchen, wie real vergangen ist (+5 % zum Aufholen von Uhren-Abweichungen). Wer mehr schickt, dessen Eingaben verfallen, und es gibt eine Verwarnung. (Die Python-Version hat bei mehr als 6 wartenden Eingaben alle auf einmal ausgeführt, damit konnte ein manipulierter Client beliebig schnell fahren und schießen.)
- **Wallhack:** Der Server schickt nichts, was man nicht sehen darf: getarnte Panzer ohne Radar und im Nebel alles außerhalb der Sicht. Ein manipulierter Client kann es also auch nicht anzeigen.
- **Paketflut, kaputte Pakete, Dauerfeuer-Makros, Chat-Spam** werden erkannt. Nach 3 Verwarnungen fliegt man raus. Alles steht im Server-Log (Konsole).
- Grenzen: Ein Bot-Client, der für einen Spieler zielt und abdrückt, sieht für den Server aus wie ein sehr guter Spieler. Und wer den Server selbst betreibt, kann ihn natürlich verändern, deshalb im Zweifel den dedizierten Server nehmen.

## Einstellungen

Name, letzte IP, Bots, Ton und eine zufällige Installations-Kennung (für Bans) stehen in `~/.config/panzer/config.txt` (Windows: `%APPDATA%\panzer\config.txt`).

## Lizenz

MIT, siehe [LICENSE](LICENSE). Die Schriften in `assets/fonts` stehen unter der Apache License 2.0.
