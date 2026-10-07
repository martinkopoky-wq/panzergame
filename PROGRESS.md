# PROGRESS

## 2026-10-06: Erste Version
**Gemacht:** `panzer.py` (eine Datei, pygame-ce) mit zufälligem Labyrinth (DFS plus 40 % der Wände entfernt, 6–10 × 4–6 Zellen), Panzer mit Kreis-Kollision, abprallenden Kugeln (achsweise Reflexion, Substeps), Punkten und Rundenende. Netzwerk: Host ist autoritativ, Client per TCP/JSON. Dazu ein Menü mit Host/Join/Lokal, die zuletzt verwendete IP wird gemerkt. Getestet: 10 Minuten Fuzz-Simulation (Panzer und Kugeln stecken nie in Wänden), Loopback-Netzwerktest (Input, Maze, Snapshot, zweiter Client wird abgelehnt, Reconnect) und gerenderte Screenshots.

**Offen:** Power-ups (wie im Original), Sound, Client-Interpolation bei hoher Latenz, mehr als 2 Spieler.

**Gelernt:** `sock.close()` weckt unter Linux einen blockierten `recv()` in einem anderen Thread nicht auf, deshalb merkte der Peer den Disconnect nicht. Vorher `shutdown(SHUT_RDWR)` aufrufen (`close_sock`). Schüsse laufen über einen Zähler statt über ein „gedrückt“-Bit, damit kurze Taps beim Host nicht verloren gehen.

**Als Nächstes:** mit dem Freund testen (LAN/Tailscale), dann eventuell Power-ups.

## 2026-10-06: Power-ups + Netcode
**Gemacht:** 5 Power-ups (Splitterbombe mit Zündung beim 2. Druck, MG, abprallender Laser per Raycast, Lenkrakete mit BFS-Pfad durchs Labyrinth, Schild). Netcode umgebaut: feste 60 Ticks/s beim Host, 30 Pakete/s statt 60 in beide Richtungen, Client-Prediction des eigenen Panzers (der Host wendet jedes Client-Kommando mit dessen dt an, daher ist die Vorhersage exakt), Interpolation für den Rest, F3 zeigt FPS/Ping. Performance: Broadphase-Gitter für Wände (Klasse `Arena`), gecachte Rotations-Sprites (3°-Schritte) und Texte, opake Maze-Surface. Getestet: 15 Minuten Fuzz mit allen Waffen, Broadphase gegen volle Wandliste, Prediction == Hostposition über Loopback, Screenshots.

**Offen:** echter Test über zwei PCs, Sound.

**Gelernt:** Die Lags kamen nicht vom Rendern (gemessen: ~1 ms/Frame, ~460 FPS uncapped unter Hyprland/XWayland). Ursache war, dass der Client jeden Snapshot direkt angezeigt hat: Der eigene Panzer reagierte erst nach dem vollen Round-Trip, und Paket-Jitter wurde direkt sichtbar. `SDL_VIDEODRIVER=""` (leer gesetzt) führt zu "windows not available", die Variable dann gar nicht setzen.

**Als Nächstes:** mit Marcel übers Netz testen und mit F3 den Ping anschauen.

## 2026-10-07: 3 Spieler, Nachlade-Anzeige, echte Rakete
**Gemacht:** Spieler haben jetzt feste Slots (0 Rot = Host, 1 Grün, 2 Blau). `Game(players)` mit `set_players()`, Tanks tragen `slot`, Schüsse `owner = slot`. Der Host nimmt bis zu 2 Clients an (`Peer` mit eigener Input-Queue, Budget und `ack`). Bei Join/Leave startet sofort eine neue Runde, die Punkte bleiben, neue Spieler starten bei 0. Handshake `hello` mit `slot` und `PROTO`. Lokalmodus für 2 oder 3 Spieler. Snapshot-Tank: `[x, y, a, alive, dead_age, weapon, shield, slot, ammo, reload]`. Nachlade-Anzeige: Ring um den Panzer plus HUD (freie Kugeln als Punkte, „Nachladen x.x s“). Die Rakete ist ein supersampled Sprite (Spitze und Ring in Spielerfarbe, Flossen), dazu flackernde Flamme, clientseitige Rauchspur und eine kleine Explosion, wenn der Treibstoff alle ist. Getestet: 10 min Fuzz mit 3 Spielern und allen Waffen, Loopback mit Host-Prozess und 3 Clients (der dritte wird abgelehnt, Leave/Rejoin, `ack` pro Client), `run_client` gerendert, Screenshots.

**Offen:** echter Test zu dritt übers Netz, Sound.

**Gelernt:** pygame-ce 2.5 hat kein `draw.aapolygon`. Glatte Formen deshalb 4× größer zeichnen, `smoothscale`n und rotiert cachen.

## 2026-10-07: 14 neue Power-ups, Sonderregeln, Sound und Extras
**Gemacht:** Abgleich mit `~/Panzergame`: gleicher Commit (`da1cf0b`), die 3 Spieler waren schon da, es gab nichts zu kopieren. Neue Waffen: Schrotflinte, Flummi, Minen, Tauschkugel, Bumerang, Eisstrahl, Schwarzes Loch, Luftschlag. Neue Effekte: Turbo, Schrumpfpilz, Geist, Tarnkappe, Verwirrung, Wundertüte mit Heißer Kartoffel. Tanks haben `eff` (Effekt -> Restzeit), `move_tank` berücksichtigt das auf Host und Client gleich, deshalb bleibt die Prediction exakt. Sonderregeln pro Runde (Kistenregen, Hyperkugeln, Riesenkugeln, Turbo, Nebel, Scharfschützen) stehen in `maze["mut"]`, so kennt sie auch der Client. Killfeed mit Texten je nach Waffe, Sprüche über die Tasten 1–5 (Bits 5–7 der Tasten), synthetisierte Sounds (`Sfx`, ohne Dateien, F2), Krone, Kettenspuren, Wrackqualm, Wackeln bei Explosionen. PROTO 3. Snapshot-Tank: `[.., reload, eff, taunt]`, Fx mit id: `[id, kind, age, life, data]`, Mine/Loch/Luftschlag in `p` mit `[.., age, owner]`, dazu `kf` für den Killfeed. Getestet: 45 min Fuzz mit 3 Spielern und allen Items, Tests für jede Mechanik, Prediction == Host mit allen Bewegungs-Effekten, Loopback mit Host-Prozess (3. Client abgelehnt, Leave), `run_local` und `run_client` gerendert, Screenshots.

**Offen:** echter Test zu dritt übers Netz, Balancing (Schwarzes Loch, Kartoffel-Chance, Mutator-Chance 35 %).

**Gelernt:** Der Renderer hat die Labyrinth-Fläche über `arena.id` gecacht. Ein neues `Game` fängt aber wieder bei id 1 an, deshalb sah man nach Menü -> neues Spiel das alte Labyrinth. Jetzt wird über das Arena-Objekt gecacht. Ein Geist in der Wand hat Kugeln in der Wand erzeugt, deshalb darf er dort nicht schießen.
