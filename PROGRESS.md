# PROGRESS

## 2026-10-06: Erste Version
**Gemacht:** `panzer.py` (eine Datei, pygame-ce) mit zufälligem Labyrinth (DFS plus 40 % der Wände entfernt, 6–10 × 4–6 Zellen), Panzer mit Kreis-Kollision, abprallenden Kugeln (achsweise Reflexion, Substeps), Punkten und Rundenende. Netzwerk: Host ist autoritativ, Client per TCP/JSON. Dazu ein Menü mit Host/Join/Lokal, die zuletzt verwendete IP wird gemerkt. Getestet: 10 Minuten Fuzz-Simulation (Panzer und Kugeln stecken nie in Wänden), Loopback-Netzwerktest (Input, Maze, Snapshot, zweiter Client wird abgelehnt, Reconnect) und gerenderte Screenshots.

**Offen:** Power-ups (wie im Original), Sound, Client-Interpolation bei hoher Latenz, mehr als 2 Spieler.

**Gelernt:** `sock.close()` weckt unter Linux einen blockierten `recv()` in einem anderen Thread nicht auf, deshalb merkte der Peer den Disconnect nicht. Vorher `shutdown(SHUT_RDWR)` aufrufen (`close_sock`). Schüsse laufen über einen Zähler statt über ein „gedrückt“-Bit, damit kurze Taps beim Host nicht verloren gehen.

**Als Nächstes:** mit dem Freund testen (LAN/Tailscale), dann eventuell Power-ups.
