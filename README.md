# Panzer

Nachbau von AZ (Tank Trouble) als Desktop-Programm in Python + pygame-ce, mit 2-Spieler-Multiplayer über IP.

## Setup

```bash
python -m venv .venv
.venv/bin/pip install -r requirements.txt      # Windows: .venv\Scripts\pip install -r requirements.txt
.venv/bin/python panzer.py
```

## Spielen

- **Hosten:** im Menü auf *Spiel hosten* klicken. Deine IP und der Port (5555) werden angezeigt. Der Host spielt Rot.
- **Beitreten:** die IP des Hosts eingeben (optional mit `:port`) und auf *Beitreten* klicken oder Enter drücken. Wer beitritt, spielt Grün.
- **Lokal:** zwei Spieler an einer Tastatur (Rot: ESDF + Q, Grün: Pfeiltasten + M).

Direktstart: `python panzer.py host [port]`, `python panzer.py join 192.168.1.20[:5555]` oder `python panzer.py local`.

Steuerung im Netzwerkspiel: Pfeiltasten oder WASD zum Fahren, Leertaste zum Schießen, F11 für Vollbild, Esc zurück ins Menü.

## Regeln

- Kugeln prallen an Wänden ab und verschwinden nach 10 s. Jeder Panzer hat maximal 5 Kugeln gleichzeitig im Spiel.
- Eigene Kugeln können dich selbst treffen.
- Ist nur noch ein Panzer übrig, endet die Runde nach 3 s und der Überlebende bekommt einen Punkt. Danach gibt es ein neues, zufälliges Labyrinth.

## Netzwerk

- Der Host simuliert das Spiel. Der Client schickt nur seine Tasten und bekommt 60× pro Sekunde den kompletten Zustand zurück (TCP, JSON-Zeilen).
- **Im selben WLAN/LAN** geht das direkt. Eventuell musst du den TCP-Port 5555 in der Firewall des Hosts freigeben (Windows fragt beim ersten Start nach).
- **Übers Internet:** am einfachsten beide in Tailscale (oder ZeroTier), dann die Tailscale-IP des Hosts verwenden (`tailscale ip -4`). Alternativ Port-Forwarding von TCP 5555 am Router des Hosts.

## Als .exe für Windows

```bash
pip install pyinstaller
pyinstaller --onefile --noconsole --name Panzer panzer.py   # Ergebnis: dist/Panzer.exe
```
