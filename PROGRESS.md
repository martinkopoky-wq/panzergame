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
