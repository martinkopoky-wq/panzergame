# Panzer

Nachbau von AZ (Tank Trouble) als Desktop-Programm in Python + pygame-ce, mit Multiplayer für bis zu 8 Spieler über IP.

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

- **Hosten:** im Menü auf *Spiel hosten* klicken. Deine IP und der Port (5555) werden angezeigt. Der Host spielt Rot. Sobald einer beitritt, geht es los. Bei jedem weiteren Beitritt startet sofort eine neue Runde mit allen (Punkte bleiben). Solange ein Platz frei ist, steht oben rechts die IP.
- **Beitreten:** die IP des Hosts eingeben (optional mit `:port`) und auf *Beitreten* klicken oder Enter drücken. Die Farben werden der Reihe nach vergeben: Grün, Blau, Orange, Lila, Türkis, Pink, Braun. Wer als Neunter kommt, wird abgelehnt.
- **Lokal:** 2 bis 4 Spieler an einer Tastatur (Rot: ESDF + Q, Grün: Pfeiltasten + M, Blau: IJKL + U, Orange: Numpad 8/4/5/6 + 0).
- **Große Maps:** Bis 4 Spieler ist das Labyrinth so groß wie immer (6–10 × 4–6 Zellen). Ab 5 Spielern wird es mit jedem Spieler deutlich größer, bei 8 Spielern etwa 20 × 11 Zellen. Damit es trotzdem ins Fenster passt, wird es verkleinert angezeigt. Es gibt dann auch mehr Kisten gleichzeitig. Mit F11 (Vollbild) sieht man mehr Details.

Direktstart: `python panzer.py host [port]`, `python panzer.py join 192.168.1.20[:5555]` oder `python panzer.py local [2|3|4]`.

Steuerung im Netzwerkspiel: Pfeiltasten oder WASD zum Fahren, Leertaste zum Schießen, 1–5 für Sprüche, F2 Ton an/aus, F3 zeigt FPS und Ping, F11 für Vollbild, Esc zurück ins Menü.

## Regeln

- Kugeln prallen an Wänden ab und verschwinden nach 10 s. Jeder Panzer hat maximal 5 Kugeln gleichzeitig im Spiel (außer bei der Sonderregel Scharfschützen). Sind alle 5 unterwegs, lädt man nach: Ein Ring um den Panzer und das HUD zeigen, wie viele Sekunden es noch dauert, bis die älteste Kugel verschwindet und wieder eine frei ist. Die Punkte neben dem Spielstand zeigen die freien Kugeln.
- Eigene Kugeln können dich selbst treffen.
- Ist nur noch ein Panzer übrig, endet die Runde nach 3 s und der Überlebende bekommt einen Punkt. Danach gibt es ein neues, zufälliges Labyrinth.

## Power-ups

Alle 6–11 s erscheint eine Kiste mit lila Schimmer, maximal 3 gleichzeitig. Drüberfahren hebt sie auf. Man trägt immer nur eine Waffe; wer schon eine hat, lässt Waffenkisten liegen. Effekte (untere Tabelle) wirken sofort und gehen immer.

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
| Luftschlag | markiert einen Punkt bis 280 px voraus (mit Zielhilfe). 1,5 s später schlägt dort eine Bombe ein, die alles im Umkreis trifft. |

**Effekte**

| Kiste | Wirkung |
|---|---|
| Schild | fängt 8 s lang einen Treffer ab und blinkt kurz vor Ablauf |
| Turbo | 6 s lang schneller fahren und lenken |
| Schrumpfpilz | 10 s lang nur noch halb so groß (schwerer zu treffen, etwas schneller) |
| Geist | 5 s lang durch Wände fahren. Solange du in einer Wand steckst, kannst du nicht schießen. |
| Tarnkappe | 8 s lang für die anderen unsichtbar. Lenkraketen finden dich nicht. |
| Verwirrung | Alle **anderen** haben 6 s lang vertauschte Steuerung (vorne/hinten, links/rechts). |
| Wundertüte (goldene Kiste) | zufälliges Power-up, ersetzt auch eine Waffe, die du schon hast. Mit 25 % Chance ist es eine **Heiße Kartoffel**: Sie explodiert nach 10 s. Fährst du einen anderen Panzer an, hat er sie (mit mindestens 3 s Restzeit). |

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

## Extras

- **Killfeed** oben rechts: wer wen womit erwischt hat, inklusive Eigentore
- **Sprüche** im Netzwerkspiel mit den Tasten 1–5 („Hehe!“, „Ups …“, „GG“, „Zu langsam!“, „Komm her!“) als Sprechblase über dem Panzer
- **Sound**: alle Effekte werden beim Start synthetisiert, es gibt keine Sounddateien. F2 schaltet den Ton aus und an.
- **Krone** über dem Panzer und im HUD für den, der alleine vorne liegt
- **Kettenspuren** auf dem Boden, qualmende Wracks und Wackeln bei Explosionen

## Netzwerk

- Der Host simuliert das Spiel mit festen 60 Ticks/s. Jeder Client bekommt beim Verbinden ein `hello` mit Spieler-Nummer und Protokollversion. Unterschiedliche Spielversionen werden mit einer Meldung abgelehnt. Beide Seiten schicken nur 30 Pakete/s (TCP, JSON-Zeilen). Der Client bündelt dafür seine Eingaben.
- Der Client sagt seinen eigenen Panzer sofort voraus (Prediction) und gleicht ihn mit der Host-Bestätigung (`ack`) ab. Den Gegner und die Kugeln zeigt er 75 ms verzögert und interpoliert, damit unregelmäßig ankommende Pakete nicht ruckeln.
- **Im selben WLAN/LAN** geht das direkt. Eventuell musst du den TCP-Port 5555 in der Firewall des Hosts freigeben (Windows fragt beim ersten Start nach).
- **Übers Internet:** am einfachsten beide in Tailscale (oder ZeroTier), dann die Tailscale-IP des Hosts verwenden (`tailscale ip -4`). Alternativ Port-Forwarding von TCP 5555 am Router des Hosts.

## Als .exe für Windows

```bash
pip install pyinstaller
pyinstaller --onefile --noconsole --name Panzer panzer.py   # Ergebnis: dist/Panzer.exe
```
