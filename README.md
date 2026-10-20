# Panzer

Nachbau von AZ (Tank Trouble) als Desktop-Programm in Python + pygame-ce, mit Multiplayer für bis zu 8 Spieler über IP, Computergegnern (Bots) und jeder Menge Krawall: Licht, Partikel, Zeitlupe, Ansagen und synthetisierter Musik. Alles steckt in `panzer.py`, es gibt keine Bild- oder Sounddateien.

## Setup

### Linux / macOS

```bash
python -m venv .venv
.venv/bin/pip install -r requirements.txt
.venv/bin/python panzer.py
```

### Windows

Voraussetzung: Python 3 von [python.org](https://www.python.org/downloads/), bei der Installation „Add python.exe to PATH“ anhaken.

1. PowerShell im Projektordner öffnen (im Explorer in die Adresszeile `powershell` tippen und Enter drücken).
2. Diese drei Befehle der Reihe nach ausführen:

   ```powershell
   py -m venv .venv
   .venv\Scripts\pip install -r requirements.txt
   .venv\Scripts\python panzer.py
   ```

   Der erste legt die virtuelle Umgebung an, der zweite installiert pygame-ce, der dritte startet das Spiel. Die ersten beiden braucht es nur einmal. Danach reicht zum Spielen `.venv\Scripts\python panzer.py`.
3. Wenn du zum ersten Mal hostest, fragt die Windows-Firewall nach dem Netzwerkzugriff. Bestätigen, sonst kann niemand beitreten.

Probleme:

- **`py` wird nicht gefunden:** Nimm `python -m venv .venv`. Öffnet sich stattdessen der Microsoft Store, ist Python nicht installiert (oder nicht im PATH).
- **`.venv/bin/pip` wird nicht gefunden:** Das ist der Linux-Pfad. Unter Windows heißt er `.venv\Scripts\pip`.
- **Fehler mit einem `.venv`-Ordner, der von Linux kopiert wurde:** Den Ordner löschen und mit `py -m venv .venv` neu anlegen. Eine venv läuft nur auf dem System, auf dem sie erstellt wurde.
- **Aktivieren mit `.venv\Scripts\Activate.ps1` wird blockiert:** Einmal `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned` ausführen. Mit den vollen Pfaden wie oben brauchst du das Aktivieren aber gar nicht.

## Spielen

- **Solo:** im Menü auf *Solo* klicken: du gegen Bots (Anzahl mit *Bots dazu* einstellen, mindestens 1). Gesteuert wird wie im Netzwerkspiel.
- **Bots:** Die Zahl bei *Bots dazu* gilt fürs Hosten und fürs lokale Spiel. Bots fahren mit Wegsuche durchs Labyrinth, suchen Bandenschüsse, weichen Kugeln aus, sammeln Kisten, fliehen vor Atombomben und Luftschlägen und haben jeweils etwas andere Reflexe. Beim Hosten nehmen sie die freien Plätze von oben (Braun, Pink, …). Tritt ein Mensch bei und alles ist voll, macht ein Bot Platz.
- **Hosten:** im Menü auf *Spiel hosten* klicken. Deine IP und der Port (5555) werden angezeigt. Der Host spielt Rot. Sobald einer beitritt (oder mit Bots sofort), geht es los. Bei jedem weiteren Beitritt startet sofort eine neue Runde mit allen (Punkte bleiben). Solange ein Platz frei ist, steht oben rechts die IP.
- **Beitreten:** die IP des Hosts eingeben (optional mit `:port`) und auf *Beitreten* klicken oder Enter drücken. Die Farben werden der Reihe nach vergeben: Grün, Blau, Orange, Lila, Türkis, Pink, Braun. Wer als Neunter kommt, wird abgelehnt.
- **Lokal:** 2 bis 4 Spieler an einer Tastatur (Rot: ESDF + Q, Grün: Pfeiltasten + M, Blau: IJKL + U, Orange: Numpad 8/4/5/6 + 0), auf Wunsch mit Bots dazu.
- **Große Maps:** Bis 4 Spieler ist das Labyrinth so groß wie immer (6–10 × 4–6 Zellen). Ab 5 Spielern wird es mit jedem Spieler deutlich größer, bei 8 Spielern etwa 20 × 11 Zellen. Damit es trotzdem ins Fenster passt, wird es verkleinert angezeigt. Es gibt dann auch mehr Kisten gleichzeitig. Mit F11 (Vollbild) sieht man mehr Details.

Direktstart: `python panzer.py host [port] [bots]`, `python panzer.py join 192.168.1.20[:5555]`, `python panzer.py local [2|3|4] [bots]` oder `python panzer.py solo [bots]`.

Steuerung im Netzwerkspiel und solo: Pfeiltasten oder WASD zum Fahren, Leertaste zum Schießen, 1–5 für Sprüche, F2 Ton an/aus, F4 Musik an/aus, F3 zeigt FPS und Ping, F11 für Vollbild, Esc zurück ins Menü.

## Regeln

- Kugeln prallen an Wänden ab und verschwinden nach 10 s. Jeder Panzer hat maximal 5 Kugeln gleichzeitig im Spiel (außer bei der Sonderregel Scharfschützen). Sind alle 5 unterwegs, lädt man nach: Ein Ring um den Panzer und das HUD zeigen, wie viele Sekunden es noch dauert, bis die älteste Kugel verschwindet und wieder eine frei ist. Die Punkte neben dem Spielstand zeigen die freien Kugeln.
- Eigene Kugeln können dich selbst treffen.
- Ist nur noch ein Panzer übrig, endet die Runde nach 3 s und der Überlebende bekommt einen Punkt. Danach gibt es ein neues, zufälliges Labyrinth.

## Power-ups

Alle 6–11 s erscheint eine Kiste mit lila Schimmer, maximal 3 gleichzeitig. Drüberfahren hebt sie auf. Man trägt immer nur eine Waffe; wer schon eine hat, lässt Waffenkisten liegen. Effekte (untere Tabelle) wirken sofort und gehen immer. Insgesamt gibt es 59 Power-ups: 38 Waffen, 16 Effekte, die Wundertüte und 4 Sachen, die es nur aus der Wundertüte gibt.

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

Ungefähr jede dritte Runde bekommt eine Sonderregel, die am Anfang groß angezeigt wird und oben in der Mitte stehen bleibt:

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
- **Sprüche** im Netzwerkspiel und solo mit den Tasten 1–5 („Hehe!“, „Ups …“, „GG“, „Zu langsam!“, „Komm her!“) als Sprechblase über dem Panzer. Bots sagen auch mal was.
- **Sound und Musik**: alle Effekte und ein Synthwave-Loop werden beim Start synthetisiert, in Stereo je nach Position. F2 schaltet den Ton aus und an, F4 nur die Musik.
- **Menü** mit Bots, die im Hintergrund gegeneinander kämpfen
- **Krone** über dem Panzer und im HUD für den, der alleine vorne liegt
- **Kettenspuren** auf dem Boden, Wackeln bei Explosionen

## Netzwerk

- Der Host simuliert das Spiel mit festen 60 Ticks/s. Jeder Client bekommt beim Verbinden ein `hello` mit Spieler-Nummer und Protokollversion. Unterschiedliche Spielversionen werden mit einer Meldung abgelehnt. Beide Seiten schicken nur 30 Pakete/s (TCP, JSON-Zeilen). Der Client bündelt dafür seine Eingaben.
- Der Client sagt seinen eigenen Panzer sofort voraus (Prediction) und gleicht ihn mit der Host-Bestätigung (`ack`) ab. Den Gegner und die Kugeln zeigt er 75 ms verzögert und interpoliert, damit unregelmäßig ankommende Pakete nicht ruckeln.
- Bots laufen nur beim Host, für die Clients sind sie normale Panzer. Alle Effekte (Licht, Partikel, Trümmer) berechnet jeder Client selbst aus den Snapshots, übers Netz gehen nur die Ereignisse.
- **Im selben WLAN/LAN** geht das direkt. Eventuell musst du den TCP-Port 5555 in der Firewall des Hosts freigeben (Windows fragt beim ersten Start nach).
- **Übers Internet:** am einfachsten beide in Tailscale (oder ZeroTier), dann die Tailscale-IP des Hosts verwenden (`tailscale ip -4`). Alternativ Port-Forwarding von TCP 5555 am Router des Hosts.

## Als .exe für Windows

```bash
pip install pyinstaller
pyinstaller --onefile --noconsole --name Panzer panzer.py   # Ergebnis: dist/Panzer.exe
```
