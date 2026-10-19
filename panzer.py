#!/usr/bin/env python3
"""Panzer – AZ/Tank-Trouble-Klon mit Multiplayer über IP, Bots und ordentlich Krawall.

    python panzer.py                          Menü
    python panzer.py host [port] [bots]       Spiel hosten (Standard-Port 5555), freie Plätze mit Bots
    python panzer.py join <ip[:port]>         Einem Spiel beitreten
    python panzer.py local [2|3|4] [bots]     2 bis 4 Spieler an einer Tastatur, dazu Bots
    python panzer.py solo [bots]              Allein gegen Bots (Standard: 3)

Netzwerk: Der Host ist autoritativ und simuliert mit festen 60 Ticks/s. Bis zu 7 Clients
können beitreten (8 Spieler), freie Plätze kann der Host mit Bots füllen. Ab 5 Spielern wird
das Labyrinth größer und verkleinert angezeigt. Der Host schickt 30 Snapshots/s, jeder Client
schickt 30 Pakete/s mit allen Eingaben seit dem letzten Paket.
Der Client sagt seinen eigenen Panzer voraus (Prediction + Abgleich über "ack") und zeigt
den Rest leicht verzögert und interpoliert an, damit unregelmäßig ankommende Pakete nicht ruckeln.
Alle Effekte (Licht, Partikel, Trümmer, Wackeln) entstehen beim Zeichnen aus den Snapshots,
jeder Client berechnet sie selbst.
"""

import functools
import itertools
import json
import math
import random
import socket
import sys
import threading
import time
from array import array
from collections import deque
from pathlib import Path

import pygame

# --- Konstanten -------------------------------------------------------------

PORT = 5555
PROTO = 5  # Protokollversion, Host und Client müssen übereinstimmen
MAX_PLAYERS = 8
MAX_LOCAL = 4  # mehr passen nicht sinnvoll auf eine Tastatur
BIG_FROM = 5  # ab so vielen Spielern wird das Labyrinth größer
W, H = 1000, 680
ARENA_TOP, ARENA_H = 15, 560
VIEW_H = (
    ARENA_TOP + ARENA_H + 10
)  # Höhe des Spielfelds auf dem Bildschirm (darunter HUD)
FPS = 60
TICK = 1 / 60
SEND_EVERY = 1  # Snapshot bzw. Input-Paket nur jeden 2. Tick -> 30 Pakete/s
INTERP_DELAY = 0.075  # so weit in der Vergangenheit rendert der Client die anderen

CELL = 90
WALL = 8
TANK_R = 15
TANK_SPEED = 130  # px/s
TANK_ROT = math.radians(200)  # rad/s
MUZZLE = 20  # Abstand Mitte -> Rohrende
BULLET_R = 4
BULLET_SPEED = 220
BULLET_LIFE = 10.0
MAX_BULLETS = 5
FIRE_COOLDOWN = 0.12
OWNER_GRACE = 0.1  # so lange kann man sich nicht selbst treffen
ROUND_END_DELAY = 3.0

# Power-ups
BOX_FIRST = 4.0
BOX_EVERY = (6.0, 11.0)
BOX_MAX = 3
BOX_PICK = TANK_R + 12
# Waffen belegen den Waffenplatz, Effekte wirken sofort beim Aufheben
WEAPONS = [
    "frag",
    "mg",
    "laser",
    "rocket",
    "shotgun",
    "bouncy",
    "mine",
    "swap",
    "rang",
    "ice",
    "hole",
    "strike",
    "flame",
    "zap",
]
BUFFS = ["shield", "turbo", "mini", "ghost", "invis", "confuse"]
POWERUPS = (
    WEAPONS + BUFFS + ["mystery"]
)  # Kartoffel und Atombombe nur aus der Wundertüte
ALL_WEAPONS = WEAPONS + ["nuke"]
POWERUP_NAMES = {
    "frag": "Splitterbombe",
    "mg": "MG",
    "laser": "Laser",
    "rocket": "Lenkrakete",
    "shotgun": "Schrotflinte",
    "bouncy": "Flummi",
    "mine": "Minen",
    "swap": "Tauschkugel",
    "rang": "Bumerang",
    "ice": "Eisstrahl",
    "hole": "Schwarzes Loch",
    "strike": "Luftschlag",
    "flame": "Flammenwerfer",
    "zap": "Blitz",
    "nuke": "Atombombe",
    "shield": "Schild",
    "turbo": "Turbo",
    "mini": "Schrumpfpilz",
    "ghost": "Geist",
    "invis": "Tarnkappe",
    "confuse": "Verwirrung",
    "mystery": "Wundertüte",
    "potato": "Heiße Kartoffel",
    "frozen": "Eingefroren",
}
AMMO = {"mg": 25, "shotgun": 2, "mine": 3, "flame": 70}  # alle anderen Waffen: 1 Schuss
FRAG_R, FRAG_SPEED, FRAG_FUSE, FRAG_PIECES = 7, 170, 5.0, 26
MG_RATE, MG_SPREAD, MG_R, MG_LIFE = 0.07, 0.12, 3, 4.0
LASER_LEN, LASER_SHOW = 1400, 0.45
ROCKET_R, ROCKET_SPEED, ROCKET_TURN, ROCKET_LIFE = 5, 170, 4.0, 10.0
ROCKET_DELAY, ROCKET_GRACE = 0.4, 0.8
SMOKE_LIFE = 0.7
SHIELD_TIME = 8.0
SG_PELLETS, SG_SPREAD, SG_LIFE = 7, 0.32, 1.1
BOUNCY_R, BOUNCY_SPEED, BOUNCY_GAIN, BOUNCY_MAX, BOUNCY_LIFE = 6, 150, 1.15, 560, 7.0
MINE_R, MINE_ARM, MINE_BLAST = 7, 1.0, 44
SWAP_R, SWAP_SPEED, SWAP_LIFE = 6, 300, 2.5
RANG_R, RANG_SPEED, RANG_CURVE, RANG_TURN, RANG_BACK, RANG_LIFE = (
    6,
    250,
    2.0,
    5.0,
    0.8,
    5.0,
)
ICE_R, ICE_SPEED, ICE_LIFE, FREEZE_TIME = 5, 290, 3.0, 3.0
HOLE_R, HOLE_SPEED, HOLE_TRAVEL, HOLE_TIME = 7, 130, 1.2, 4.5
HOLE_RANGE, HOLE_PULL, HOLE_KILL, HOLE_SHOT_PULL = 210, 175, 16, 650
STRIKE_DIST, STRIKE_DELAY, STRIKE_BLAST = 280, 1.5, 62
# Flammenwerfer: Feuer halten, jede Flamme wird schnell langsamer und verglüht
FLAME_RATE, FLAME_SPREAD, FLAME_SPEED, FLAME_LIFE, FLAME_R, FLAME_DRAG = (
    0.03,
    0.16,
    340,
    0.42,
    7,
    2.2,
)
# Blitz: springt auf den nächsten Panzer vor einem und von dort weiter
ZAP_RANGE, ZAP_CHAIN, ZAP_JUMPS, ZAP_CONE, ZAP_SHOW = 250, 200, 3, 1.0, 0.45
# Atombombe (nur aus der Wundertüte): rollt aus und geht nach NUKE_FUSE s hoch
NUKE_R, NUKE_SPEED, NUKE_DRAG, NUKE_FUSE, NUKE_BLAST, NUKE_CHANCE = (
    10,
    150,
    1.3,
    4.0,
    230,
    0.15,
)
EFFECT_TIME = {"turbo": 6.0, "mini": 10.0, "ghost": 5.0, "invis": 8.0, "confuse": 6.0}
TURBO_SPEED, MINI_SCALE, MINI_SPEED = 1.65, 0.55, 1.15
POTATO_TIME, POTATO_MIN, POTATO_BLAST, POTATO_CHANCE = 10.0, 3.0, 55, 0.25
# Zeitlupe, sobald die Runde entschieden ist (Panzer fahren normal, alles andere langsam)
SLOWMO_TIME, SLOWMO_SCALE = 1.5, 0.25
# Ansagen: Mehrfachkill innerhalb dieser Zeit, Trickshot ab so vielen Abprallern
MULTI_WINDOW, TRICK_BOUNCES, ANN_TIME = 1.6, 3, 1.9

# Chaos-Runden: mit etwas Glück bekommt eine Runde eine Sonderregel
MUTATOR_CHANCE = 0.35
MUTATORS = {
    "kisten": ("Kistenregen", "Power-ups ohne Ende"),
    "hyper": ("Hyperkugeln", "Kugeln fast doppelt so schnell"),
    "riesen": ("Riesenkugeln", "dicke, langsame Kugeln"),
    "turbo": ("Alle auf Turbo", "alle Panzer fahren schneller"),
    "nebel": ("Nebel", "man sieht nur, was direkt vor einem liegt"),
    "sniper": ("Scharfschützen", "nur 1 Kugel, aber pfeilschnell"),
    "dunkel": ("Stromausfall", "Licht gibt es nur von Scheinwerfern und Feuer"),
}
FOG_R = 165

TAUNTS = [
    "Hehe!",
    "Ups …",
    "GG",
    "Zu langsam!",
    "Komm her!",
    "DU DUMMER NEGGER",
    "JUlIAN SPRING",
    "SCHEIß JUNKY",
]
TAUNT_TIME = 2.5
FEED_TIME, FEED_MAX = 5.0, 4

COLORS = [
    (220, 35, 35),
    (35, 190, 45),
    (45, 95, 225),
    (240, 140, 15),
    (150, 60, 200),
    (20, 175, 185),
    (235, 85, 170),
    (130, 85, 40),
]
NAMES = ["Rot", "Grün", "Blau", "Orange", "Lila", "Türkis", "Pink", "Braun"]
DEAD_SPRITE = len(COLORS)  # Wrack von Slot i = Sprite i + DEAD_SPRITE
BG = (11, 12, 17)
TEXT_C = (232, 236, 244)
DIM_C = (138, 146, 164)
INK_C = (28, 30, 38)  # dunkle Schrift auf hellem Grund
SHOT_C = (30, 30, 30)
ACCENT_C = (255, 196, 60)

# Arenen-Looks: der Host würfelt pro Runde einen aus (maze["theme"]), rein optisch
THEMES = [
    {
        "name": "Nachtbasis",
        "floor": (34, 38, 49),
        "line": (45, 51, 65),
        "wall": (84, 96, 120),
        "hi": (152, 170, 204),
        "lo": (36, 42, 56),
        "track": (25, 28, 37),
        "dust": (128, 138, 160),
        "accent": (110, 180, 255),
        "amb": "dust",
    },
    {
        "name": "Wüste",
        "floor": (70, 55, 40),
        "line": (79, 63, 46),
        "wall": (156, 122, 82),
        "hi": (220, 186, 132),
        "lo": (86, 64, 42),
        "track": (54, 42, 30),
        "dust": (196, 166, 120),
        "accent": (255, 170, 80),
        "amb": "sand",
    },
    {
        "name": "Vulkan",
        "floor": (36, 27, 27),
        "line": (47, 34, 33),
        "wall": (72, 54, 50),
        "hi": (150, 104, 84),
        "lo": (30, 21, 21),
        "track": (24, 17, 17),
        "dust": (96, 74, 68),
        "accent": (255, 110, 40),
        "glow": (255, 80, 15),
        "cracks": True,
        "amb": "ember",
    },
    {
        "name": "Gletscher",
        "floor": (30, 45, 62),
        "line": (40, 58, 78),
        "wall": (122, 172, 212),
        "hi": (226, 244, 255),
        "lo": (60, 96, 134),
        "track": (22, 34, 48),
        "dust": (205, 228, 248),
        "accent": (150, 220, 255),
        "amb": "snow",
    },
    {
        "name": "Dschungel",
        "floor": (30, 43, 31),
        "line": (38, 53, 38),
        "wall": (72, 100, 60),
        "hi": (132, 172, 104),
        "lo": (34, 50, 29),
        "track": (21, 31, 22),
        "dust": (112, 144, 92),
        "accent": (170, 240, 110),
        "amb": "firefly",
    },
    {
        "name": "Neon-City",
        "floor": (19, 16, 33),
        "line": (52, 28, 82),
        "wall": (34, 26, 64),
        "hi": (90, 235, 255),
        "lo": (255, 60, 200),
        "track": (13, 11, 24),
        "dust": (150, 100, 230),
        "accent": (255, 80, 210),
        "glow": (60, 150, 255),
        "amb": "neon",
    },
]

UP, DOWN, LEFT, RIGHT, FIRE = 1, 2, 4, 8, 16
TAUNT_SHIFT = 5  # Bits 5-7 der Tasten: Nummer des Spruchs (1-5), 0 = keiner

LAST_IP_FILE = Path(__file__).with_name(".panzer_last_ip")
RAY_STEP = 20  # Abtastschritt, mit dem ein Strahl die Wände seiner Zellen sammelt


# --- Labyrinth & Geometrie --------------------------------------------------


def maze_size(players):
    """Zellen (cols, rows). Bis 4 Spieler wie gehabt, danach pro Spieler deutlich größer."""
    if players < BIG_FROM:
        return random.randint(6, 10), random.randint(4, 6)
    extra = players - BIG_FROM + 1
    cols = random.randint(11 + 2 * extra, 13 + 2 * extra)
    rows = max(6, round(cols / 1.75) + random.randint(-1, 0))  # ungefähr Fensterformat
    return cols, rows


def generate_maze(maze_id, players=2):
    cols, rows = maze_size(players)
    # h[r][c]: Wand oberhalb von Zelle (r, c); v[r][c]: Wand links von Zelle (r, c)
    h = [[True] * cols for _ in range(rows + 1)]
    v = [[True] * (cols + 1) for _ in range(rows)]

    # Perfektes Labyrinth per DFS ...
    seen = [[False] * cols for _ in range(rows)]
    start = (random.randrange(rows), random.randrange(cols))
    seen[start[0]][start[1]] = True
    stack = [start]
    while stack:
        r, c = stack[-1]
        nbrs = [
            (r + dr, c + dc, dr, dc)
            for dr, dc in ((-1, 0), (1, 0), (0, -1), (0, 1))
            if 0 <= r + dr < rows and 0 <= c + dc < cols and not seen[r + dr][c + dc]
        ]
        if not nbrs:
            stack.pop()
            continue
        nr, nc, dr, dc = random.choice(nbrs)
        if dr == -1:
            h[r][c] = False
        elif dr == 1:
            h[r + 1][c] = False
        elif dc == -1:
            v[r][c] = False
        else:
            v[r][c + 1] = False
        seen[nr][nc] = True
        stack.append((nr, nc))

    # ... dann aufbrechen, damit es offener wird wie im Original
    for r in range(1, rows):
        for c in range(cols):
            if h[r][c] and random.random() < 0.4:
                h[r][c] = False
    for r in range(rows):
        for c in range(1, cols):
            if v[r][c] and random.random() < 0.4:
                v[r][c] = False

    # Passt das Labyrinth nicht ins Fenster, wird die Welt größer und beim Zeichnen
    # mit scale verkleinert. ww/wh = Weltgröße mit demselben Seitenverhältnis wie das Spielfeld.
    scale = min(1.0, W / (cols * CELL + 60), ARENA_H / (rows * CELL + 20))
    ww, wh = round(W / scale), round(VIEW_H / scale)
    ox = (ww - cols * CELL) // 2
    oy = round(ARENA_TOP / scale) + (round(ARENA_H / scale) - rows * CELL) // 2
    return {
        "id": maze_id,
        "cols": cols,
        "rows": rows,
        "ox": ox,
        "oy": oy,
        "ww": ww,
        "wh": wh,
        "h": [[int(b) for b in row] for row in h],  # Rohdaten, falls Wände wegfliegen
        "v": [[int(b) for b in row] for row in v],
        "walls": merge_walls(h, v, ox, oy),
    }


def merge_walls(h, v, ox, oy):
    """Wandstücke aus dem Raster als Rechtecke, zusammenhängende Stücke gemergt."""
    rows, cols = len(v), len(h[0])
    half = WALL // 2
    walls = []
    # Zusammenhängende Wandstücke zu einem Rechteck mergen
    for r in range(rows + 1):
        c = 0
        while c < cols:
            if h[r][c]:
                start = c
                while c < cols and h[r][c]:
                    c += 1
                walls.append(
                    [
                        ox + start * CELL - half,
                        oy + r * CELL - half,
                        (c - start) * CELL + WALL,
                        WALL,
                    ]
                )
            else:
                c += 1
    for c in range(cols + 1):
        r = 0
        while r < rows:
            if v[r][c]:
                start = r
                while r < rows and v[r][c]:
                    r += 1
                walls.append(
                    [
                        ox + c * CELL - half,
                        oy + start * CELL - half,
                        WALL,
                        (r - start) * CELL + WALL,
                    ]
                )
            else:
                r += 1
    return walls


def circle_hits_walls(cx, cy, r, walls):
    r2 = r * r
    for x, y, w, h in walls:
        nx = x if cx < x else x + w if cx > x + w else cx
        ny = y if cy < y else y + h if cy > y + h else cy
        if (cx - nx) ** 2 + (cy - ny) ** 2 < r2:
            return True
    return False


def push_out(cx, cy, r, walls):
    """Schiebt einen Kreis aus allen Wänden heraus."""
    for _ in range(4):
        moved = False
        for x, y, w, h in walls:
            nx = min(max(cx, x), x + w)
            ny = min(max(cy, y), y + h)
            dx, dy = cx - nx, cy - ny
            d2 = dx * dx + dy * dy
            if d2 >= r * r:
                continue
            moved = True
            if d2 > 1e-9:
                d = math.sqrt(d2)
                cx += dx / d * (r - d)
                cy += dy / d * (r - d)
            else:  # Mittelpunkt steckt in der Wand
                pen = {"l": cx - x, "r": x + w - cx, "t": cy - y, "b": y + h - cy}
                side = min(pen, key=pen.get)
                if side == "l":
                    cx = x - r
                elif side == "r":
                    cx = x + w + r
                elif side == "t":
                    cy = y - r
                else:
                    cy = y + h + r
        if not moved:
            break
    return cx, cy


def ray_box(x, y, dx, dy, bx, by, bw, bh):
    """Eintritts-Distanz eines Strahls in ein Rechteck und die getroffene Achse."""
    if abs(dx) < 1e-12:
        if not bx <= x <= bx + bw:
            return None, None
        tx0, tx1 = -math.inf, math.inf
    else:
        t1, t2 = (bx - x) / dx, (bx + bw - x) / dx
        tx0, tx1 = min(t1, t2), max(t1, t2)
    if abs(dy) < 1e-12:
        if not by <= y <= by + bh:
            return None, None
        ty0, ty1 = -math.inf, math.inf
    else:
        t1, t2 = (by - y) / dy, (by + bh - y) / dy
        ty0, ty1 = min(t1, t2), max(t1, t2)
    t_in, t_out = max(tx0, ty0), min(tx1, ty1)
    if t_in > t_out or t_in < 1e-9:  # verfehlt, dahinter oder wir stecken drin
        return None, None
    axis = "x" if tx0 > ty0 + 1e-9 else "y" if ty0 > tx0 + 1e-9 else "xy"
    return t_in, axis


def ray_circle(x, y, dx, dy, cx, cy, r):
    fx, fy = x - cx, y - cy
    b = fx * dx + fy * dy
    c = fx * fx + fy * fy - r * r
    if c < 0:  # Start im Kreis -> ignorieren (eigener Panzer)
        return None
    disc = b * b - c
    if disc < 0:
        return None
    t = -b - math.sqrt(disc)
    return t if t > 0 else None


class Arena:
    """Labyrinth mit Broadphase-Gitter und Zell-Nachbarschaft. Läuft auf Host und Client."""

    MARGIN = 24

    def __init__(self, maze):
        self.maze = maze
        self.id = maze["id"]
        self.cols, self.rows = maze["cols"], maze["rows"]
        self.ox, self.oy = maze["ox"], maze["oy"]
        self.ww, self.wh = maze.get("ww", W), maze.get("wh", VIEW_H)
        self.walls = [tuple(w) for w in maze["walls"]]
        self.mut = maze.get("mut")  # Sonderregel der Runde (oder None)
        self.theme = THEMES[maze.get("theme", 0) % len(THEMES)]
        m = self.MARGIN
        # Pro Zelle nur die Wände in der Nähe -> Kollisionstests prüfen ~4 statt ~30 Rechtecke
        self.grid = []
        for r in range(self.rows):
            row = []
            for c in range(self.cols):
                x0, y0 = self.ox + c * CELL - m, self.oy + r * CELL - m
                x1, y1 = x0 + CELL + 2 * m, y0 + CELL + 2 * m
                row.append(
                    [
                        w
                        for w in self.walls
                        if w[0] < x1
                        and w[0] + w[2] > x0
                        and w[1] < y1
                        and w[1] + w[3] > y0
                    ]
                )
            self.grid.append(row)
        # Welche Nachbarzellen sind offen verbunden (für die Lenkrakete)
        self.nbrs = {(r, c): [] for r in range(self.rows) for c in range(self.cols)}
        for r in range(self.rows):
            for c in range(self.cols):
                for dr, dc in ((0, 1), (1, 0)):
                    r2, c2 = r + dr, c + dc
                    if r2 >= self.rows or c2 >= self.cols:
                        continue
                    if dc:
                        px, py = self.ox + c2 * CELL, self.oy + r * CELL + CELL / 2
                    else:
                        px, py = self.ox + c * CELL + CELL / 2, self.oy + r2 * CELL
                    if not any(
                        x <= px <= x + w and y <= py <= y + h
                        for x, y, w, h in self.walls
                    ):
                        self.nbrs[(r, c)].append((r2, c2))
                        self.nbrs[(r2, c2)].append((r, c))

    def cell(self, x, y):
        c = min(max(int((x - self.ox) // CELL), 0), self.cols - 1)
        r = min(max(int((y - self.oy) // CELL), 0), self.rows - 1)
        return r, c

    def near(self, x, y):
        r, c = self.cell(x, y)
        return self.grid[r][c]

    def center(self, r, c):
        return self.ox + c * CELL + CELL / 2, self.oy + r * CELL + CELL / 2

    def path(self, start, goal):
        prev = {start: None}
        q = deque([start])
        while q:
            cur = q.popleft()
            if cur == goal:
                break
            for n in self.nbrs[cur]:
                if n not in prev:
                    prev[n] = cur
                    q.append(n)
        if goal not in prev:
            return [start]
        out = [goal]
        while prev[out[-1]] is not None:
            out.append(prev[out[-1]])
        return out[::-1]

    def bfs(self, start):
        """Schritte von start zu jeder Zelle und Vorgänger (für die Wege der Bots)."""
        dist, prev = {start: 0}, {start: None}
        q = deque([start])
        while q:
            cur = q.popleft()
            for n in self.nbrs[cur]:
                if n not in dist:
                    dist[n] = dist[cur] + 1
                    prev[n] = cur
                    q.append(n)
        return dist, prev

    def _first_wall(self, x, y, dx, dy, length):
        """Erste Wand auf dem Strahl: (Strecke, Achse) bzw. (length, None).
        Sammelt nur die Wände der Zellen entlang des Strahls statt alle zu prüfen. Jeder
        Auftreffpunkt liegt höchstens RAY_STEP/2 neben einem Abtastpunkt, also im Rand
        (MARGIN) von dessen Zelle -> gleiches Ergebnis wie die Suche über alle Wände."""
        best, axis = length, None
        grid, seen, last = self.grid, set(), None
        d = 0.0
        while d <= best + RAY_STEP:
            cell = self.cell(x + dx * d, y + dy * d)
            if cell != last:
                last = cell
                for w in grid[cell[0]][cell[1]]:
                    if w not in seen:
                        seen.add(w)
                        t, ax = ray_box(x, y, dx, dy, *w)
                        if t is not None and t < best:
                            best, axis = t, ax
            d += RAY_STEP
        return best, axis

    def clear(self, x0, y0, x1, y1):
        """Freie Sicht zwischen zwei Punkten (keine Wand dazwischen)?"""
        d = math.hypot(x1 - x0, y1 - y0)
        if d < 1e-6:
            return True
        return self._first_wall(x0, y0, (x1 - x0) / d, (y1 - y0) / d, d)[1] is None

    def raycast(self, x, y, a, length, circles=()):
        """Abprallender Strahl. Gibt (Punkte, Index des getroffenen Kreises oder None) zurück."""
        dx, dy = math.cos(a), math.sin(a)
        pts = [(x, y)]
        for _ in range(40):
            if length <= 0.5:
                break
            t_wall, axis = self._first_wall(x, y, dx, dy, length)
            best, hit = t_wall, None
            for i, (cx, cy, cr) in enumerate(circles):
                t = ray_circle(x, y, dx, dy, cx, cy, cr)
                if t is not None and t < best:
                    best, hit = t, i
            if hit is not None:
                pts.append((x + dx * best, y + dy * best))
                return pts, hit
            x, y = x + dx * t_wall, y + dy * t_wall
            pts.append((x, y))
            length -= t_wall
            if axis is None:
                break
            if "x" in axis:
                dx = -dx
            if "y" in axis:
                dy = -dy
            x, y = x + dx * 0.01, y + dy * 0.01
        return pts, None


def tank_radius(eff):
    return TANK_R * MINI_SCALE if "mini" in eff else TANK_R


def bullet_params(mut):
    """(Tempo, Radius, Lebensdauer, max. Anzahl) der normalen Kugeln je nach Sonderregel."""
    if mut == "hyper":
        return BULLET_SPEED * 1.8, BULLET_R, BULLET_LIFE, MAX_BULLETS
    if mut == "riesen":
        return BULLET_SPEED * 0.85, 10, BULLET_LIFE, MAX_BULLETS
    if mut == "sniper":
        return BULLET_SPEED * 2.2, BULLET_R, 3.5, 1
    return BULLET_SPEED, BULLET_R, BULLET_LIFE, MAX_BULLETS


def clamp_arena(x, y, r, arena):
    x0, y0 = arena.ox + r, arena.oy + r
    x1, y1 = arena.ox + arena.cols * CELL - r, arena.oy + arena.rows * CELL - r
    return min(max(x, x0), x1), min(max(y, y0), y1)


def move_tank(t, keys, dt, arena):
    """Panzerbewegung – identisch auf Host und Client (Prediction). t.eff = aktive Effekte."""
    eff = t.eff
    if "frozen" in eff:
        return
    if "confuse" in eff:  # vorne/hinten und links/rechts vertauscht
        keys = (
            ((keys & UP) << 1)
            | ((keys & DOWN) >> 1)
            | ((keys & LEFT) << 1)
            | ((keys & RIGHT) >> 1)
        )
    speed, rot = TANK_SPEED, TANK_ROT
    if "turbo" in eff:
        speed, rot = speed * TURBO_SPEED, rot * 1.25
    if "mini" in eff:
        speed *= MINI_SPEED
    if arena.mut == "turbo":
        speed *= 1.4
    if keys & LEFT:
        t.a -= rot * dt
    if keys & RIGHT:
        t.a += rot * dt
    t.a %= 2 * math.pi
    move = (1 if keys & UP else 0) - (0.7 if keys & DOWN else 0)
    if move:
        r = tank_radius(eff)
        t.x += math.cos(t.a) * speed * move * dt
        t.y += math.sin(t.a) * speed * move * dt
        if "ghost" in eff:  # durch Wände, aber nicht aus dem Labyrinth raus
            t.x, t.y = clamp_arena(t.x, t.y, r, arena)
        else:
            t.x, t.y = push_out(t.x, t.y, r, arena.near(t.x, t.y))


# --- Spiellogik (läuft nur beim Host bzw. lokal) -----------------------------


KILL_TEXTS = {
    "b": "{k} hat {v} abgeschossen",
    "mg": "{k} hat {v} durchlöchert",
    "sg": "{k} hat {v} mit Schrot durchsiebt",
    "frag": "{k} hat {v} die Bombe direkt reingedrückt",
    "frg": "{k} hat {v} zerbröselt",
    "laser": "{k} hat {v} gegrillt",
    "rocket": "{k}s Rakete hat {v} gefunden",
    "bnc": "{k}s Flummi hat {v} erwischt",
    "rang": "{k} hat {v} mit dem Bumerang umgehauen",
    "mine": "{v} ist auf {k}s Mine getreten",
    "hole": "{k}s Schwarzes Loch hat {v} verschluckt",
    "strike": "{k} hat {v} aus der Luft erwischt",
    "potato": "{v} stand zu nah an {k}s Kartoffel",
    "fl": "{k} hat {v} flambiert",
    "zap": "{k} hat {v} unter Strom gesetzt",
    "nuke": "{k} hat {v} atomisiert",
}
SELF_TEXTS = {
    "laser": "{v} hat sich selbst gegrillt",
    "mine": "{v} ist auf die eigene Mine getreten",
    "hole": "{v} wurde vom eigenen Schwarzen Loch verschluckt",
    "strike": "{v} hat den Luftschlag auf sich selbst bestellt",
    "potato": "Die Kartoffel ist bei {v} explodiert",
    "bnc": "{v} hat den eigenen Flummi unterschätzt",
    "nuke": "{v} stand zu nah an der eigenen Atombombe",
}
SELF_GENERIC = [
    "{v} hat sich selbst erledigt",
    "{v}: Eigentor!",
    "{v} wurde von der eigenen Kugel überrascht",
]
MULTI_KILLS = {2: "DOPPELKILL!", 3: "DREIFACHKILL!", 4: "MEGAKILL!"}
TRICK_KINDS = ("b", "bnc", "mg", "sg", "laser")  # Waffen, bei denen Abpraller zählen
AGED = ("mine", "hole", "strike", "frag", "fl", "nuke")  # Renderer braucht deren Alter


def kill_text(by, victim, how):
    v = NAMES[victim]
    if by is None or by == victim:
        return SELF_TEXTS.get(how, random.choice(SELF_GENERIC)).format(v=v)
    return KILL_TEXTS.get(how, KILL_TEXTS["b"]).format(k=NAMES[by], v=v)


def streak_text(n):
    """Untertitel zum Rundensieg, je nachdem wie viele Runden in Folge es waren."""
    if n < 2:
        return ""
    return {2: "Zweiter Sieg in Folge", 3: "HATTRICK!", 4: "Nicht zu stoppen!"}.get(
        n, f"{n} Siege in Folge – UNAUFHALTSAM!"
    )


def first_wall(arena, x, y, a, length):
    """Strecke bis zur ersten Wand in Richtung a (höchstens length)."""
    pts = arena.raycast(x, y, a, length)[0]
    return math.hypot(pts[1][0] - x, pts[1][1] - y) if len(pts) > 1 else 0.0


def strike_target(arena, x, y, a):
    """Zielpunkt des Luftschlags: STRIKE_DIST voraus, aber vor der ersten Wand."""
    d = max(0.0, first_wall(arena, x, y, a, STRIKE_DIST) - 14)
    return x + math.cos(a) * d, y + math.sin(a) * d


class Tank:
    def __init__(self, slot, x, y, a):
        self.slot = slot  # Spieler-Nummer = Farbe, bleibt über Runden gleich
        self.x, self.y, self.a = x, y, a
        self.alive = True
        self.dead_age = 0.0
        self.cooldown = 0.0
        self.fire_seen = None
        self.weapon = None
        self.ammo = 0
        self.shield = 0.0
        self.eff = {}  # Effekt -> Restzeit. Schlüssel mit "_" bleiben beim Host.
        self.taunt, self.taunt_time, self.taunt_key = 0, 0.0, 0


class Shot:
    __slots__ = (
        "id",
        "kind",
        "owner",
        "x",
        "y",
        "vx",
        "vy",
        "speed",
        "r",
        "life",
        "grace",
        "age",
        "dead",
        "bounces",
    )

    def __init__(self, sid, kind, owner, x, y, a, speed, r, life, grace):
        self.id, self.kind, self.owner = sid, kind, owner
        self.x, self.y = x, y
        self.vx, self.vy = math.cos(a) * speed, math.sin(a) * speed
        self.speed, self.r, self.life, self.grace = speed, r, life, grace
        self.age = 0.0
        self.dead = False  # wird am Ende von Game.step() aussortiert
        self.bounces = 0  # Abpraller insgesamt (für Trickshots)

    def travel(self, dt, arena):
        """Bewegt die Kugel, prallt achsweise an Wänden ab (Substeps von max. 3 px).
        Gibt die Anzahl der Abpraller zurück."""
        bounces = 0
        steps = max(1, math.ceil(self.speed * dt / 3))
        f = dt / steps
        for _ in range(steps):
            self.x += self.vx * f
            if circle_hits_walls(self.x, self.y, self.r, arena.near(self.x, self.y)):
                self.x -= self.vx * f
                self.vx = -self.vx
                bounces += 1
            self.y += self.vy * f
            if circle_hits_walls(self.x, self.y, self.r, arena.near(self.x, self.y)):
                self.y -= self.vy * f
                self.vy = -self.vy
                bounces += 1
        return bounces


class Fx:
    def __init__(self, fid, kind, data, life):
        self.id, self.kind, self.data, self.life, self.age = fid, kind, data, life, 0.0


class Game:
    def __init__(self, players=(0, 1)):
        self.players = list(players)
        self.scores = [0] * MAX_PLAYERS
        self.streak = [0] * MAX_PLAYERS  # gewonnene Runden in Folge
        self.last_winner = None
        self.maze_id = 0
        self.time = 0.0
        self.next_id = 0
        self.fire_seen = [None] * MAX_PLAYERS
        self.feed = []  # Killfeed: [text, farbe (slot), alter, opfer, waffe]
        self.mut = None
        self.theme = None
        self.bots = []  # Slots der Computergegner (nur für die Anzeige)
        self.new_round()

    def set_players(self, players, fresh=()):
        """Neue Spielerliste -> sofort neue Runde. Neue Spieler (fresh) starten bei 0 Punkten."""
        for slot in fresh:
            self.scores[slot] = 0
            self.streak[slot] = 0
            self.fire_seen[slot] = None  # neuer Client zählt seine Schüsse ab 0
        self.players = list(players)
        self.new_round()

    def new_round(self):
        self.maze_id += 1
        self.maze = generate_maze(self.maze_id, len(self.players))
        # Sonderregel, nie zweimal dieselbe hintereinander. Steckt im Maze -> der Client kennt sie.
        mut = None
        if random.random() < MUTATOR_CHANCE:
            mut = random.choice([m for m in MUTATORS if m != self.mut])
        self.mut = self.maze["mut"] = mut
        # Jede Runde ein anderer Look (rein optisch, steckt auch im Maze)
        self.theme = self.maze["theme"] = random.choice(
            [k for k in range(len(THEMES)) if k != self.theme]
        )
        self.arena = Arena(self.maze)
        m = self.maze
        cells = [(r, c) for r in range(m["rows"]) for c in range(m["cols"])]
        # Startfelder mit möglichst großem Abstand, wenn es nicht klappt etwas lockerer.
        # Bei vielen Spielern passt der halbe Umfang nicht mehr -> Abstand nach Fläche pro Spieler.
        n = len(self.players)
        min_dist = min(
            (m["rows"] + m["cols"]) // 2, max(2, int(1.6 * math.sqrt(len(cells) / n)))
        )
        for attempt in range(400):
            spots = random.sample(cells, n)
            need = min_dist - attempt // 100
            if all(
                abs(a[0] - b[0]) + abs(a[1] - b[1]) >= need
                for a, b in itertools.combinations(spots, 2)
            ):
                break
        self.tanks = []
        for slot, (r, c) in zip(self.players, spots):
            t = Tank(
                slot,
                *self.arena.center(r, c),
                random.choice((0, 0.5, 1, 1.5)) * math.pi,
            )
            t.fire_seen = self.fire_seen[slot]
            self.tanks.append(t)
        self.by_slot = {t.slot: t for t in self.tanks}
        self.shots = []
        self.boxes = []
        self.fx = []
        self.box_timer = 1.0 if mut == "kisten" else BOX_FIRST
        self.box_max = max(BOX_MAX, len(cells) // 30)  # große Labyrinthe: mehr Kisten
        self.end_timer = None
        self.msg = None
        self.win = None  # [Sieger-Slot oder None, Untertitel]
        self.kills = {}  # slot -> Zeitpunkte seiner Abschüsse in dieser Runde
        self.first_blood = False
        self.slow = self.slow_len = 0.0  # Zeitlupe: Restzeit und Gesamtlänge
        self.ts = 1.0  # aktueller Zeitfaktor

    # Eingabe eines Spielers. Beim Client-Panzer wird das pro empfangenem Input-Kommando
    # mit dessen dt aufgerufen, damit die Prediction des Clients exakt übereinstimmt.
    def apply_input(self, i, keys, fires, dt):
        t = self.by_slot.get(i)
        if t is None:
            return
        # Schuss-Zähler: jeder Tastendruck erhöht ihn -> keine verlorenen Taps
        new_shot = t.fire_seen is not None and fires > t.fire_seen
        t.fire_seen = self.fire_seen[i] = fires
        if not t.alive:
            return
        taunt = (keys >> TAUNT_SHIFT) & 7
        if taunt and taunt != t.taunt_key and taunt <= len(TAUNTS):
            t.taunt, t.taunt_time = taunt, TAUNT_TIME
        t.taunt_key = taunt
        move_tank(t, keys, dt, self.arena)
        t.cooldown = max(0.0, t.cooldown - dt)
        if "frozen" in t.eff:
            return
        if new_shot:
            self._trigger(i, t)
        # Dauerfeuer-Waffen: solange die Taste gedrückt ist
        if (
            t.weapon in ("mg", "flame")
            and keys & FIRE
            and t.cooldown == 0
            and not self._in_wall(t)
        ):
            if t.weapon == "mg":
                self._shoot(
                    "mg",
                    i,
                    t,
                    t.a + random.uniform(-MG_SPREAD, MG_SPREAD),
                    BULLET_SPEED * 1.1,
                    MG_R,
                    MG_LIFE,
                    OWNER_GRACE,
                )
                t.cooldown = MG_RATE
            else:
                self._shoot(
                    "fl",
                    i,
                    t,
                    t.a + random.uniform(-FLAME_SPREAD, FLAME_SPREAD),
                    FLAME_SPEED * random.uniform(0.85, 1.1),
                    FLAME_R,
                    FLAME_LIFE * random.uniform(0.8, 1.1),
                    FLAME_LIFE * 2,  # die eigenen Flammen verbrennen einen nicht
                )
                t.cooldown = FLAME_RATE
            self._use_ammo(t)

    def _in_wall(self, t):
        """Ein Geist in der Wand kann nicht schießen (die Kugel würde drin stecken)."""
        return "ghost" in t.eff and circle_hits_walls(
            t.x, t.y, 11, self.arena.near(t.x, t.y)
        )

    @staticmethod
    def _use_ammo(t):
        t.ammo -= 1
        if t.ammo <= 0:
            t.weapon = None

    def _shoot(self, kind, owner, t, a, speed, r, life, grace):
        # Mini-Panzer an der Wand + dicke Kugel: Startpunkt erst aus der Wand schieben
        x, y = push_out(t.x, t.y, r, self.arena.near(t.x, t.y))
        s = Shot(self.next_id, kind, owner, x, y, a, speed, r, life, grace)
        self.next_id += 1
        s.travel(
            MUZZLE / speed, self.arena
        )  # von der Mitte bis zum Rohrende, prallt korrekt ab
        self.shots.append(s)
        return s

    def _place(self, kind, owner, x, y, r, life):
        """Liegendes Objekt ohne Bewegung (Mine, Luftschlag-Markierung)."""
        s = Shot(self.next_id, kind, owner, x, y, 0, 0, r, life, 0)
        self.next_id += 1
        self.shots.append(s)
        return s

    def _fx(self, kind, data, life):
        self.fx.append(Fx(self.next_id, kind, data, life))
        self.next_id += 1

    def _feed(self, text, slot, victim, how):
        self.feed = (self.feed + [[text, slot, 0.0, victim, how]])[-FEED_MAX:]

    def _announce(self, text, slot, sub=""):
        """Große Ansage in der Bildmitte (Doppelkill, Trickshot ...)."""
        self._fx("ann", [text, slot, sub], ANN_TIME)

    def _slowmo(self, length):
        if length > self.slow:
            self.slow = self.slow_len = length

    def _time_scale(self):
        """Zeitfaktor der Zeitlupe: erst richtig langsam, dann weich zurück auf normal."""
        if self.slow <= 0:
            return 1.0
        k = (self.slow_len - self.slow) / self.slow_len
        if k < 0.4:
            return SLOWMO_SCALE
        k = (k - 0.4) / 0.6
        return SLOWMO_SCALE + (1 - SLOWMO_SCALE) * k * k * (3 - 2 * k)

    def _settle(self, t, x, y):
        """Gültige Position für Panzer t in der Nähe von (x, y)."""
        r = tank_radius(t.eff)
        if "ghost" in t.eff:
            return clamp_arena(x, y, r, self.arena)
        return push_out(x, y, r, self.arena.near(x, y))

    def _trigger(self, i, t):
        # 2. Druck: Splitterbombe zünden bzw. zur Tauschkugel springen
        for s in self.shots:
            if s.owner == i and not s.dead and s.kind in ("frag", "swap"):
                (self._explode if s.kind == "frag" else self._teleport)(s)
                return
        if t.cooldown > 0 or t.weapon in ("mg", "flame") or self._in_wall(t):
            return
        w = t.weapon
        if w is None:
            speed, r, life, most = bullet_params(self.mut)
            if sum(1 for s in self.shots if s.owner == i and s.kind == "b") >= most:
                return
            self._shoot("b", i, t, t.a, speed, r, life, OWNER_GRACE)
            t.cooldown = FIRE_COOLDOWN
            return
        if w == "frag":
            self._shoot("frag", i, t, t.a, FRAG_SPEED, FRAG_R, FRAG_FUSE, 0.3)
        elif w == "rocket":
            self._shoot(
                "rocket", i, t, t.a, ROCKET_SPEED, ROCKET_R, ROCKET_LIFE, ROCKET_GRACE
            )
        elif w == "laser":
            self._laser(i, t)
        elif w == "shotgun":
            for k in range(SG_PELLETS):
                a = t.a + SG_SPREAD * (2 * k / (SG_PELLETS - 1) - 1)
                self._shoot(
                    "sg",
                    i,
                    t,
                    a + random.uniform(-0.04, 0.04),
                    BULLET_SPEED * random.uniform(1.1, 1.35),
                    3,
                    SG_LIFE * random.uniform(0.85, 1.1),
                    OWNER_GRACE,
                )
        elif w == "bouncy":
            self._shoot("bnc", i, t, t.a, BOUNCY_SPEED, BOUNCY_R, BOUNCY_LIFE, 0.3)
        elif w == "mine":  # hinter den Panzer legen, aber nicht durch eine Wand
            back = t.a + math.pi
            d = min(
                tank_radius(t.eff) + MINE_R + 3,
                first_wall(self.arena, t.x, t.y, back, 40) - MINE_R - 1,
            )
            x, y = t.x + math.cos(back) * max(d, 0), t.y + math.sin(back) * max(d, 0)
            self._place("mine", i, x, y, MINE_R, 1e9)
        elif w == "swap":
            self._shoot("swap", i, t, t.a, SWAP_SPEED, SWAP_R, SWAP_LIFE, 0.4)
        elif w == "rang":
            self._shoot("rang", i, t, t.a, RANG_SPEED, RANG_R, RANG_LIFE, 0.5)
        elif w == "ice":
            self._shoot("ice", i, t, t.a, ICE_SPEED, ICE_R, ICE_LIFE, OWNER_GRACE)
        elif w == "hole":
            self._shoot(
                "hole", i, t, t.a, HOLE_SPEED, HOLE_R, HOLE_TRAVEL + HOLE_TIME, 1e9
            )
        elif w == "strike":
            self._place(
                "strike", i, *strike_target(self.arena, t.x, t.y, t.a), 0, STRIKE_DELAY
            )
        elif w == "zap":
            self._zap(i, t)
        elif w == "nuke":
            self._shoot("nuke", i, t, t.a, NUKE_SPEED, NUKE_R, NUKE_FUSE, 1e9)
        self._use_ammo(t)
        t.cooldown = 0.35 if w == "mine" else FIRE_COOLDOWN

    def _explode(self, bomb):
        bomb.dead = True
        for k in range(FRAG_PIECES):
            a = 2 * math.pi * k / FRAG_PIECES + random.uniform(-0.1, 0.1)
            self.shots.append(
                Shot(
                    self.next_id,
                    "frg",
                    bomb.owner,
                    bomb.x,
                    bomb.y,
                    a,
                    random.uniform(200, 300),
                    2,
                    random.uniform(0.5, 0.9),
                    0,
                )
            )
            self.next_id += 1
        self._fx("boom", [round(bomb.x), round(bomb.y)], 0.5)

    def _blast(self, x, y, radius, by, how, size=1.0, pierce=False):
        """Explosion: trifft alle Panzer im Radius, Wände schützen nicht. size 0 = ohne Feuerball."""
        if size:
            self._fx("boom", [round(x), round(y), size], 0.6)
        for t in self.tanks:
            if t.alive and math.hypot(t.x - x, t.y - y) < radius + tank_radius(t.eff):
                self._hit(t, by, how, pierce, angle=math.atan2(t.y - y, t.x - x))

    def _nuke(self, s):
        """Atombombe: riesige Explosion, auch durch Wände und Schilde. Reißt Minen, Bomben
        und die Wände in der Nähe mit (die Außenwände bleiben)."""
        s.dead = True
        self._blow_walls(s.x, s.y, NUKE_BLAST * 0.7)
        self._fx("nuke", [round(s.x), round(s.y)], 2.5)
        self._slowmo(1.2)
        for o in self.shots[:]:
            if o.dead or math.hypot(o.x - s.x, o.y - s.y) > NUKE_BLAST:
                continue
            if o.kind == "mine":
                self._mine_boom(o)
            elif o.kind == "frag":
                self._explode(o)
        self._blast(s.x, s.y, NUKE_BLAST, s.owner, "nuke", 0, pierce=True)

    def _blow_walls(self, x, y, radius):
        """Innere Wandstücke im Umkreis entfernen. Neues Maze mit derselben id, aber höherer
        rev -> der Host verschickt es, Client und Renderer tauschen die Arena mitten in der Runde.
        """
        m = self.maze
        h, v = m["h"], m["v"]
        gone = 0
        for r in range(1, m["rows"]):
            for c in range(m["cols"]):
                if (
                    h[r][c]
                    and math.hypot(
                        m["ox"] + (c + 0.5) * CELL - x, m["oy"] + r * CELL - y
                    )
                    < radius
                ):
                    h[r][c], gone = 0, gone + 1
        for r in range(m["rows"]):
            for c in range(1, m["cols"]):
                if (
                    v[r][c]
                    and math.hypot(
                        m["ox"] + c * CELL - x, m["oy"] + (r + 0.5) * CELL - y
                    )
                    < radius
                ):
                    v[r][c], gone = 0, gone + 1
        if gone:
            m["walls"] = merge_walls(h, v, m["ox"], m["oy"])
            m["rev"] = m.get("rev", 0) + 1
            self.arena = Arena(m)

    def _teleport(self, s):
        s.dead = True
        t = self.by_slot.get(s.owner)
        if t is None or not t.alive or "frozen" in t.eff:
            return
        self._fx("tele", [round(t.x), round(t.y)], 0.5)
        t.x, t.y = self._settle(t, s.x, s.y)
        self._fx("tele", [round(t.x), round(t.y)], 0.5)

    def _swap(self, s, t):
        """Tauschkugel trifft Panzer t -> Schütze und t tauschen die Plätze."""
        owner = self.by_slot.get(s.owner)
        if owner is None or not owner.alive or owner is t:
            return
        self._fx("tele", [round(owner.x), round(owner.y)], 0.5)
        self._fx("tele", [round(t.x), round(t.y)], 0.5)
        (owner.x, owner.y), (t.x, t.y) = (
            self._settle(owner, t.x, t.y),
            self._settle(t, owner.x, owner.y),
        )

    def _laser(self, i, t):
        targets = [o for o in self.tanks if o.alive]
        circles = [(o.x, o.y, tank_radius(o.eff)) for o in targets]
        pts, hit = self.arena.raycast(t.x, t.y, t.a, LASER_LEN, circles)
        if hit is not None:
            (x0, y0), (x1, y1) = pts[-2], pts[-1]
            self._hit(
                targets[hit],
                i,
                "laser",
                angle=math.atan2(y1 - y0, x1 - x0),
                bounces=len(pts) - 2,
            )
        self._fx("laser", [i, [[round(x), round(y)] for x, y in pts]], LASER_SHOW)

    def _zap(self, i, t):
        """Blitz: trifft den nächsten Panzer vor einem (freie Sicht) und springt weiter."""
        x, y, rng = t.x, t.y, ZAP_RANGE
        pts, hit = [(t.x + math.cos(t.a) * MUZZLE, t.y + math.sin(t.a) * MUZZLE)], []
        for _ in range(ZAP_JUMPS):
            best = None
            for o in self.tanks:
                if not o.alive or o is t or o in hit:
                    continue
                d = math.hypot(o.x - x, o.y - y)
                if d > rng or (best and d >= best[0]):
                    continue
                if not hit:  # der erste Sprung geht nur nach vorne
                    diff = (math.atan2(o.y - y, o.x - x) - t.a + math.pi) % (
                        2 * math.pi
                    ) - math.pi
                    if abs(diff) > ZAP_CONE:
                        continue
                if self.arena.clear(x, y, o.x, o.y):
                    best = (d, o)
            if best is None:
                break
            o = best[1]
            hit.append(o)
            pts.append((o.x, o.y))
            x, y, rng = o.x, o.y, ZAP_CHAIN
        if not hit:  # ins Leere: schlägt in die nächste Wand ein
            d = first_wall(self.arena, t.x, t.y, t.a, ZAP_RANGE * 0.6)
            pts.append((t.x + math.cos(t.a) * d, t.y + math.sin(t.a) * d))
        prev = pts[0]
        for o in hit:
            self._hit(o, i, "zap", angle=math.atan2(o.y - prev[1], o.x - prev[0]))
            prev = (o.x, o.y)
        self._fx("zap", [i, [[round(px), round(py)] for px, py in pts]], ZAP_SHOW)

    def _hit(self, t, by=None, how="b", pierce=False, angle=None, bounces=0):
        """Trifft Panzer t. pierce = Schild hilft nicht. True, wenn er zerstört wurde.
        angle = Flugrichtung des Treffers (dorthin fliegen die Trümmer)."""
        if not t.alive:
            return False
        if t.shield > 0 and not pierce:
            t.shield = 0.0
            self._fx("block", [round(t.x), round(t.y)], 0.6)
            return False
        t.alive = False
        t.weapon = None
        t.eff.clear()  # eine Kartoffel verschwindet mit
        if angle is None:
            angle = random.uniform(-math.pi, math.pi)
        self._fx("boom", [round(t.x), round(t.y), 1.25], 0.8)
        self._fx("die", [round(t.x), round(t.y), t.slot, round(angle, 2)], 0.5)
        self._feed(
            kill_text(by, t.slot, how), t.slot if by is None else by, t.slot, how
        )
        self._kill_event(t, by, how, bounces)
        return True

    def _kill_event(self, t, by, how, bounces):
        """Ansagen: Eigentor, Mehrfachkill, Trickshot, Erstes Blut."""
        if by is None or by == t.slot:
            self._fx("pop", [round(t.x), round(t.y) - 34, "EIGENTOR!", t.slot], 1.6)
            return
        kills = self.kills.setdefault(by, [])
        kills.append(self.time)
        multi = sum(1 for k in kills if self.time - k < MULTI_WINDOW)
        if multi >= 2:
            self._announce(MULTI_KILLS[min(multi, 4)], by, NAMES[by])
        elif how in TRICK_KINDS and bounces >= TRICK_BOUNCES:
            self._announce("TRICKSHOT!", by, f"{NAMES[by]} · {bounces} Abpraller")
        elif not self.first_blood and len(self.tanks) > 2:
            self._announce("ERSTES BLUT!", by, NAMES[by])
        self.first_blood = True

    @staticmethod
    def _turn_toward(s, tx, ty, rate, dt):
        cur = math.atan2(s.vy, s.vx)
        diff = (math.atan2(ty - s.y, tx - s.x) - cur + math.pi) % (
            2 * math.pi
        ) - math.pi
        cur += max(-rate * dt, min(rate * dt, diff))
        s.vx, s.vy = math.cos(cur) * s.speed, math.sin(cur) * s.speed

    def _rocket_steer(self, s, dt):
        # Getarnte Panzer sieht die Rakete nicht
        enemies = [
            t
            for t in self.tanks
            if t.alive and t.slot != s.owner and "invis" not in t.eff
        ]
        owner = self.by_slot.get(s.owner)
        if not enemies and s.age > s.grace and owner and owner.alive:
            enemies = [owner]
        if not enemies:
            return
        target = min(enemies, key=lambda t: math.hypot(t.x - s.x, t.y - s.y))
        path = self.arena.path(
            self.arena.cell(s.x, s.y), self.arena.cell(target.x, target.y)
        )
        tx, ty = (target.x, target.y) if len(path) <= 2 else self.arena.center(*path[1])
        self._turn_toward(s, tx, ty, ROCKET_TURN, dt)

    def _rang_steer(self, s, dt):
        """Bumerang: fliegt erst eine Kurve, dann zurück zum Werfer."""
        owner = self.by_slot.get(s.owner)
        if s.age < RANG_BACK or owner is None or not owner.alive:
            cur = math.atan2(s.vy, s.vx) + RANG_CURVE * dt
            s.vx, s.vy = math.cos(cur) * s.speed, math.sin(cur) * s.speed
        else:
            self._turn_toward(s, owner.x, owner.y, RANG_TURN, dt)

    def _hole_pull(self, s, dt):
        """Aktives Schwarzes Loch: zieht Panzer und Geschosse an, verschluckt alles in der Mitte."""
        s.speed = s.vx = s.vy = 0
        for t in self.tanks:
            if not t.alive:
                continue
            dx, dy = s.x - t.x, s.y - t.y
            d = math.hypot(dx, dy)
            if d < HOLE_KILL:
                self._hit(t, s.owner, "hole", pierce=True, angle=math.atan2(dy, dx))
            elif d < HOLE_RANGE:
                step = min(HOLE_PULL * (1 - d / HOLE_RANGE) * dt, d)
                t.x, t.y = self._settle(t, t.x + dx / d * step, t.y + dy / d * step)
        for o in self.shots:
            if o.dead or o.kind in ("hole", "mine", "strike", "nuke"):
                continue
            dx, dy = s.x - o.x, s.y - o.y
            d = math.hypot(dx, dy)
            if d < 10:
                o.dead = True
            elif d < HOLE_RANGE:
                acc = HOLE_SHOT_PULL * (1 - d / HOLE_RANGE) * dt
                vx, vy = o.vx + dx / d * acc, o.vy + dy / d * acc
                v = math.hypot(vx, vy) or 1
                o.vx, o.vy = vx / v * o.speed, vy / v * o.speed

    def _mine_check(self, s):
        if s.age > MINE_ARM and any(
            t.alive and math.hypot(t.x - s.x, t.y - s.y) < tank_radius(t.eff) + MINE_R
            for t in self.tanks
        ):
            return self._mine_boom(s)
        # Draufschießen zündet die Mine auch (Flammen ebenfalls)
        for o in self.shots:
            if o.dead or o.kind in ("mine", "strike", "hole", "swap", "ice", "nuke"):
                continue
            if math.hypot(o.x - s.x, o.y - s.y) < o.r + MINE_R:
                if o.kind == "frag":
                    self._explode(o)
                else:
                    o.dead = True
                return self._mine_boom(s)

    def _mine_boom(self, s):
        s.dead = True
        self._blast(s.x, s.y, MINE_BLAST, s.owner, "mine", 0.7)

    def _contact(self, s, t):
        """Geschoss s berührt Panzer t."""
        s.dead = True
        if s.kind == "rang" and t.slot == s.owner:  # gefangen -> wieder werfen
            if t.weapon is None:
                t.weapon, t.ammo = "rang", 1
        elif s.kind == "swap":
            self._swap(s, t)
        elif s.kind == "ice":
            if t.shield > 0:
                t.shield = 0.0
                self._fx("block", [round(t.x), round(t.y)], 0.6)
            else:
                t.eff["frozen"] = FREEZE_TIME
                self._fx("freeze", [round(t.x), round(t.y)], 0.5)
        else:
            self._hit(
                t,
                s.owner,
                s.kind,
                angle=math.atan2(s.vy, s.vx),
                bounces=s.bounces,
            )

    def _expire(self, s):
        if s.kind == "frag":
            return self._explode(s)
        if s.kind == "nuke":
            return self._nuke(s)
        s.dead = True
        if s.kind == "rocket":  # Treibstoff alle -> kleine Explosion
            self._fx("boom", [round(s.x), round(s.y), 0.5], 0.4)
        elif s.kind == "strike":
            self._blast(s.x, s.y, STRIKE_BLAST, s.owner, "strike", 1.0)
        elif s.kind == "hole":
            self._fx("boom", [round(s.x), round(s.y), 0.4], 0.35)

    def _give(self, t, kind, mystery=False):
        """Power-up an Panzer t. False, wenn er es nicht nehmen kann (hat schon eine Waffe)."""
        if kind == "mystery":
            roll = random.random()
            if roll < POTATO_CHANCE:
                kind = "potato"
            elif roll < POTATO_CHANCE + NUKE_CHANCE:
                kind = "nuke"
            else:
                kind = random.choice(WEAPONS + BUFFS)
            mystery = True
        if kind in ALL_WEAPONS:
            if t.weapon is not None and not mystery:
                return False
            t.weapon, t.ammo = kind, AMMO.get(kind, 1)
        elif kind == "shield":
            t.shield = SHIELD_TIME
        elif kind == "confuse":  # trifft alle anderen
            for o in self.tanks:
                if o.alive and o is not t:
                    o.eff["confuse"] = EFFECT_TIME["confuse"]
        elif kind == "potato":
            t.eff["potato"] = POTATO_TIME
            t.eff["_nopass"] = 0.5
        else:
            t.eff[kind] = EFFECT_TIME[kind]
        self._fx("pick", [round(t.x), round(t.y), kind, int(mystery)], 1.3)
        return True

    def _pass_potato(self, t1, t2):
        for a, b in ((t1, t2), (t2, t1)):
            if "potato" in a.eff and "potato" not in b.eff and "_nopass" not in a.eff:
                b.eff["potato"] = max(a.eff.pop("potato"), POTATO_MIN)
                b.eff["_nopass"] = 0.8  # nicht sofort zurückgeben
                self._fx("pick", [round(b.x), round(b.y), "potato", 0], 1.3)
                return

    def _spawn_box(self):
        cells = [(r, c) for r in range(self.arena.rows) for c in range(self.arena.cols)]
        random.shuffle(cells)
        for r, c in cells:
            x, y = self.arena.center(r, c)
            if any(t.alive and math.hypot(t.x - x, t.y - y) < 110 for t in self.tanks):
                continue
            if any(math.hypot(b[0] - x, b[1] - y) < 1 for b in self.boxes):
                continue
            self.boxes.append([x, y, random.choice(POWERUPS + ["mystery"])])
            return

    def step(self, dt):
        self.time += dt
        if self.slow > 0:
            self.slow = max(0.0, self.slow - dt)
        self.ts = self._time_scale()
        gdt = dt * self.ts  # Spielzeit: in der Zeitlupe langsamer, Panzer fahren normal
        for f in self.feed:
            f[2] += dt
        self.feed = [f for f in self.feed if f[2] < FEED_TIME]
        for t in self.tanks:
            if not t.alive:
                t.dead_age += dt
                continue
            t.shield = max(0.0, t.shield - gdt)
            t.taunt_time = max(0.0, t.taunt_time - dt)
            for k in list(t.eff):
                if k not in t.eff:  # Panzer ist gerade explodiert
                    continue
                t.eff[k] -= gdt
                if t.eff[k] > 0:
                    continue
                del t.eff[k]
                if k in ("ghost", "mini"):  # aus der Wand raus bzw. wieder groß
                    t.x, t.y = self._settle(t, t.x, t.y)
                elif k == "potato":
                    self._blast(
                        t.x, t.y, POTATO_BLAST, t.slot, "potato", 0.85, pierce=True
                    )

        # Panzer gegeneinander
        for t1, t2 in itertools.combinations(self.tanks, 2):
            if not (t1.alive and t2.alive):
                continue
            r = tank_radius(t1.eff) + tank_radius(t2.eff)
            dx, dy = t2.x - t1.x, t2.y - t1.y
            d = math.hypot(dx, dy)
            if d < r + 3:
                self._pass_potato(t1, t2)
            if 1e-6 < d < r:
                p = (r - d) / 2
                t1.x, t1.y = self._settle(t1, t1.x - dx / d * p, t1.y - dy / d * p)
                t2.x, t2.y = self._settle(t2, t2.x + dx / d * p, t2.y + dy / d * p)

        # Power-up-Kisten
        rain = self.mut == "kisten"
        self.box_timer -= dt
        if self.box_timer <= 0:
            self.box_timer = random.uniform(*((1.0, 2.0) if rain else BOX_EVERY))
            self.box_timer *= BOX_MAX / self.box_max
            if (
                len(self.boxes) < (self.box_max + 5 if rain else self.box_max)
                and self.end_timer is None
            ):
                self._spawn_box()
        for b in self.boxes[:]:
            for t in self.tanks:
                if (
                    not t.alive
                    or math.hypot(t.x - b[0], t.y - b[1]) > tank_radius(t.eff) + 12
                ):
                    continue
                if self._give(t, b[2]):
                    self.boxes.remove(b)
                    break

        # Geschosse
        for s in self.shots[:]:
            if s.dead:
                continue
            s.age += gdt
            if s.age > s.life:
                self._expire(s)
                continue
            k = s.kind
            if k == "mine":
                self._mine_check(s)
                continue
            if k == "strike":
                continue
            if k == "hole" and s.age > HOLE_TRAVEL:
                self._hole_pull(s, gdt)
                continue
            if k == "rocket" and s.age > ROCKET_DELAY:
                self._rocket_steer(s, gdt)
            elif k == "rang":
                self._rang_steer(s, gdt)
            elif k in ("fl", "nuke"):  # Flammen und die rollende Bombe werden langsamer
                f = max(0.0, 1 - (FLAME_DRAG if k == "fl" else NUKE_DRAG) * gdt)
                s.vx, s.vy, s.speed = s.vx * f, s.vy * f, s.speed * f
            bounces = s.travel(gdt, self.arena)
            s.bounces += bounces
            if k == "bnc" and bounces:  # Flummi wird mit jedem Abpraller schneller
                s.speed = min(s.speed * BOUNCY_GAIN**bounces, BOUNCY_MAX)
                a = math.atan2(s.vy, s.vx)
                s.vx, s.vy = math.cos(a) * s.speed, math.sin(a) * s.speed
            if k in (
                "hole",
                "nuke",
            ):  # unterwegs trifft das Loch niemanden, die Bombe rollt
                continue
            for t in self.tanks:
                if not t.alive or (t.slot == s.owner and s.age < s.grace):
                    continue
                if math.hypot(t.x - s.x, t.y - s.y) < tank_radius(t.eff) + s.r - 1:
                    self._contact(s, t)
                    break
        self.shots = [s for s in self.shots if not s.dead]

        for f in self.fx[:]:
            f.age += gdt
            if f.age > f.life:
                self.fx.remove(f)

        # Rundenende: sobald es entschieden ist, kurz Zeitlupe
        alive = [t.slot for t in self.tanks if t.alive]
        if self.end_timer is None and len(alive) <= 1:
            self.end_timer = ROUND_END_DELAY
            self._slowmo(SLOWMO_TIME)
        if self.end_timer is not None:
            self.end_timer -= dt
            if self.end_timer < ROUND_END_DELAY - 1:
                if alive:
                    w = alive[0]
                    n = self.streak[w] + 1 if self.last_winner == w else 1
                    self.msg = f"{NAMES[w]} gewinnt die Runde!"
                    self.win = [w, streak_text(n)]
                else:
                    self.msg, self.win = "Unentschieden!", [None, ""]
            if self.end_timer <= 0:
                if len(alive) == 1:
                    w = alive[0]
                    self.scores[w] += 1
                    self.streak[w] = self.streak[w] + 1 if self.last_winner == w else 1
                    self.last_winner = w
                else:
                    self.last_winner = None
                self.new_round()

    def reload_state(self, t):
        """(verfügbare Schuss, Sekunden bis wieder eine Kugel frei ist)."""
        if t.weapon in AMMO:
            return t.ammo, 0.0
        most = bullet_params(self.mut)[3]
        left = [
            s.life - s.age for s in self.shots if s.owner == t.slot and s.kind == "b"
        ]
        free = most - len(left)
        if free > 0:
            return free, 0.0
        # alle Kugeln unterwegs -> warten, bis die älteste verschwindet
        return 0, max(0.0, min(left))

    def snapshot(self, ack=None):
        return {
            "t": "s",
            "st": round(self.time, 4),
            "ack": ack,
            "m": self.maze_id,
            "tk": [self._tank_state(t) for t in self.tanks],
            "p": [self._shot_state(s) for s in self.shots],
            "bx": [[round(b[0]), round(b[1]), b[2]] for b in self.boxes],
            "fx": [[f.id, f.kind, round(f.age, 2), f.life, f.data] for f in self.fx],
            "kf": [
                [txt, s, round(age, 1), v, how] for txt, s, age, v, how in self.feed
            ],
            "sc": self.scores,
            "msg": self.msg,
            "win": self.win,
            "ts": round(self.ts, 3),
            "et": None if self.end_timer is None else round(self.end_timer, 2),
            "bots": self.bots,
        }

    def _tank_state(self, t):
        # [x, y, a, alive, dead_age, weapon, shield, slot, ammo, reload, effekte, spruch]
        ammo, reload = self.reload_state(t)
        return [
            round(t.x, 1),
            round(t.y, 1),
            round(t.a, 3),
            t.alive,
            round(t.dead_age, 2),
            t.weapon,
            round(t.shield, 1),
            t.slot,
            ammo,
            round(reload, 2),
            {k: round(v, 1) for k, v in t.eff.items() if k[0] != "_"},
            t.taunt if t.taunt_time > 0 else 0,
        ]

    @staticmethod
    def _shot_state(s):
        # [id, art, x, y, besitzer] + Rakete: Winkel, sonst bei Bedarf das Alter
        p = [s.id, s.kind, round(s.x, 1), round(s.y, 1), s.owner]
        if s.kind == "rocket":
            p.append(round(math.atan2(s.vy, s.vx), 3))
        elif s.kind in AGED:
            p.append(round(s.age, 2))
        return p


# --- Netzwerk ----------------------------------------------------------------


def send_msg(sock, obj):
    sock.sendall((json.dumps(obj, separators=(",", ":")) + "\n").encode())


def read_lines(sock, on_msg):
    """Blockierende Empfangsschleife für JSON-Zeilen. Endet bei Verbindungsabbruch."""
    buf = b""
    try:
        while True:
            data = sock.recv(65536)
            if not data:
                return
            buf += data
            *lines, buf = buf.split(b"\n")
            for line in lines:
                on_msg(json.loads(line))
    except (OSError, ValueError):
        return


def close_sock(sock):
    # shutdown weckt auch einen blockierten recv() im Empfangs-Thread auf
    try:
        sock.shutdown(socket.SHUT_RDWR)
    except OSError:
        pass
    sock.close()


def local_ip():
    """IP des Interfaces mit Default-Route (es wird nichts gesendet)."""
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            s.connect(("8.8.8.8", 80))
            return s.getsockname()[0]
    except OSError:
        return "127.0.0.1"


class Peer:
    """Ein verbundener Client beim Host."""

    def __init__(self, conn):
        self.conn = conn
        self.cmds = deque()  # Input-Kommandos: [seq, keys, fires, dt]
        self.budget = 0.0  # Jitter-Puffer für die Input-Abarbeitung
        self.ack = None  # letzte abgearbeitete seq


class HostNet:
    def __init__(self, port):
        self.srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.srv.bind(("", port))
        self.srv.listen(MAX_PLAYERS)
        self.lock = threading.Lock()
        self.peers = {}  # slot (1..MAX_PLAYERS-1) -> Peer
        self.gen = (
            0  # zählt bei jedem Join/Leave hoch -> Host passt die Spielerliste an
        )
        threading.Thread(target=self._accept_loop, daemon=True).start()

    def _accept_loop(self):
        while True:
            try:
                conn, _ = self.srv.accept()
            except OSError:
                return
            with self.lock:
                free = [s for s in range(1, MAX_PLAYERS) if s not in self.peers]
            if not free:
                try:
                    send_msg(conn, {"t": "full"})
                except OSError:
                    pass
                close_sock(conn)
                continue
            conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            peer = Peer(conn)
            try:
                send_msg(conn, {"t": "hello", "v": PROTO, "slot": free[0]})
            except OSError:
                close_sock(conn)
                continue
            with self.lock:
                self.peers[free[0]] = peer
                self.gen += 1
            threading.Thread(target=self._recv_loop, args=(peer,), daemon=True).start()

    def _recv_loop(self, peer):
        def on_msg(msg):
            if msg.get("t") == "i":
                peer.cmds.extend(msg["c"])

        read_lines(peer.conn, on_msg)
        self._drop(peer)

    def _drop(self, peer):
        with self.lock:
            for slot, p in list(self.peers.items()):
                if p is peer:
                    del self.peers[slot]
                    self.gen += 1
        close_sock(peer.conn)

    def clients(self):
        with self.lock:
            return dict(self.peers)

    def send(self, peer, obj):
        try:
            send_msg(peer.conn, obj)
        except OSError:
            self._drop(peer)

    def broadcast(self, obj):
        for peer in self.clients().values():
            self.send(peer, obj)

    def close(self):
        close_sock(self.srv)
        for peer in self.clients().values():
            self._drop(peer)


class ClientNet:
    def __init__(self, host, port):
        self.sock = socket.create_connection((host, port), timeout=5)
        self.sock.settimeout(None)
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.arena = None
        self.slot = None  # eigene Spieler-Nummer, kommt mit "hello"
        self.snaps = deque(maxlen=40)
        self.offset = None  # Serverzeit - lokale Zeit
        self.connected = True
        self.error = None
        threading.Thread(target=self._recv_loop, daemon=True).start()

    def _recv_loop(self):
        def on_msg(msg):
            t = msg.get("t")
            if t == "full":
                self.error = f"Das Spiel ist schon voll ({MAX_PLAYERS} Spieler)."
            elif t == "hello":
                if msg.get("v") != PROTO:
                    self.error = "Host hat eine andere Spielversion."
                self.slot = msg["slot"]
            elif self.slot is None:  # alter Host ohne Handshake
                self.error = "Host hat eine andere Spielversion."
            elif t == "m":
                self.arena = Arena(msg["maze"])
            elif t == "s":
                off = msg["st"] - time.perf_counter()
                # Früh ankommende Pakete bestimmen den Offset, langsam nachdriften
                if self.offset is None or off > self.offset:
                    self.offset = off
                else:
                    self.offset += (off - self.offset) * 0.02
                self.snaps.append(msg)

        read_lines(self.sock, on_msg)
        self.connected = False

    def send(self, obj):
        try:
            send_msg(self.sock, obj)
        except OSError:
            self.connected = False

    def close(self):
        close_sock(self.sock)


def lerp_angle(a, b, k):
    return a + ((b - a + math.pi) % (2 * math.pi) - math.pi) * k


def interpolate(snaps, render_t):
    """Snapshot-ähnliches Dict zum Zeitpunkt render_t (Serverzeit)."""
    snaps = list(snaps)
    b = snaps[-1]
    if render_t >= b["st"] or len(snaps) == 1:
        return b
    a = None
    for i in range(len(snaps) - 1, 0, -1):
        if snaps[i - 1]["st"] <= render_t:
            a, b = snaps[i - 1], snaps[i]
            break
    if a is None or a["m"] != b["m"]:
        return b
    k = (render_t - a["st"]) / max(b["st"] - a["st"], 1e-6)
    tanks = []
    for ta, tb in zip(a["tk"], b["tk"]):
        tanks.append(
            [
                ta[0] + (tb[0] - ta[0]) * k,
                ta[1] + (tb[1] - ta[1]) * k,
                lerp_angle(ta[2], tb[2], k),
            ]
            + tb[3:]
        )
    old = {p[0]: p for p in a["p"]}
    shots = []
    for p in b["p"]:
        q = old.get(p[0])
        if not q:
            shots.append(p)
            continue
        s = [p[0], p[1], q[2] + (p[2] - q[2]) * k, q[3] + (p[3] - q[3]) * k, p[4]]
        if p[1] == "rocket":  # Rakete: Winkel
            s.append(lerp_angle(q[5], p[5], k))
        else:
            s += p[5:]
        shots.append(s)
    return dict(b, tk=tanks, p=shots)


# --- Computergegner -------------------------------------------------------------

BOT_PLAN = 0.3  # so oft planen Bots neu (Ziel, Weg, Zielwinkel)
BOT_RAYS = 18  # so viele Winkel probiert ein Bot für einen Bandenschuss
BOT_TAUNTS = (1, 4, 5)  # "Hehe!", "Zu langsam!", "Komm her!"


def angle_diff(a, b):
    """Kürzester Winkel von b nach a (-pi..pi)."""
    return (a - b + math.pi) % (2 * math.pi) - math.pi


class Bot:
    """Computergegner. Liefert jeden Tick (Tasten, Schuss-Zähler) wie ein Spieler an der
    Tastatur. Läuft nur beim Host bzw. lokal, die Clients sehen nur einen weiteren Panzer.
    """

    def __init__(self, slot, skill=None):
        self.slot = slot
        self.skill = random.uniform(0.55, 0.9) if skill is None else skill
        self.fires = 0
        self.round = None
        self.tick = slot  # versetzt, damit nicht alle Bots im selben Tick rechnen
        self._reset()

    def _reset(self):
        self.plan_in = random.uniform(0, BOT_PLAN)
        self.path, self.goal, self.target, self.aim = [], None, None, None
        self.lock = 0.0  # so lange passt der Schuss schon (Reaktionszeit)
        self.ok = False  # Ergebnis der letzten Trefferprüfung
        self.cool = 0.0  # Pause zwischen zwei Schüssen
        self.noticed = {}  # Geschoss-id -> bemerkt? (Bots übersehen manchmal etwas)
        self.stuck, self.escape, self.last = 0.0, 0.0, None
        self.held = 0.0  # so lange liegt die Waffe schon ungenutzt herum
        self.turn = random.choice((LEFT, RIGHT))
        self.taunt, self.taunt_t, self.kills, self.gg = 0, 0.0, 0, False

    def think(self, game):
        t = game.by_slot.get(self.slot)
        if game.maze_id != self.round:
            self.round = game.maze_id
            self._reset()
        if t is None or not t.alive:
            return 0, self.fires
        self.tick += 1
        foes = [
            o for o in game.tanks if o.alive and o is not t and "invis" not in o.eff
        ]
        self.plan_in -= TICK
        if self.plan_in <= 0:
            self.plan_in = BOT_PLAN
            self._plan(game, t, foes)
        keys = self._drive(game, t)
        press, hold = self._fire(game, t, foes)
        if hold:
            keys |= FIRE
        if press:
            self.fires += 1
        return keys | self._taunt(game) << TAUNT_SHIFT, self.fires

    # --- Planen ---

    def _plan(self, game, t, foes):
        arena = game.arena
        dist, prev = arena.bfs(arena.cell(t.x, t.y))

        def steps(x, y):
            return dist.get(arena.cell(x, y), 99)

        self.target = min(
            foes,
            key=lambda o: (steps(o.x, o.y), math.hypot(o.x - t.x, o.y - t.y)),
            default=None,
        )
        far = steps(self.target.x, self.target.y) if self.target else 99
        threat = self._threat(game, t)
        goal = None
        if threat:
            goal = self._flee(arena, dist, threat)
        elif (
            "potato" in t.eff and self.target
        ):  # Kartoffel loswerden: hin und anstupsen
            goal = (self.target.x, self.target.y)
        else:
            box = self._box(game, t, dist)
            if box and box[0] <= far + 1:
                goal = box[1]
            elif self.target:
                if far > 1 or not arena.clear(t.x, t.y, self.target.x, self.target.y):
                    goal = (self.target.x, self.target.y)
            elif self.goal is None or random.random() < 0.08:  # niemand zu sehen
                goal = arena.center(*random.choice(list(dist)))
            else:
                goal = self.goal
        self.goal, self.path = goal, []
        if goal:
            cell = arena.cell(*goal)
            while cell is not None and cell in prev:
                self.path.append(cell)
                cell = prev[cell]
            self.path.reverse()
        self.aim = None
        if self.target and not threat:
            self.aim = self._find_aim(game, t, self.target)

    def _threat(self, game, t):
        """Gefahr in der Nähe (x, y, Radius), vor der man besser wegfährt."""
        for s in game.shots:
            if s.kind == "nuke":
                r = NUKE_BLAST + 50
            elif s.kind == "strike":
                r = STRIKE_BLAST + 30
            elif s.kind == "hole":
                r = HOLE_RANGE * 0.9
            elif s.kind == "frag" and s.owner != self.slot:
                r = 100
            else:
                continue
            if math.hypot(s.x - t.x, s.y - t.y) < r:
                return s.x, s.y, r
        if "potato" not in t.eff:
            for o in game.tanks:
                if (
                    o.alive
                    and "potato" in o.eff
                    and math.hypot(o.x - t.x, o.y - t.y) < 170
                ):
                    return o.x, o.y, 170
        return None

    @staticmethod
    def _flee(arena, dist, threat):
        """Zelle in der Nähe, die möglichst weit von der Gefahr weg liegt."""
        x, y, _ = threat

        def score(c):
            cx, cy = arena.center(*c)
            return math.hypot(cx - x, cy - y) - dist[c] * 10

        return arena.center(*max((c for c, d in dist.items() if d <= 6), key=score))

    @staticmethod
    def _box(game, t, dist):
        """Nächste Kiste, die der Bot nehmen kann: (Schritte, (x, y)) oder None."""
        best = None
        for x, y, kind in game.boxes:
            if t.weapon and kind in WEAPONS:
                continue  # mit Waffe bleiben Waffenkisten liegen
            d = dist.get(game.arena.cell(x, y))
            if d is not None and d <= 6 and (best is None or d < best[0]):
                best = (d, (x, y))
        return best

    def _find_aim(self, game, t, target):
        """Winkel, mit dem ein Schuss target trifft: direkt oder über die Bande (oder None)."""
        arena = game.arena
        if arena.clear(t.x, t.y, target.x, target.y):
            return math.atan2(target.y - t.y, target.x - t.x)
        if math.hypot(target.x - t.x, target.y - t.y) > 460 or t.weapon in (
            "flame",
            "zap",
            "shotgun",
            "mine",
            "strike",
            "nuke",
            "rocket",
            "mg",
        ):
            return None
        circles = [
            (target.x, target.y, tank_radius(target.eff)),
            (t.x, t.y, tank_radius(t.eff) + 2),  # nicht sich selbst treffen
        ]
        best = None
        base = random.uniform(0, 2 * math.pi)
        for k in range(BOT_RAYS):
            a = base + k * 2 * math.pi / BOT_RAYS
            pts, hit = arena.raycast(t.x, t.y, a, 560, circles)
            if hit == 0 and len(pts) <= 4:
                d = abs(angle_diff(a, t.a))
                if best is None or d < best[0]:
                    best = (d, a)
        return best[1] if best else None

    # --- Fahren ---

    def _drive(self, game, t):
        if "frozen" in t.eff:
            return 0
        danger = self._danger(game, t)
        if danger:
            return self._dodge(t, danger)
        if self.escape > 0:  # festgefahren: kurz zurücksetzen und drehen
            self.escape -= TICK
            return DOWN | self.turn
        keys = 0
        ready = t.weapon is not None or game.reload_state(t)[0] > 0
        if self.aim is not None and ready:
            diff = angle_diff(self.aim, t.a)
            if abs(diff) > 0.05:
                keys |= RIGHT if diff > 0 else LEFT
            close = self.target and math.hypot(self.target.x - t.x, self.target.y - t.y)
            if abs(diff) < 0.5 and self.goal and close and close > 150:
                keys |= UP
        else:
            wp = self._waypoint(game.arena, t)
            if wp:
                diff = angle_diff(math.atan2(wp[1] - t.y, wp[0] - t.x), t.a)
                if abs(diff) > 0.08:
                    keys |= RIGHT if diff > 0 else LEFT
                if abs(diff) < 0.8:
                    keys |= UP
        if keys & UP:  # steckt er fest?
            if self.last and math.hypot(t.x - self.last[0], t.y - self.last[1]) < 0.6:
                self.stuck += TICK
                if self.stuck > 0.35:
                    self.stuck, self.escape = 0.0, 0.35
                    self.turn = random.choice((LEFT, RIGHT))
            else:
                self.stuck = 0.0
        self.last = (t.x, t.y)
        return keys

    def _waypoint(self, arena, t):
        """Nächster Punkt auf dem Weg: Mitte der nächsten Zelle bzw. das Ziel selbst."""
        if not self.path:
            return self.goal
        cur = arena.cell(t.x, t.y)
        if cur not in self.path:
            return arena.center(*self.path[0])
        i = self.path.index(cur)
        if i + 1 >= len(self.path):
            return self.goal
        return arena.center(*self.path[i + 1])

    def _danger(self, game, t):
        """Geschoss, das in den nächsten 0,6 s nah vorbeikommt: (Zeit, Abstand x/y, vx, vy)."""
        r = tank_radius(t.eff)
        worst = None
        for s in game.shots:
            k = s.kind
            if k in ("mine", "strike", "hole", "nuke", "swap"):
                continue
            if s.owner == self.slot and (s.age < s.grace or k == "rang"):
                continue
            vv = s.vx * s.vx + s.vy * s.vy
            if vv < 100:
                continue
            rx, ry = t.x - s.x, t.y - s.y
            when = (rx * s.vx + ry * s.vy) / vv
            if not 0 < when < 0.6:
                continue
            mx, my = rx - s.vx * when, ry - s.vy * when  # vom Vorbeiflug-Punkt zum Bot
            if mx * mx + my * my > (r + s.r + 8) ** 2:
                continue
            if s.id not in self.noticed:
                self.noticed[s.id] = random.random() < self.skill
            if self.noticed[s.id] and (worst is None or when < worst[0]):
                worst = (when, mx, my, s.vx, s.vy)
        return worst

    @staticmethod
    def _dodge(t, danger):
        """Quer zur Flugbahn ausweichen (vorwärts oder rückwärts, je nachdem was näher liegt)."""
        _, mx, my, vx, vy = danger
        d = math.hypot(mx, my)
        if d < 1:  # kommt genau drauf zu: irgendeine Seite
            mx, my, d = -vy, vx, math.hypot(vx, vy)
        ex, ey = mx / d, my / d
        fwd = math.cos(t.a) * ex + math.sin(t.a) * ey
        keys = UP if fwd > 0 else DOWN
        if abs(fwd) < 0.7:  # steht quer -> beim Fahren eindrehen
            side = math.cos(t.a) * ey - math.sin(t.a) * ex
            keys |= RIGHT if (side > 0) == (fwd > 0) else LEFT
        return keys

    # --- Schießen ---

    def _fire(self, game, t, foes):
        """(einmal drücken, Feuer halten)."""
        self.cool = max(0.0, self.cool - TICK)
        if "frozen" in t.eff or game._in_wall(t):
            return False, False
        own = [s for s in game.shots if s.owner == self.slot]
        frag = next((s for s in own if s.kind == "frag"), None)
        if frag:  # Splitterbombe zünden: Gegner nah dran, man selbst weit genug weg
            near = min(
                (math.hypot(o.x - frag.x, o.y - frag.y) for o in foes), default=999
            )
            return near < 75 and math.hypot(t.x - frag.x, t.y - frag.y) > 120, False
        if not foes or any(s.kind == "swap" for s in own):
            return False, False  # ein zweiter Druck würde zur Tauschkugel springen
        w = t.weapon
        self.held = self.held + TICK if w else 0.0
        if w in ("mg", "flame"):
            reach = 130 if w == "flame" else 420
            return False, self._facing(game, t, foes, reach, 0.22)
        if self.cool > 0:
            return False, False
        if self.tick % 3 == 0:  # die teuren Prüfungen nur jeden 3. Tick
            self.ok = self._should_fire(game, t, foes, w)
        self.lock = self.lock + TICK if self.ok else 0.0
        if self.lock < 0.22 - 0.16 * self.skill:  # Reaktionszeit
            return False, False
        self.lock, self.cool, self.ok = 0.0, 0.2, False
        return True, False

    def _should_fire(self, game, t, foes, w):
        arena = game.arena
        if w is None:
            return game.reload_state(t)[0] > 0 and self._lined_up(game, t, 620)
        if w == "laser":
            tanks = [o for o in game.tanks if o.alive]
            circles = [(o.x, o.y, tank_radius(o.eff)) for o in tanks]
            hit = arena.raycast(t.x, t.y, t.a, LASER_LEN, circles)[1]
            return hit is not None and tanks[hit] in foes
        if w == "rocket":
            return self.target is not None and first_wall(arena, t.x, t.y, t.a, 90) > 70
        if w == "zap":
            return self._facing(game, t, foes, ZAP_RANGE - 20, ZAP_CONE * 0.8)
        if w == "nuke":
            return any(math.hypot(o.x - t.x, o.y - t.y) < 320 for o in foes)
        if w == "strike":
            x, y = strike_target(arena, t.x, t.y, t.a)
            return math.hypot(x - t.x, y - t.y) > STRIKE_BLAST + 25 and any(
                math.hypot(o.x - x, o.y - y) < STRIKE_BLAST * 0.8 for o in foes
            )
        if w == "mine":
            return random.random() < 0.03
        if w == "hole":
            return self._facing(game, t, foes, 330, 0.3) and not self._facing(
                game, t, foes, 120, math.pi
            )
        if w == "shotgun":
            return self._lined_up(game, t, 240)
        if self.held > 9 and self._facing(game, t, foes, 600, 0.6):
            return True  # Waffe nicht ewig horten
        return self._lined_up(game, t, 560)  # Splitterbombe, Flummi, Tauschkugel ...

    @staticmethod
    def _lined_up(game, t, length):
        """Trifft ein Schuss in Blickrichtung (auch über die Bande) einen Gegner und nicht
        zuerst einen selbst? Getarnte Panzer zählen nicht, die sieht der Bot ja nicht.
        """
        tanks = [o for o in game.tanks if o is t or o.alive and "invis" not in o.eff]
        circles = [(o.x, o.y, tank_radius(o.eff) + 1) for o in tanks]
        hit = game.arena.raycast(t.x, t.y, t.a, length, circles)[1]
        return hit is not None and tanks[hit] is not t

    @staticmethod
    def _facing(game, t, foes, reach, cone):
        """Steht ein Gegner in Reichweite ungefähr vor dem Rohr (mit freier Sicht)?"""
        for o in foes:
            d = math.hypot(o.x - t.x, o.y - t.y)
            if (
                d < reach
                and abs(angle_diff(math.atan2(o.y - t.y, o.x - t.x), t.a)) < cone
                and game.arena.clear(t.x, t.y, o.x, o.y)
            ):
                return True
        return False

    # --- Sprüche ---

    def _taunt(self, game):
        """Ab und zu ein Spruch: nach einem Abschuss oder nach dem Rundensieg."""
        kills = len(game.kills.get(self.slot, ()))
        if kills > self.kills and random.random() < 0.5:
            self.taunt, self.taunt_t = random.choice(BOT_TAUNTS), 0.3
        self.kills = kills
        if game.win and game.win[0] == self.slot and not self.gg:
            self.gg, self.taunt, self.taunt_t = True, 3, 0.3  # "GG"
        self.taunt_t -= TICK
        return self.taunt if self.taunt_t > 0 else 0


# --- Grafik ------------------------------------------------------------------


def mix(c1, c2, k):
    """Farbe zwischen c1 und c2 (k = 0..1)."""
    return tuple(int(a + (b - a) * k) for a, b in zip(c1, c2))


def shade(c, k):
    """Farbe heller (k > 1) oder dunkler (k < 1)."""
    return tuple(max(0, min(255, int(v * k))) for v in c)


def ease_out(k):
    k = min(1.0, max(0.0, k))
    return 1 - (1 - k) ** 3


def ease_back(k):
    """Schießt etwas über das Ziel hinaus und federt zurück (für Pop-Animationen)."""
    k = min(1.0, max(0.0, k)) - 1
    return 1 + 2.7 * k**3 + 1.7 * k**2


def light_r(r):
    """Radius auf Stufen runden, damit die Licht-Caches klein bleiben."""
    r = max(2, int(r))
    step = 2 if r < 24 else 6 if r < 80 else 20
    return r - r % step + step


def rot_rect(x, y, w, h, a):
    """Eckpunkte eines um a gedrehten Rechtecks (Mitte x, y)."""
    c, s = math.cos(a) * 0.5, math.sin(a) * 0.5
    return [
        (x + c * w - s * h, y + s * w + c * h),
        (x - c * w - s * h, y - s * w + c * h),
        (x - c * w + s * h, y - s * w - c * h),
        (x + c * w + s * h, y + s * w - c * h),
    ]


@functools.cache
def _falloff(r):
    """Weißer, weich auslaufender Kreis auf Schwarz – Grundlage für alle Lichter."""
    s = pygame.Surface((2 * r, 2 * r))
    for q in range(r, 0, -1):
        v = int(255 * (1 - q / r) ** 2)
        pygame.draw.circle(s, (v, v, v), (r, r), q)
    return s.convert()


@functools.lru_cache(maxsize=1500)
def glow(color, r, level=8):
    """Licht zum additiven Blenden (BLEND_ADD). level 1..8 = Helligkeit in Stufen."""
    s = _falloff(r).copy()
    s.fill(shade(color, level / 8), special_flags=pygame.BLEND_RGB_MULT)
    return s


@functools.lru_cache(maxsize=1500)
def puff(r, alpha, color=(150, 150, 150)):
    """Weiche Wolke (Rauch, Staub) für normales Alpha-Blending."""
    s = pygame.Surface((2 * r + 2, 2 * r + 2), pygame.SRCALPHA)
    for i in range(5):
        q = max(1, round(r * (1 - i / 5)))
        pygame.draw.circle(s, (*color, int(alpha * (i + 1) / 5)), (r + 1, r + 1), q)
    return s.convert_alpha()


@functools.cache
def shadow_sprite(r):
    """Weicher Schatten unter Panzern und Kisten."""
    s = pygame.Surface((4 * r, 4 * r), pygame.SRCALPHA)
    pygame.draw.circle(s, (0, 0, 0, 125), (2 * r, 2 * r), r)
    return pygame.transform.gaussian_blur(s, max(1, r // 3)).convert_alpha()


@functools.lru_cache(maxsize=64)
def scorch_sprite(r):
    """Brandfleck für den Boden."""
    s = pygame.Surface((2 * r, 2 * r), pygame.SRCALPHA)
    rnd = random.Random(r)
    for i in range(6):
        q = r * (1 - i / 7) * 0.8
        pos = (r + rnd.uniform(-2, 2), r + rnd.uniform(-2, 2))
        pygame.draw.circle(s, (14, 11, 10, 26 + 22 * i), pos, q)
    for _ in range(int(r * 0.7)):  # Spritzer
        a, d = rnd.uniform(0, 2 * math.pi), rnd.uniform(0.45, 0.95) * r
        pos = (r + math.cos(a) * d, r + math.sin(a) * d)
        pygame.draw.circle(s, (16, 13, 12, 80), pos, rnd.uniform(1, max(1.5, r * 0.1)))
    return s.convert_alpha()


@functools.lru_cache(maxsize=4)
def vignette(w, h, strength=0.5):
    """Abdunklung zu den Rändern (wird mit BLEND_RGB_MULT aufgelegt)."""
    small = pygame.Surface((48, 30))
    for y in range(30):
        for x in range(48):
            d = min(1.0, math.hypot((x - 23.5) / 24, (y - 14.5) / 15) / 1.3)
            v = int(255 * (1 - strength * d**2.4))
            small.set_at((x, y), (v, v, v))
    return pygame.transform.smoothscale(small, (w, h)).convert()


@functools.cache
def edge_light():
    """Helle Ränder, dunkle Mitte: wird eingefärbt und additiv aufgelegt (Gefahr am Rand)."""
    inv = pygame.Surface((W, VIEW_H)).convert()
    inv.fill((255, 255, 255))
    inv.blit(vignette(W, VIEW_H, 1.0), (0, 0), special_flags=pygame.BLEND_RGB_SUB)
    return inv


TREAD_C, TREAD_HI = (34, 36, 42), (72, 76, 86)


def make_tank_sprite(color, wreck=False):
    """Panzer von oben, Rohr nach +x. 4-fach überabgetastet -> glatte Kanten."""
    ss, n = 4, 44
    s = pygame.Surface((n * ss, n * ss), pygame.SRCALPHA)

    def r(x, y, w, h):
        return pygame.Rect(round(x * ss), round(y * ss), round(w * ss), round(h * ss))

    def at(x, y):
        return round(x * ss), round(y * ss)

    if wreck:
        color = mix(color, (34, 28, 26), 0.74)
    dark, light = shade(color, 0.55), mix(color, (255, 255, 255), 0.42)
    tread, tread_hi = ((26, 24, 24), (46, 42, 40)) if wreck else (TREAD_C, TREAD_HI)
    for y in (5, 30):  # Ketten mit Gliedern
        pygame.draw.rect(s, (16, 17, 20), r(3.5, y - 0.5, 37, 10), border_radius=4 * ss)
        pygame.draw.rect(s, tread, r(4, y, 36, 9), border_radius=3 * ss)
        for x in range(6, 39, 4):
            pygame.draw.rect(s, tread_hi, r(x, y + 1.5, 1.6, 6))
    pygame.draw.rect(s, (16, 17, 20), r(5.5, 8.5, 33, 27), border_radius=5 * ss)
    pygame.draw.rect(s, dark, r(6, 9, 32, 26), border_radius=4 * ss)  # Wanne
    pygame.draw.rect(s, color, r(7.5, 10.5, 29, 22.5), border_radius=3 * ss)
    pygame.draw.rect(s, light, r(9, 11.5, 26, 3.6), border_radius=2 * ss)  # Lichtkante
    pygame.draw.rect(s, shade(color, 0.78), r(8.5, 28.5, 27, 4), border_radius=2 * ss)
    for y in (14.5, 26.5):  # Lüftungsgitter am Heck
        pygame.draw.rect(s, dark, r(9.5, y, 5, 1.5))
    barrel = (40, 36, 34) if wreck else (60, 63, 72)
    pygame.draw.rect(s, (16, 17, 20), r(20.5, 19, 22, 6), border_radius=ss)
    pygame.draw.rect(s, barrel, r(21, 19.5, 21, 5), border_radius=ss)  # Rohr
    pygame.draw.rect(s, mix(barrel, (255, 255, 255), 0.35), r(22, 20.3, 19, 1.3))
    pygame.draw.rect(s, shade(barrel, 0.7), r(38.5, 18.3, 4.5, 7.4), border_radius=ss)
    pygame.draw.circle(s, (16, 17, 20), at(22, 22), 10.6 * ss)  # Turm
    pygame.draw.circle(s, dark, at(22, 22), 10 * ss)
    pygame.draw.circle(s, color, at(22, 22), 8.4 * ss)
    pygame.draw.circle(s, light, at(20.6, 20.4), 4.8 * ss)
    pygame.draw.circle(s, color, at(21.6, 21.6), 4.9 * ss)
    pygame.draw.circle(s, dark, at(18.4, 24.6), 2.3 * ss)  # Luke
    pygame.draw.circle(s, shade(color, 1.15), at(18.0, 24.0), 1.0 * ss)
    if wreck:  # Ruß und Brandlöcher
        rnd = random.Random(sum(color))
        for _ in range(10):
            pos = at(rnd.uniform(8, 36), rnd.uniform(9, 35))
            pygame.draw.circle(s, (18, 16, 16, 175), pos, rnd.uniform(1.6, 4.8) * ss)
    return pygame.transform.smoothscale(s, (n, n)).convert_alpha()


def make_turret_sprite(color):
    """Abgesprengter Turm mit Rohr (fliegt beim Abschuss davon)."""
    ss, w, h = 4, 34, 22
    s = pygame.Surface((w * ss, h * ss), pygame.SRCALPHA)
    cx, cy = 11, 11
    dark, light = shade(color, 0.55), mix(color, (255, 255, 255), 0.35)
    pygame.draw.rect(s, (40, 38, 38), ((cx - 1) * ss, (cy - 2.5) * ss, 22 * ss, 5 * ss))
    pygame.draw.circle(s, (16, 17, 20), (cx * ss, cy * ss), 10.6 * ss)
    pygame.draw.circle(s, dark, (cx * ss, cy * ss), 10 * ss)
    pygame.draw.circle(s, color, (cx * ss, cy * ss), 8.4 * ss)
    pygame.draw.circle(s, light, ((cx - 1.4) * ss, (cy - 1.6) * ss), 4.8 * ss)
    pygame.draw.circle(s, color, ((cx - 0.4) * ss, (cy - 0.4) * ss), 4.9 * ss)
    return pygame.transform.smoothscale(s, (w, h)).convert_alpha()


def make_jet():
    """Kampfjet von oben (Nase nach +x) für den Luftschlag, 4-fach überabgetastet."""
    ss, w, h = 4, 66, 58

    def tf(pts):
        return [((x + w / 2) * ss, (y + h / 2) * ss) for x, y in pts]

    s = pygame.Surface((w * ss, h * ss), pygame.SRCALPHA)
    body, dark, light = (128, 136, 150), (74, 80, 94), (196, 204, 216)
    wings = [
        (8, -3),
        (-8, -26),
        (-15, -26),
        (-9, -3),
        (-9, 3),
        (-15, 26),
        (-8, 26),
        (8, 3),
    ]
    tail = [
        (-21, -2),
        (-29, -12),
        (-32, -12),
        (-28, -2),
        (-28, 2),
        (-32, 12),
        (-29, 12),
        (-21, 2),
    ]
    hull = [(32, 0), (23, -3.6), (-27, -4), (-31, -2), (-31, 2), (-27, 4), (23, 3.6)]
    for part, col in ((wings, dark), (tail, dark), (hull, body)):
        pygame.draw.polygon(s, col, tf(part))
        pygame.draw.polygon(s, (30, 32, 38), tf(part), ss)
    pygame.draw.polygon(s, light, tf([(23, -3.2), (-26, -3.6), (-26, -1), (23, -0.6)]))
    pygame.draw.polygon(
        s, (70, 150, 210), tf([(19, 0), (13, -2), (6, -2), (4, 0), (6, 2), (13, 2)])
    )
    pygame.draw.polygon(
        s, (220, 40, 40), tf([(-23, -2), (-29, -10), (-31, -10), (-27, -2)])
    )
    pygame.draw.polygon(
        s, (220, 40, 40), tf([(-23, 2), (-29, 10), (-31, 10), (-27, 2)])
    )
    return pygame.transform.smoothscale(s, (w, h)).convert_alpha()


LIGHT_SCALE = (
    4  # Lichtkarte beim Stromausfall in Viertel-Auflösung (wird weich hochskaliert)
)


@functools.lru_cache(maxsize=80)
def cone_sprite(step):
    """Lichtkegel eines Scheinwerfers (Viertel-Auflösung, Spitze in der Mitte, nach +x).
    step = 6°-Schritte."""
    if step:
        return pygame.transform.rotate(cone_sprite(0), -step * 6)
    n, half = 78, 0.52  # Länge, halber Öffnungswinkel
    s = pygame.Surface((2 * n, 2 * n))
    for i in range(n, 3, -2):  # von außen (dunkel) nach innen (hell)
        v = int(250 * (1 - i / n) ** 0.65)
        arc = [-half + j * half / 6 for j in range(13)]
        pts = [(n, n)] + [(n + math.cos(b) * i, n + math.sin(b) * i) for b in arc]
        pygame.draw.polygon(s, (v, int(v * 0.96), int(v * 0.86)), pts)
    pygame.draw.circle(s, (120, 116, 104), (n, n), 7)  # etwas Licht rund um den Panzer
    return pygame.transform.box_blur(s.convert(), 3)


def bolt_points(x0, y0, x1, y1, rnd, depth=4):
    """Zackige Blitzlinie per Mittelpunkt-Verschiebung."""
    pts = [(x0, y0), (x1, y1)]
    amp = math.hypot(x1 - x0, y1 - y0) * 0.16
    for _ in range(depth):
        out = [pts[0]]
        for (ax, ay), (bx, by) in zip(pts, pts[1:]):
            nx, ny = by - ay, ax - bx  # Normale
            d = math.hypot(nx, ny) or 1
            off = rnd.uniform(-amp, amp)
            out += [
                ((ax + bx) / 2 + nx / d * off, (ay + by) / 2 + ny / d * off),
                (bx, by),
            ]
        pts = out
        amp *= 0.55
    return pts


# Rakete in lokalen Koordinaten (Spitze zeigt nach +x), Ursprung = Kollisionskreis
ROCKET_BODY = [(-10, -4), (7, -4), (7, 4), (-10, 4)]
ROCKET_NOSE = [(7, -4), (11, -3), (15, 0), (11, 3), (7, 4)]
ROCKET_FINS = [
    [(-2, -4), (-8, -4), (-13, -10), (-11, -4)],
    [(-2, 4), (-8, 4), (-13, 10), (-11, 4)],
]


@functools.cache
def rocket_sprite(color, step=0, k=1.0):
    """Raketen-Sprite, 4-fach überabgetastet gezeichnet -> glatte Kanten. step = 3°-Schritte."""
    if step or k != 1.0:
        return pygame.transform.rotozoom(rocket_sprite(color), -step * 3, k)
    ss, w, h = 4, 34, 24
    big = pygame.Surface((w * ss, h * ss), pygame.SRCALPHA)

    def tf(pts):
        return [((x + w / 2) * ss, (y + h / 2) * ss) for x, y in pts]

    dark = tuple(int(c * 0.6) for c in color)
    outline = (55, 55, 60)
    for fin in ROCKET_FINS:
        pygame.draw.polygon(big, dark, tf(fin))
        pygame.draw.polygon(big, outline, tf(fin), ss)
    pygame.draw.rect(big, outline, (*tf([(-12, -2.5)])[0], 3 * ss, 5 * ss))  # Düse
    pygame.draw.polygon(big, (210, 212, 220), tf(ROCKET_BODY))
    pygame.draw.polygon(
        big, (175, 178, 190), tf([(-10, 1.5), (7, 1.5), (7, 4), (-10, 4)])
    )
    pygame.draw.polygon(big, color, tf([(-7, -4), (-5, -4), (-5, 4), (-7, 4)]))  # Ring
    pygame.draw.polygon(big, color, tf(ROCKET_NOSE))
    pygame.draw.polygon(
        big, outline, tf(ROCKET_BODY[:1] + ROCKET_NOSE + ROCKET_BODY[3:]), ss
    )
    return pygame.transform.smoothscale(big, (w, h)).convert_alpha()


def draw_rocket(surf, x, y, a, color, k=1.0, flame=0.0):
    """Rakete mit Flamme der Länge flame (0 = aus)."""
    if flame > 0:
        ca, sa = math.cos(a) * k, math.sin(a) * k

        def tf(pts):
            return [(x + px * ca - py * sa, y + px * sa + py * ca) for px, py in pts]

        tail = -12
        pygame.draw.polygon(
            surf, (255, 140, 25), tf([(tail, -3), (tail - flame, 0), (tail, 3)])
        )
        pygame.draw.polygon(
            surf,
            (255, 235, 120),
            tf([(tail, -1.6), (tail - flame * 0.55, 0), (tail, 1.6)]),
        )
    spr = rocket_sprite(tuple(color), round(math.degrees(a) / 3) % 120, k)
    surf.blit(spr, spr.get_rect(center=(round(x), round(y))))


@functools.cache
def icon_font(size):
    return pygame.font.Font(None, size)


def draw_icon(surf, kind, cx, cy, k=1.0):
    """Power-up-Symbol, k = Skalierung."""

    def p(v):  # Längen mindestens 1 px, Vorzeichen bleibt
        return round(v * k) if v < 0 else max(1, round(v * k))

    if kind == "frag":
        pygame.draw.circle(surf, SHOT_C, (cx, cy), p(6))
        for i in range(6):
            a = i * math.pi / 3
            pygame.draw.circle(
                surf, SHOT_C, (cx + math.cos(a) * p(9), cy + math.sin(a) * p(9)), p(1.6)
            )
    elif kind == "mg":
        for dx in (-6, 0, 6):
            pygame.draw.circle(surf, SHOT_C, (cx + p(dx) if dx else cx, cy), p(2.6))
    elif kind == "laser":
        pts = [
            (cx - p(9), cy + p(4)),
            (cx - p(3), cy - p(4)),
            (cx + p(3), cy + p(4)),
            (cx + p(9), cy - p(4)),
        ]
        pygame.draw.lines(surf, (230, 20, 20), False, pts, p(2.5))
    elif kind == "rocket":
        draw_rocket(
            surf, cx + 2 * k, cy - 2 * k, -math.pi / 4, (235, 120, 20), 0.7 * k, 4
        )
    elif kind == "shield":
        pygame.draw.circle(surf, (40, 120, 230), (cx, cy), p(8), p(2.5))
    elif kind == "shotgun":
        for dy in (-7, 0, 7):
            end = (cx + p(8), cy + (p(dy) if dy else 0))
            pygame.draw.line(surf, SHOT_C, (cx - p(8), cy), end, p(1.5))
            pygame.draw.circle(surf, SHOT_C, end, p(2))
    elif kind == "bouncy":
        pygame.draw.circle(surf, (235, 70, 160), (cx, cy), p(7))
        pygame.draw.circle(surf, (255, 200, 230), (cx + p(-2), cy + p(-2)), p(2))
    elif kind == "mine":
        for a in (0.8, 2.4, 3.9, 5.5):
            pygame.draw.line(
                surf,
                SHOT_C,
                (cx, cy),
                (cx + math.cos(a) * p(9), cy + math.sin(a) * p(9)),
                p(2),
            )
        pygame.draw.circle(surf, (60, 60, 60), (cx, cy), p(6))
        pygame.draw.circle(surf, (240, 40, 40), (cx, cy), p(2))
    elif kind == "swap":
        c = (150, 60, 210)
        pygame.draw.line(surf, c, (cx - p(8), cy - p(4)), (cx + p(6), cy - p(4)), p(2))
        pygame.draw.polygon(
            surf, c, [(cx + p(9), cy - p(4)), (cx + p(4), cy - p(8)), (cx + p(4), cy)]
        )
        pygame.draw.line(surf, c, (cx + p(8), cy + p(4)), (cx - p(6), cy + p(4)), p(2))
        pygame.draw.polygon(
            surf, c, [(cx - p(9), cy + p(4)), (cx - p(4), cy), (cx - p(4), cy + p(8))]
        )
    elif kind == "rang":
        pts = [(cx - p(8), cy + p(5)), (cx, cy - p(6)), (cx + p(8), cy + p(5))]
        pygame.draw.lines(surf, (165, 100, 35), False, pts, p(3.5))
    elif kind == "ice":
        for i in range(3):
            a = i * math.pi / 3
            dx, dy = math.cos(a) * p(8), math.sin(a) * p(8)
            pygame.draw.line(
                surf, (60, 160, 235), (cx - dx, cy - dy), (cx + dx, cy + dy), p(2)
            )
        pygame.draw.circle(surf, (230, 245, 255), (cx, cy), p(2))
    elif kind == "hole":
        pygame.draw.circle(surf, (10, 10, 15), (cx, cy), p(8))
        pygame.draw.circle(surf, (150, 70, 220), (cx, cy), p(8), p(2))
        pygame.draw.circle(surf, (150, 70, 220), (cx + p(2), cy + p(-1)), p(3), 1)
    elif kind == "strike":
        c = (220, 30, 30)
        pygame.draw.circle(surf, c, (cx, cy), p(7), p(2))
        pygame.draw.line(surf, c, (cx - p(10), cy), (cx + p(10), cy), p(1.5))
        pygame.draw.line(surf, c, (cx, cy - p(10)), (cx, cy + p(10)), p(1.5))
    elif kind == "flame":
        outer = [
            (cx + p(1), cy - p(10)),
            (cx + p(7), cy - p(1)),
            (cx + p(6), cy + p(6)),
            (cx, cy + p(9)),
            (cx - p(6), cy + p(6)),
            (cx - p(7), cy),
            (cx - p(3), cy - p(4)),
        ]
        pygame.draw.polygon(surf, (235, 80, 20), outer)
        inner = [
            (cx, cy - p(3)),
            (cx + p(3.5), cy + p(3)),
            (cx, cy + p(7)),
            (cx - p(3.5), cy + p(3)),
        ]
        pygame.draw.polygon(surf, (255, 205, 60), inner)
    elif kind == "zap":
        pts = [
            (cx + p(3), cy - p(10)),
            (cx - p(6), cy + p(1)),
            (cx - p(0.5), cy + p(1)),
            (cx - p(3), cy + p(10)),
            (cx + p(6), cy - p(2)),
            (cx + p(1), cy - p(2)),
        ]
        pygame.draw.polygon(surf, (40, 150, 255), pts)
        pygame.draw.polygon(surf, (15, 60, 140), pts, max(1, p(1)))
    elif kind == "nuke":
        pygame.draw.circle(surf, (250, 205, 30), (cx, cy), p(10))
        pygame.draw.circle(surf, (40, 34, 10), (cx, cy), p(10), max(1, p(1.2)))
        for j in range(3):  # Strahlenzeichen
            a0 = j * 2 * math.pi / 3 - math.pi / 2 - math.pi / 6
            arc = [a0 + math.pi / 3 * q / 6 for q in range(7)]
            pts = [(cx + math.cos(a) * p(8.5), cy + math.sin(a) * p(8.5)) for a in arc]
            pts += [
                (cx + math.cos(a) * p(2.8), cy + math.sin(a) * p(2.8))
                for a in reversed(arc)
            ]
            pygame.draw.polygon(surf, (25, 22, 10), pts)
        pygame.draw.circle(surf, (25, 22, 10), (cx, cy), p(1.8))
    elif kind == "turbo":
        for dx in (-4, 3):
            pts = [
                (cx + p(dx - 3), cy - p(6)),
                (cx + p(dx + 3), cy),
                (cx + p(dx - 3), cy + p(6)),
            ]
            pygame.draw.lines(surf, (245, 130, 20), False, pts, p(3))
    elif kind == "mini":  # Pilz
        pygame.draw.rect(surf, (240, 225, 195), (cx - p(3), cy, p(6), p(8)))
        pygame.draw.circle(
            surf,
            (215, 40, 40),
            (cx, cy + p(1)),
            p(9),
            draw_top_left=True,
            draw_top_right=True,
        )
        for dx, dy in ((-4, -4), (3, -5), (5, -1)):
            pygame.draw.circle(surf, (255, 255, 255), (cx + p(dx), cy + p(dy)), p(1.5))
    elif kind == "ghost":
        body = (250, 250, 250)
        pygame.draw.circle(surf, body, (cx, cy - p(2)), p(7))
        pygame.draw.rect(surf, body, (cx - p(7), cy - p(2), p(14), p(8)))
        for dx in (-5, 0, 5):
            pygame.draw.circle(
                surf, body, (cx + (p(dx) if dx else 0), cy + p(6)), p(2.4)
            )
        pygame.draw.circle(surf, (120, 120, 130), (cx, cy), p(7), 1)
        for dx in (-3, 3):
            pygame.draw.circle(surf, SHOT_C, (cx + p(dx), cy - p(2)), p(1.5))
    elif kind == "invis":
        for j in range(10):
            a = j * math.pi / 5
            pygame.draw.circle(
                surf,
                (110, 120, 140),
                (cx + math.cos(a) * p(8), cy + math.sin(a) * p(8)),
                p(1.6),
            )
    elif kind == "confuse":
        pts = [
            (
                cx + math.cos(j * 0.5) * (1 + j * 0.3) * k,
                cy + math.sin(j * 0.5) * (1 + j * 0.3) * k,
            )
            for j in range(28)
        ]
        pygame.draw.lines(surf, (225, 175, 20), False, pts, p(2))
    elif kind == "mystery":
        img = icon_font(max(8, round(30 * k))).render("?", True, (110, 70, 0))
        surf.blit(img, img.get_rect(center=(cx, cy + p(1))))
    elif kind == "potato":
        r = pygame.Rect(0, 0, p(20), p(13))
        r.center = (cx, cy)
        pygame.draw.ellipse(surf, (175, 120, 60), r)
        pygame.draw.ellipse(surf, (110, 70, 30), r, p(1.5))
        for dx, dy in ((-4, -2), (2, 2), (5, -2)):
            pygame.draw.circle(surf, (110, 70, 30), (cx + p(dx), cy + p(dy)), p(1))


def box_color(kind):
    """Farbe der Kiste: Waffen orange, Effekte blau, Wundertüte gold."""
    if kind in ("mystery", "nuke", "potato"):
        return (255, 196, 40)
    return (60, 190, 255) if kind in BUFFS else (255, 122, 48)


def make_box_sprite(kind):
    """Kiste: helle Platte mit Symbol in einem Rahmen in der Farbe der Kategorie."""
    ss, n = 4, 32
    s = pygame.Surface((n * ss, n * ss), pygame.SRCALPHA)
    frame = box_color(kind)
    pygame.draw.rect(s, (14, 15, 19), (0, 0, n * ss, n * ss), border_radius=8 * ss)
    pygame.draw.rect(
        s, frame, (ss, ss, (n - 2) * ss, (n - 2) * ss), border_radius=7 * ss
    )
    pygame.draw.rect(
        s,
        mix(frame, (255, 255, 255), 0.45),
        (ss, ss, (n - 2) * ss, 4 * ss),
        border_top_left_radius=7 * ss,
        border_top_right_radius=7 * ss,
    )
    face = (255, 232, 140) if kind == "mystery" else (230, 234, 242)
    inner = pygame.Rect(4 * ss, 4 * ss, (n - 8) * ss, (n - 8) * ss)
    pygame.draw.rect(s, shade(face, 0.72), inner, border_radius=4 * ss)
    pygame.draw.rect(
        s, face, inner.inflate(0, -ss).move(0, -ss // 2), border_radius=4 * ss
    )
    for x, y in (
        (2.7, 2.7),
        (n - 2.7, 2.7),
        (2.7, n - 2.7),
        (n - 2.7, n - 2.7),
    ):  # Nieten
        pygame.draw.circle(s, shade(frame, 0.55), (x * ss, y * ss), 1.1 * ss)
    draw_icon(s, kind, n * ss // 2, n * ss // 2, 0.88 * ss)
    return pygame.transform.smoothscale(s, (n, n)).convert_alpha()


def make_ice(size):
    s = pygame.Surface((size, size), pygame.SRCALPHA)
    pygame.draw.rect(
        s, (170, 220, 250, 150), (0, 0, size, size), border_radius=size // 5
    )
    pygame.draw.rect(
        s, (235, 250, 255, 220), (0, 0, size, size), 2, border_radius=size // 5
    )
    pygame.draw.line(
        s, (255, 255, 255, 200), (size * 0.2, size * 0.35), (size * 0.4, size * 0.15), 3
    )
    pygame.draw.line(
        s, (255, 255, 255, 160), (size * 0.25, size * 0.6), (size * 0.6, size * 0.25), 2
    )
    return s.convert_alpha()


def draw_crown(surf, cx, cy, k=1.0):
    pts = [(-7, 4), (-7, -3), (-3.5, 0.5), (0, -5), (3.5, 0.5), (7, -3), (7, 4)]
    pts = [(cx + x * k, cy + y * k) for x, y in pts]
    pygame.draw.polygon(surf, (250, 200, 30), pts)
    pygame.draw.polygon(surf, (160, 110, 0), pts, 1)


def build_maze_surface(arena):
    """Boden mit Platten und Struktur, Wände mit Schatten und Kanten, Vignette.
    Wird einmal pro Runde gebaut, Spuren und Brandflecken kommen danach direkt drauf."""
    th = arena.theme
    ww, wh = arena.ww, arena.wh
    # Zufall nur aus Labyrinth-Daten, die sich nie ändern: wird die Fläche nach einer
    # Atombombe neu gebaut, sieht der Boden genauso aus wie vorher
    rnd = random.Random(arena.id * 7919 + arena.cols * 31 + arena.rows)
    surf = pygame.Surface((ww, wh)).convert()
    surf.fill(shade(th["floor"], 0.4))
    x0, y0 = arena.ox, arena.oy
    fw, fh = arena.cols * CELL, arena.rows * CELL
    floor, line = th["floor"], th["line"]
    for r in range(arena.rows):  # Bodenplatten, jede etwas anders hell
        for c in range(arena.cols):
            rect = (x0 + c * CELL, y0 + r * CELL, CELL, CELL)
            pygame.draw.rect(surf, shade(floor, rnd.uniform(0.93, 1.07)), rect)
    for _ in range(fw * fh // 240):  # feine Sprenkel
        pos = (rnd.uniform(x0, x0 + fw), rnd.uniform(y0, y0 + fh))
        size = rnd.choice((1, 1, 1, 2))
        pygame.draw.rect(surf, shade(floor, rnd.uniform(0.8, 1.22)), (*pos, size, size))
    amb = th["amb"]
    if amb in ("dust", "neon"):  # Metallplatten bzw. Gitter
        for r in range(arena.rows):
            for c in range(arena.cols):
                cx, cy = x0 + c * CELL, y0 + r * CELL
                mid = CELL // 2
                pygame.draw.line(surf, line, (cx + mid, cy), (cx + mid, cy + CELL))
                pygame.draw.line(surf, line, (cx, cy + mid), (cx + CELL, cy + mid))
                if amb == "dust":
                    for dx, dy in (
                        (8, 8),
                        (CELL - 8, 8),
                        (8, CELL - 8),
                        (CELL - 8, CELL - 8),
                    ):
                        pygame.draw.circle(
                            surf, shade(floor, 1.35), (cx + dx, cy + dy), 1.6
                        )
    elif amb == "sand":  # Rippel
        for _ in range(fw * fh // 2600):
            x, y = rnd.uniform(x0, x0 + fw - 40), rnd.uniform(y0, y0 + fh)
            pts = [
                (x + i * 4, y + math.sin(i * 0.7) * 2)
                for i in range(rnd.randint(6, 12))
            ]
            pygame.draw.lines(surf, shade(floor, 1.12), False, pts)
            pygame.draw.lines(
                surf, shade(floor, 0.88), False, [(px, py + 2) for px, py in pts]
            )
    elif amb == "snow":  # Frost
        for _ in range(fw * fh // 1800):
            x, y = rnd.uniform(x0, x0 + fw), rnd.uniform(y0, y0 + fh)
            a, d = rnd.uniform(0, math.pi), rnd.uniform(6, 22)
            end = (x + math.cos(a) * d, y + math.sin(a) * d)
            pygame.draw.line(surf, shade(floor, 1.25), (x, y), end)
    elif amb == "firefly":  # Laub
        for _ in range(fw * fh // 700):
            x, y = rnd.uniform(x0, x0 + fw), rnd.uniform(y0, y0 + fh)
            leaf = pygame.Rect(0, 0, rnd.randint(3, 7), rnd.randint(2, 4))
            leaf.center = (x, y)
            pygame.draw.ellipse(surf, shade(floor, rnd.choice((0.75, 1.25, 1.4))), leaf)
    for c in range(arena.cols + 1):  # Fugen zwischen den Zellen
        x = x0 + c * CELL
        pygame.draw.line(surf, shade(line, 1.15), (x, y0), (x, y0 + fh))
    for r in range(arena.rows + 1):
        y = y0 + r * CELL
        pygame.draw.line(surf, shade(line, 1.15), (x0, y), (x0 + fw, y))
    for _ in range(max(2, fw * fh // 90000)):  # ein paar Ölflecken
        pos = (rnd.uniform(x0, x0 + fw), rnd.uniform(y0, y0 + fh))
        oil = puff(rnd.randint(10, 22), 70, shade(floor, 0.45))
        surf.blit(oil, oil.get_rect(center=pos))
    if th.get("cracks"):  # glühende Risse im Boden
        layer = pygame.Surface((ww, wh)).convert()
        layer.set_clip((x0, y0, fw, fh))
        cracks = []
        for _ in range(max(5, fw * fh // 15000)):
            x, y = rnd.uniform(x0, x0 + fw), rnd.uniform(y0, y0 + fh)
            a = rnd.uniform(0, 2 * math.pi)
            pts = [(x, y)]
            for _ in range(rnd.randint(3, 7)):
                a += rnd.uniform(-0.9, 0.9)
                d = rnd.uniform(8, 24)
                x, y = x + math.cos(a) * d, y + math.sin(a) * d
                pts.append((x, y))
            cracks.append(pts)
        for pts in cracks:
            pygame.draw.lines(layer, th["glow"], False, pts, 5)
        layer = pygame.transform.box_blur(layer, 6)
        layer.fill((120, 120, 120), special_flags=pygame.BLEND_RGB_MULT)
        surf.blit(layer, (0, 0), special_flags=pygame.BLEND_RGB_ADD)
        surf.set_clip((x0, y0, fw, fh))
        for pts in cracks:
            pygame.draw.lines(surf, (190, 80, 30), False, pts, 2)
            pygame.draw.lines(surf, (255, 190, 110), False, pts, 1)
        surf.set_clip(None)
    # Wände als Maske: so gibt es an Kreuzungen keine Nähte
    mask_surf = pygame.Surface((ww, wh), pygame.SRCALPHA)
    for w in arena.walls:
        pygame.draw.rect(mask_surf, (255, 255, 255, 255), w)
    mask = pygame.mask.from_surface(mask_surf)
    shadow = pygame.Surface(
        (ww, wh), pygame.SRCALPHA
    )  # Schlagschatten nach rechts unten
    for x, y, w, h in arena.walls:
        pygame.draw.rect(shadow, (0, 0, 0, 150), (x + 4, y + 7, w + 2, h))
    surf.blit(pygame.transform.box_blur(shadow, 4), (0, 0))
    if th.get("glow") and not th.get("cracks"):  # Neon: die Wände leuchten
        g = mask.to_surface(setcolor=(*th["glow"], 255), unsetcolor=(0, 0, 0, 255))
        g = pygame.transform.box_blur(g.convert(), 7)
        surf.blit(g, (0, 0), special_flags=pygame.BLEND_RGB_ADD)
        surf.blit(g, (0, 0), special_flags=pygame.BLEND_RGB_ADD)
    clear = (0, 0, 0, 0)
    surf.blit(mask.to_surface(setcolor=(*th["wall"], 255), unsetcolor=clear), (0, 0))
    lo, hi = mask.copy(), mask.copy()
    lo.erase(mask, (-2, -2))  # untere/rechte Kante
    hi.erase(mask, (2, 2))  # obere/linke Kante
    surf.blit(lo.to_surface(setcolor=(*th["lo"], 255), unsetcolor=clear), (0, 0))
    surf.blit(hi.to_surface(setcolor=(*th["hi"], 255), unsetcolor=clear), (0, 0))
    surf.blit(vignette(ww, wh), (0, 0), special_flags=pygame.BLEND_RGB_MULT)
    return surf


# Schriften: Titel in einer kräftigen Systemschrift (wenn vorhanden), sonst die Standardschrift
TITLE_FONTS = ("bahnschrift", "impact", "robotocondensed", "liberationsansnarrow")


@functools.cache
def font(size, title=False):
    if title:
        for name in TITLE_FONTS:
            path = pygame.font.match_font(name, bold=name != "impact")
            if path:
                try:
                    f = pygame.font.Font(path, size)
                except (OSError, pygame.error):
                    continue
                f.bold = (
                    name == "bahnschrift"
                )  # Bahnschrift hat nur eine (variable) Datei
                return f
    return pygame.font.Font(None, size)


@functools.lru_cache(maxsize=400)
def fancy(text, size, color, ow=3, title=True):
    """Text mit Farbverlauf, dunklem Rand und Schlagschatten (gecacht)."""
    f = font(size, title)
    face = f.render(text, True, (255, 255, 255))
    w, h = face.get_size()
    grad = pygame.Surface((w, h), pygame.SRCALPHA)
    top, bottom = mix(color, (255, 255, 255), 0.5), shade(color, 0.8)
    for y in range(h):
        pygame.draw.line(grad, mix(top, bottom, y / max(1, h - 1)), (0, y), (w, y))
    face.blit(grad, (0, 0), special_flags=pygame.BLEND_RGBA_MULT)
    f.outline = ow
    edge = f.render(text, True, (12, 12, 18))
    f.outline = 0
    sh = 2 + size // 22  # Schlagschatten
    out = pygame.Surface(
        (edge.get_width() + sh, edge.get_height() + sh), pygame.SRCALPHA
    )
    shadow = edge.copy()
    shadow.fill((0, 0, 0, 120), special_flags=pygame.BLEND_RGBA_MULT)
    out.blit(shadow, (sh, sh))
    out.blit(edge, (0, 0))
    out.blit(face, (ow, ow))
    return out.convert_alpha()


@functools.lru_cache(maxsize=96)
def fancy_glow(text, size, color, ow=3, radius=10, level=8):
    """Leuchten hinter einem fancy-Text (für BLEND_ADD). Beim additiven Blenden zählt kein
    Alpha, deshalb gibt es Helligkeitsstufen (level 1..8) statt set_alpha."""
    if level < 8:
        s = fancy_glow(text, size, color, ow, radius).copy()
        s.fill((level * 32,) * 3, special_flags=pygame.BLEND_RGB_MULT)
        return s
    base = fancy(text, size, color, ow)
    pad = radius * 2
    s = pygame.Surface((base.get_width() + 2 * pad, base.get_height() + 2 * pad))
    solid = pygame.transform.solid_overlay(base, (*color, 255), keep_alpha=True)
    s.blit(solid, (pad, pad))
    return pygame.transform.box_blur(pygame.transform.box_blur(s, radius), radius)


def rich_parts(text, names):
    """Zerlegt text in (Stück, Farbe): Spielernamen in ihrer Farbe, der Rest None."""
    parts = [(text, None)]
    for name, col in names:
        out = []
        for piece, c in parts:
            if c is not None:
                out.append((piece, c))
                continue
            for j, bit in enumerate(piece.split(name)):
                if j:
                    out.append((name, col))
                if bit:
                    out.append((bit, None))
        parts = out
    return parts


# --- Partikel ----------------------------------------------------------------

SPARK, FIRE, SMOKE, DUST, DEBRIS, EMBER, SHARD, CONFETTI, FLASH, STAR, RING = range(11)
FIRE_GRAD = [
    (255, 248, 222),
    (255, 222, 130),
    (255, 172, 64),
    (242, 112, 32),
    (205, 64, 22),
    (130, 34, 12),
    (60, 16, 6),
]


class Particles:
    """Rein optische Partikel (Funken, Feuer, Rauch, Trümmer ...). Jeder Client hat seine
    eigenen, nichts davon geht übers Netz.
    Partikel: [x, y, vx, vy, alter, leben, art, größe, farbe, bremsen, schwerkraft, winkel, drehung]
    """

    MAX = 1600

    def __init__(self):
        self.items = []

    def add(
        self,
        kind,
        x,
        y,
        vx=0.0,
        vy=0.0,
        life=1.0,
        size=3.0,
        color=(255, 255, 255),
        drag=0.0,
        grav=0.0,
        spin=0.0,
    ):
        if len(self.items) < self.MAX:
            a = random.uniform(0, 2 * math.pi)
            self.items.append(
                [x, y, vx, vy, 0.0, life, kind, size, color, drag, grav, a, spin]
            )

    def burst(
        self,
        kind,
        x,
        y,
        n,
        speed,
        life,
        size,
        color=(255, 255, 255),
        drag=0.0,
        grav=0.0,
        angle=0.0,
        spread=math.pi,
        spin=0.0,
    ):
        """n Partikel rundherum bzw. im Fächer angle ± spread."""
        for _ in range(int(n)):
            a = angle + random.uniform(-spread, spread)
            v = speed * random.uniform(0.3, 1.0)
            self.add(
                kind,
                x,
                y,
                math.cos(a) * v,
                math.sin(a) * v,
                life * random.uniform(0.6, 1.25),
                size * random.uniform(0.7, 1.3),
                color,
                drag,
                grav,
                random.uniform(-spin, spin),
            )

    def update(self, dt, arena):
        """Bewegt alles. Gibt die Trümmer zurück, die liegen geblieben sind."""
        keep, landed = [], []
        for p in self.items:
            p[4] += dt
            if p[4] >= p[5]:
                if p[6] == DEBRIS:
                    landed.append(p)
                continue
            if p[9]:
                f = max(0.0, 1 - p[9] * dt)
                p[2] *= f
                p[3] *= f
            p[3] += p[10] * dt
            nx, ny = p[0] + p[2] * dt, p[1] + p[3] * dt
            if arena is not None and p[6] in (DEBRIS, SHARD):  # prallen an Wänden ab
                walls = arena.near(nx, ny)
                if circle_hits_walls(nx, p[1], 2, walls):
                    p[2], nx = -p[2] * 0.4, p[0]
                if circle_hits_walls(nx, ny, 2, walls):
                    p[3], ny = -p[3] * 0.4, p[1]
            p[0], p[1] = nx, ny
            p[11] += p[12] * dt
            keep.append(p)
        self.items = keep
        return landed


# --- Sound -------------------------------------------------------------------

# Ein Klang besteht aus Schichten: (Startfrequenz, Endfrequenz, Dauer, Welle, Lautstärke,
# Rauschanteil, Abklingen[, Verzögerung, Hüllkurve]). Welle "noise" ist gefiltertes Rauschen,
# die Frequenzen bestimmen dann, wie hell es klingt. Hüllkurve "swell" schwillt an und ab.
SOUNDS = {
    "shot": [
        (880, 320, 0.09, "sq", 0.12, 0.0, 5),
        (3200, 900, 0.05, "noise", 0.1, 1, 8),
    ],
    "mg": [(700, 420, 0.05, "sq", 0.1, 0.2, 6), (2600, 900, 0.04, "noise", 0.1, 1, 8)],
    "sg": [(300, 110, 0.2, "sq", 0.18, 0.5, 8), (2600, 300, 0.28, "noise", 0.32, 1, 6)],
    "rocket": [
        (110, 240, 0.45, "saw", 0.14, 0.6, 3),
        (1600, 500, 0.5, "noise", 0.2, 1, 3),
    ],
    "frag": [(220, 90, 0.15, "sin", 0.3, 0.0, 5)],
    "laser": [
        (1800, 180, 0.3, "saw", 0.12, 0.0, 4),
        (2600, 260, 0.32, "sin", 0.1, 0, 3),
    ],
    "boom": [
        (95, 32, 0.8, "sin", 0.5, 0.0, 4),
        (2600, 200, 0.75, "noise", 0.5, 1, 4.5),
        (6000, 2400, 0.16, "noise", 0.18, 1, 9, 0.02),
    ],
    "bigboom": [
        (75, 22, 1.4, "sin", 0.6, 0.0, 3),
        (2000, 110, 1.3, "noise", 0.6, 1, 3.2),
        (6000, 1800, 0.3, "noise", 0.24, 1, 8, 0.03),
    ],
    "nuke": [
        (60, 16, 3.0, "sin", 0.62, 0.0, 1.6),
        (1500, 50, 3.0, "noise", 0.75, 1, 1.5),
        (7000, 2200, 0.5, "noise", 0.35, 1, 6),
    ],
    "pick": [
        (600, 1150, 0.13, "sq", 0.1, 0.0, 2),
        (900, 1800, 0.12, "sin", 0.08, 0, 2, 0.06),
    ],
    "mystery": [
        (520, 520, 0.07, "sq", 0.07, 0, 3),
        (660, 660, 0.07, "sq", 0.07, 0, 3, 0.07),
        (780, 780, 0.07, "sq", 0.07, 0, 3, 0.14),
        (1040, 1040, 0.25, "sq", 0.09, 0, 3, 0.21),
    ],
    "tele": [
        (300, 1500, 0.25, "sin", 0.2, 0.0, 2),
        (2000, 7000, 0.25, "noise", 0.08, 1, 3),
    ],
    "ice": [(1900, 2700, 0.22, "sin", 0.1, 0.1, 3)],
    "freeze": [
        (3600, 1800, 0.4, "noise", 0.26, 1, 4),
        (2600, 3400, 0.3, "sin", 0.07, 0, 4),
    ],
    "bnc": [(280, 620, 0.12, "sin", 0.22, 0.0, 4)],
    "rang": [(200, 420, 0.3, "saw", 0.08, 0.4, 3)],
    "mine": [(1200, 1200, 0.05, "sq", 0.12, 0.0, 6)],
    "hole": [
        (90, 38, 0.8, "saw", 0.18, 0.2, 1.5),
        (700, 60, 0.9, "noise", 0.2, 1, 1.5),
    ],
    "strike": [(1000, 560, 0.45, "sq", 0.08, 0.0, 1)],
    "whistle": [(2000, 480, 0.95, "sin", 0.11, 0.0, 0.3)],
    "jet": [(300, 3000, 1.3, "noise", 0.32, 1, 0, 0, "swell")],
    "blip": [(520, 760, 0.08, "sin", 0.16, 0.0, 3)],
    "win": [
        (523, 523, 0.13, "sq", 0.09, 0, 3),
        (659, 659, 0.13, "sq", 0.09, 0, 3, 0.11),
        (784, 784, 0.13, "sq", 0.09, 0, 3, 0.22),
        (1046, 1046, 0.45, "sq", 0.11, 0, 3, 0.33),
        (1046, 1046, 0.45, "sin", 0.08, 0, 3, 0.33),
    ],
    "tink": [(2700, 1900, 0.05, "sin", 0.08, 0.2, 8)],
    "shield": [
        (1500, 700, 0.3, "sq", 0.07, 0.0, 5),
        (6500, 2500, 0.32, "noise", 0.24, 1, 6),
    ],
    "zap": [
        (150, 90, 0.42, "sq", 0.12, 0.6, 3),
        (7500, 2500, 0.42, "noise", 0.32, 1, 4),
    ],
    "flame": [(1500, 500, 0.22, "noise", 0.16, 1, 4)],
    "tick": [(1400, 1400, 0.05, "sq", 0.14, 0.0, 6)],
    "siren": [
        (620, 940, 0.42, "saw", 0.11, 0.0, 0, 0, "swell"),
        (940, 620, 0.42, "saw", 0.11, 0.0, 0, 0.42, "swell"),
    ],
    "drop": [
        (150, 60, 0.18, "sin", 0.26, 0.0, 6),
        (1000, 200, 0.15, "noise", 0.14, 1, 8),
    ],
    "round": [
        (220, 220, 0.7, "saw", 0.05, 0.0, 0, 0, "swell"),
        (330, 330, 0.7, "saw", 0.04, 0.0, 0, 0, "swell"),
        (440, 440, 0.6, "sq", 0.03, 0, 0, 0.08, "swell"),
    ],
    "ann": [
        (660, 990, 0.16, "sq", 0.11, 0.0, 3),
        (990, 1320, 0.26, "sq", 0.11, 0, 3, 0.12),
    ],
    "slowmo": [
        (170, 38, 1.1, "sin", 0.22, 0.0, 1.4),
        (1000, 70, 1.2, "noise", 0.14, 1, 1.4),
    ],
    "clank": [(520, 300, 0.12, "sq", 0.12, 0.5, 7)],
}
SHOT_SOUNDS = {
    "b": "shot",
    "mg": "mg",
    "sg": "sg",
    "rocket": "rocket",
    "frag": "frag",
    "bnc": "bnc",
    "rang": "rang",
    "swap": "tele",
    "ice": "ice",
    "hole": "hole",
    "mine": "mine",
    "strike": "strike",
    "fl": "flame",
    "nuke": "frag",
}
SOUND_GAP = {"flame": 0.11, "tink": 0.06, "boom": 0.05, "siren": 0.5}  # Mindestabstand


class Sfx:
    """Synthetisierte Soundeffekte, ganz ohne Dateien, mit Stereo-Position.
    Ohne Audiogerät bleibt es still. Die Klänge entstehen in einem Hintergrund-Thread.
    """

    def __init__(self):
        self.on = True
        self.sounds = {}
        self.last = {}
        try:
            if not pygame.mixer.get_init():
                pygame.mixer.init(22050, -16, 2, 512)
            freq, size, channels = pygame.mixer.get_init()
            if size != -16:
                return
            pygame.mixer.set_num_channels(32)
        except pygame.error:
            return
        threading.Thread(target=self._build, args=(freq, channels), daemon=True).start()

    def _build(self, rate, channels):
        for name, layers in SOUNDS.items():
            try:
                data = self._synth(rate, channels, layers)
                self.sounds[name] = pygame.mixer.Sound(buffer=data)
            except (pygame.error, TypeError, ValueError):
                return

    @staticmethod
    def _synth(rate, channels, layers):
        total = max(
            int(rate * (lay[2] + (lay[7] if len(lay) > 7 else 0))) for lay in layers
        )
        buf = [0.0] * (total + 1)
        for lay in layers:
            f0, f1, dur, wave, vol, noise, decay, *rest = lay
            start = int(rate * rest[0]) if rest else 0
            swell = len(rest) > 1 and rest[1] == "swell"
            n = int(rate * dur)
            rnd = random.Random(int(f0 * 13 + f1))  # gleicher Klang bei jedem Start
            uniform, sin, exp = rnd.uniform, math.sin, math.exp
            phase = lp = 0.0
            att = rate * 0.004
            tau = 2 * math.pi
            for j in range(n):
                t = j / n
                f = f0 + (f1 - f0) * t
                if wave == "noise":
                    c = min(1.0, f / 5000)
                    lp += (uniform(-1, 1) - lp) * c
                    v = lp * (0.5 / max(c, 0.05)) ** 0.5
                else:
                    phase += f / rate
                    ph = phase % 1
                    if wave == "sin":
                        tone = sin(tau * ph)
                    elif wave == "saw":
                        tone = 2 * ph - 1
                    else:
                        tone = 1.0 if ph < 0.5 else -1.0
                    if noise:
                        lp += (uniform(-1, 1) - lp) * 0.25
                        tone = tone * (1 - noise) + lp * 2 * noise
                    v = tone
                env = sin(math.pi * t) if swell else exp(-decay * t)
                if j < att:
                    env *= j / att
                buf[start + j] += v * env * vol
        out = array("h", (int(max(-1.0, min(1.0, v)) * 32767) for v in buf))
        if channels == 1:
            return out.tobytes()
        multi = array("h", bytes(2 * channels * len(out)))
        for ch in range(channels):
            multi[ch::channels] = out
        return multi.tobytes()

    def play(self, name, pan=None, vol=1.0):
        """pan: 0 = ganz links, 1 = ganz rechts, None = Mitte."""
        snd = self.sounds.get(name)
        if not (self.on and snd):
            return
        now = time.perf_counter()
        if now - self.last.get(name, 0) < SOUND_GAP.get(name, 0.035):
            return  # z. B. 7 Schrotkugeln -> 1 Knall
        self.last[name] = now
        ch = snd.play()
        if ch is not None:
            p = 0.5 if pan is None else min(1.0, max(0.0, pan))
            ch.set_volume(vol * min(1.0, 2 * (1 - p)), vol * min(1.0, 2 * p))


# --- Musik -------------------------------------------------------------------

MUSIC_BPM = 116
MUSIC_VOL = 0.3
# Takte: Grundton der Bass-Oktave (Hz) und Akkordtöne in Halbtönen (Am, F, C, G)
MUSIC_CHORDS = [
    (55.0, (0, 3, 7, 12)),
    (43.65, (0, 4, 7, 12)),
    (65.41, (0, 4, 7, 12)),
    (49.0, (0, 4, 7, 12)),
]


class Music:
    """Kleiner Synthwave-Loop: jeder Takt wird einmal synthetisiert und dann lückenlos
    hintereinander in einen eigenen Kanal gequeued (Channel.queue), ganz ohne Dateien.
    """

    def __init__(self):
        self.on = True
        self.bars = []  # fertige Takte in Abspielreihenfolge
        self.idx = 0
        self.channel = None
        self.playing = None
        try:
            if not pygame.mixer.get_init():
                return
            rate, size, channels = pygame.mixer.get_init()
            if size != -16:
                return
            pygame.mixer.set_reserved(1)  # Kanal 0 gehört der Musik
            self.channel = pygame.mixer.Channel(0)
        except pygame.error:
            return
        threading.Thread(target=self._build, args=(rate, channels), daemon=True).start()

    def update(self, enabled, ts=1.0):
        """Jeden Frame aufrufen: nächsten Takt nachlegen, in der Zeitlupe leiser."""
        ch = self.channel
        if ch is None or not self.bars:
            return
        if not (self.on and enabled):
            if ch.get_busy():
                ch.fadeout(300)
            return
        if not ch.get_busy():
            self.playing = self.bars[self.idx % len(self.bars)]
            self.idx += 1
            ch.play(self.playing)
        elif ch.get_queue() is None:
            self.playing = self.bars[self.idx % len(self.bars)]
            self.idx += 1
            ch.queue(self.playing)
        vol = MUSIC_VOL * (0.4 + 0.6 * ts)
        for bar in self.bars:
            if abs(bar.get_volume() - vol) > 0.01:
                bar.set_volume(vol)

    def _build(self, rate, channels):
        try:
            step = 60 / MUSIC_BPM / 4  # Sechzehntel
            n = int(rate * step * 16)
            drums = self._drums(rate, n, step)
            plain, lead = [], []
            for root, chord in MUSIC_CHORDS:
                base = drums[:]
                self._bass(base, rate, step, root)
                self._pad(base, rate, n, root, chord)
                arp = [0.0] * n
                self._arp(arp, rate, step, root, chord)
                plain.append(self._sound(base, channels))
                lead.append(self._sound([b + a for b, a in zip(base, arp)], channels))
            self.bars = plain + lead  # erst ohne, dann mit Melodie
        except (pygame.error, ValueError, TypeError):
            self.bars = []

    @staticmethod
    def _sound(buf, channels):
        out = array("h", (int(max(-1.0, min(1.0, v)) * 32767) for v in buf))
        if channels > 1:
            multi = array("h", bytes(2 * channels * len(out)))
            for ch in range(channels):
                multi[ch::channels] = out
            out = multi
        snd = pygame.mixer.Sound(buffer=out.tobytes())
        snd.set_volume(MUSIC_VOL)
        return snd

    @staticmethod
    def _drums(rate, n, step):
        buf = [0.0] * n
        rnd = random.Random(5)
        sin, exp, tau = math.sin, math.exp, 2 * math.pi
        for k in range(16):
            at = int(k * step * rate)
            if k % 4 == 0:  # Kick: Sinus mit fallender Tonhöhe
                phase = 0.0
                for j in range(min(int(0.28 * rate), n - at)):
                    t = j / rate
                    phase += (48 + 110 * exp(-t * 28)) / rate
                    buf[at + j] += sin(tau * phase) * exp(-t * 9) * 0.62
            if k in (4, 12):  # Clap: Rauschen, mehrfach angerissen
                lp = 0.0
                for j in range(min(int(0.22 * rate), n - at)):
                    t = j / rate
                    lp += (rnd.uniform(-1, 1) - lp) * 0.55
                    hit = exp(-((t * 1000) % 11) / 3) if t < 0.03 else 1.0
                    buf[at + j] += lp * exp(-t * 16) * 0.32 * hit
            if k % 2 == 1 or k % 4 == 2:  # Hi-Hats, auf den Offbeats lauter
                vol = 0.1 if k % 4 == 2 else 0.045
                lp = 0.0
                for j in range(min(int(0.06 * rate), n - at)):
                    t = j / rate
                    x = rnd.uniform(-1, 1)
                    lp += (x - lp) * 0.5
                    buf[at + j] += (x - lp) * exp(-t * 70) * vol
        return buf

    @staticmethod
    def _bass(buf, rate, step, root):
        """Achtel-Bass (gefilterter Sägezahn), jede zweite Note eine Oktave höher."""
        n = len(buf)
        exp = math.exp
        for k in range(0, 16, 2):
            f = root * (2 if k % 4 == 2 else 1)
            at = int(k * step * rate)
            phase = lp = 0.0
            for j in range(min(int(step * 1.8 * rate), n - at)):
                t = j / rate
                phase += f / rate
                saw = 2 * (phase % 1) - 1
                lp += (saw - lp) * (0.05 + 0.25 * exp(-t * 18))  # Filter schließt sich
                env = min(1.0, t * 400) * exp(-t * 7)
                buf[at + j] += lp * env * 0.5

    @staticmethod
    def _pad(buf, rate, n, root, chord):
        """Leiser Flächenklang über den ganzen Takt (Grundton + Quinte, zwei Oktaven höher)."""
        sin, tau = math.sin, 2 * math.pi
        for semi in (chord[0], chord[2]):
            f = root * 4 * 2 ** (semi / 12)
            fd = f * 1.004  # leicht verstimmt -> Schwebung
            for j in range(n):
                t = j / rate
                env = min(1.0, t * 3) * min(1.0, (n - j) / rate * 6)
                buf[j] += (sin(tau * f * t) + sin(tau * fd * t)) * env * 0.028

    @staticmethod
    def _arp(buf, rate, step, root, chord):
        """Arpeggio in Sechzehnteln mit Echo."""
        n = len(buf)
        exp = math.exp
        notes = list(chord) + [chord[2], chord[1]]
        echo = int(3 * step * rate)
        for k in range(16):
            f = root * 8 * 2 ** (notes[k % len(notes)] / 12)
            at = int(k * step * rate)
            phase = 0.0
            for j in range(int(step * 1.5 * rate)):
                t = j / rate
                phase += f / rate
                sq = 0.6 if (phase % 1) < 0.5 else -0.6
                v = sq * min(1.0, t * 600) * exp(-t * 22) * 0.12
                buf[(at + j) % n] += v
                buf[(at + j + echo) % n] += v * 0.35


# --- Renderer ----------------------------------------------------------------

TURRET_AIR = 0.8  # so lange fliegt ein abgesprengter Turm
AMBIENT = {"dust": 30, "sand": 55, "ember": 40, "snow": 70, "firefly": 22, "neon": 28}
BOUNCE_FX = ("b", "mg", "sg", "bnc", "frag", "ice", "swap")  # Funken beim Abprallen


class Demo:
    """Bots, die im Hintergrund von Menü und Wartebildschirmen gegeneinander kämpfen."""

    def __init__(self):
        n = random.randint(3, 4)
        self.game = Game(range(n))
        self.game.bots = list(range(n))
        self.bots = [Bot(s, random.uniform(0.75, 0.95)) for s in range(n)]
        self.acc, self.last = 0.0, None

    def step(self):
        now = time.perf_counter()
        self.acc = min(self.acc + (now - self.last if self.last else 0.0), 0.1)
        self.last = now
        while self.acc >= TICK:
            self.acc -= TICK
            for b in self.bots:
                self.game.apply_input(b.slot, *b.think(self.game), TICK)
            self.game.step(TICK)


class Renderer:
    def __init__(self, screen):
        self.screen = screen
        self.font = pygame.font.Font(None, 28)
        self.big = pygame.font.Font(None, 48)
        self.score_font = pygame.font.Font(None, 64)
        self.small = pygame.font.Font(None, 22)
        self.tiny = pygame.font.Font(None, 18)
        tanks = [make_tank_sprite(c) for c in COLORS]
        wrecks = [make_tank_sprite(c, wreck=True) for c in COLORS]
        self.sprites = tanks + wrecks  # Wrack von Slot i = i + DEAD_SPRITE
        self.turrets = [make_turret_sprite(c) for c in COLORS]
        self.turret_wrecks = [
            make_turret_sprite(mix(c, (34, 28, 26), 0.7)) for c in COLORS
        ]
        self.icons = [pygame.transform.rotozoom(s, 0, 1.6) for s in tanks]
        self.hud_icons = [pygame.transform.rotozoom(s, 0, 1.15) for s in tanks + wrecks]
        self.boxes = {k: make_box_sprite(k) for k in POWERUPS + ["nuke", "potato"]}
        self.ice = {1.0: make_ice(40), MINI_SCALE: make_ice(24)}
        self.jet = make_jet()
        self.jet_shadow = pygame.transform.gaussian_blur(
            pygame.transform.solid_overlay(self.jet, (0, 0, 0, 110), keep_alpha=True), 3
        )
        self.jets = []  # Jets in diesem Bild (werden über den Panzern gezeichnet)
        self.hud_bg = self._make_hud_bg()
        self.parts = Particles()
        self.fog = None
        self.rot_cache = {}
        self.text_cache = {}
        self.card_cache = {}
        self.feed_cache = {}
        self.maze_surf = None
        self.maze_arena = (
            None  # Objekt statt id: ein neues Game fängt wieder bei id 1 an
        )
        self.world = self.light = None  # Zeichenfläche in Weltgröße und Lichtpuffer
        self.debug = False
        self.sfx = Sfx()
        self.round_arena = None  # Labyrinth, für das die Merker unten gelten
        self.round_start = 0.0
        self.seen_shots, self.seen_fx = set(), set()
        self.tracks = {}  # slot -> letzte Kettenspur-Position
        self.taunts = {}
        self.last_msg = None
        self.trauma = 0.0  # Wackeln (0..1), klingt von selbst ab
        self.flash = 0.0  # Aufblitzen des ganzen Bildes
        self.last_frame = time.perf_counter()
        self.hist = {}  # Geschoss-id -> letzte Positionen [(x, y, zeit)]
        self.kick = {}  # slot -> Zeitpunkt des letzten Schusses (Rückstoß)
        self.after = {}  # slot -> Nachbilder beim Turbo
        self.prev = {}  # slot -> Panzerzustand im letzten Bild
        self.flying = (
            []
        )  # abgesprengte Türme [x, y, vx, vy, alter, winkel, drehung, slot]
        self.box_seen = {}  # (x, y, art) -> Zeitpunkt des Auftauchens
        self.landed = set()
        self.marks = set()  # einmalige Geräusche je Geschoss (Pfeifen, Jet, Sirene ...)
        self.scores_seen, self.score_pop = {}, {}
        self.amb, self.amb_arena = [], None
        self.adds, self.lines, self.rings = [], [], []
        self.me_pos = None
        self.world_w = W
        self.quiet = False
        self.slow_seen = False
        self.demo = None
        self.veil = pygame.Surface((W, H), pygame.SRCALPHA)
        self.music = Music()
        self.lightmap = None
        self.dark = False  # Sonderregel Stromausfall: Lichter werden mitgeschrieben
        self.light_log = []  # (farbe, x, y, radius, stufe) aller Lichter dieses Bildes
        self.focus = None  # wo zuletzt ein Panzer explodiert ist (Zeitlupen-Zoom)
        self.cine = 0.0  # Kinobalken und Zoom, weich nachgeführt

    # --- Hilfen ---

    def text(self, txt, pos, font=None, color=TEXT_C, center=True, right=False):
        font = font or self.font
        key = (txt, id(font), color)
        surf = self.text_cache.get(key)
        if surf is None:
            if len(self.text_cache) > 300:
                self.text_cache.clear()
            surf = self.text_cache[key] = font.render(txt, True, color)
        if center:
            rect = surf.get_rect(center=pos)
        elif right:
            rect = surf.get_rect(topright=pos)
        else:
            rect = surf.get_rect(topleft=pos)
        self.screen.blit(surf, rect)
        return rect

    def rotated(self, idx, a, k=1.0, alpha=255):
        # auf 3° gerundet und gecacht -> rotozoom nur einmal pro Winkel statt jeden Frame
        step = round(math.degrees(a) / 3) % 120
        key = (idx, step, k, alpha)
        surf = self.rot_cache.get(key)
        if surf is None:
            surf = pygame.transform.rotozoom(self.sprites[idx], -step * 3, k)
            if alpha < 255:
                surf.set_alpha(alpha)
            self.rot_cache[key] = surf
        return surf

    def _light(self, color, x, y, r, level=8):
        """Additives Licht an (x, y)."""
        if level >= 1:
            if self.dark:
                self.light_log.append((color, x, y, r, level))
            r = light_r(r)
            self.adds.append((glow(color, r, min(8, int(level))), (x - r, y - r)))

    def _sound(self, name, x=None, vol=1.0):
        if name and not self.quiet:
            self.sfx.play(name, None if x is None else x / self.world_w, vol)

    def _shake(self, amount, x=None, y=None):
        """Wackeln. Im Netzspiel schwächer, je weiter weg vom eigenen Panzer."""
        if self.quiet:
            return
        if self.me_pos and x is not None:
            d = math.hypot(x - self.me_pos[0], y - self.me_pos[1])
            amount *= max(0.35, 1 - d / 750)
        self.trauma = min(1.0, self.trauma + amount)

    def dotted(self, pts, color):
        carry = 0.0
        for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
            seg = math.hypot(x1 - x0, y1 - y0)
            d = carry
            while d < seg:
                pygame.draw.circle(
                    self.screen,
                    color,
                    (x0 + (x1 - x0) * d / seg, y0 + (y1 - y0) * d / seg),
                    2,
                )
                d += 10
            carry = d - seg

    def _maze_surface(self, arena):
        if self.maze_arena is not arena:
            self.maze_surf, self.maze_arena = build_maze_surface(arena), arena
        return self.maze_surf

    def _new_round(self, arena, now):
        self.round_arena = arena
        self.round_start = now
        self.seen_shots.clear()
        self.seen_fx.clear()
        self.tracks.clear()
        self.hist.clear()
        self.after.clear()
        self.prev.clear()
        self.box_seen.clear()
        self.landed.clear()
        self.marks.clear()
        self.parts.items.clear()
        self.flying.clear()
        self.slow_seen = False
        self.focus = None
        if not self.quiet:
            self._sound("round")

    # --- Ereignisse: alles, was seit dem letzten Bild neu ist ---

    def _events(self, snap, me, arena, now):
        tanks = {t[7]: t for t in snap["tk"]}
        for p in snap["p"]:
            sid = p[0]
            if sid in self.seen_shots:
                continue
            self.seen_shots.add(sid)
            kind = p[1]
            self._sound(SHOT_SOUNDS.get(kind), p[2])
            tk = tanks.get(p[4])
            if kind not in ("frg", "mine", "strike") and tk and tk[3]:
                self._muzzle(tk, kind, now)
        for fid, kind, age, life, data in snap["fx"]:
            if fid in self.seen_fx:
                continue
            self.seen_fx.add(fid)
            if age > life * 0.7:  # z. B. nach dem Beitreten: Altes nicht nachholen
                continue
            if kind == "boom":
                x, y, *z = data
                z = z[0] if z else 1
                self._explosion(x, y, z)
                self._sound("bigboom" if z >= 1.2 else "boom", x)
                self._shake(0.3 * z, x, y)
            elif kind == "die":
                x, y, slot, a = data
                self.focus = (x, y)
                self._wreck_burst(x, y, slot, a)
                self._shake(0.3, x, y)
                self.flash = max(self.flash, 0.22 if slot == me else 0.07)
            elif kind == "laser":
                slot, pts = data
                self._sound("laser", pts[0][0])
                self.kick[slot] = now
                for x, y in pts[1:-1]:
                    self.parts.add(FLASH, x, y, life=0.25, size=22, color=COLORS[slot])
                ex, ey = pts[-1]
                self.parts.burst(SPARK, ex, ey, 10, 300, 0.4, 2, drag=3)
            elif kind == "zap":
                slot, pts = data
                self._sound("zap", pts[0][0])
                self.kick[slot] = now
                for x, y in pts[1:]:
                    self.parts.burst(SPARK, x, y, 12, 320, 0.35, 2, drag=3)
                    self.parts.add(
                        FLASH, x, y, life=0.3, size=40, color=(120, 190, 255)
                    )
                self.flash = max(self.flash, 0.1)
            elif kind == "tele":
                x, y = data
                self._sound("tele", x)
                self.parts.burst(STAR, x, y, 16, 150, 0.6, 2, (190, 110, 255), drag=4)
            elif kind == "pick":
                x, y, item, mystery = data
                self._sound("mystery" if mystery else "pick", x)
                c = box_color(item)
                self.parts.burst(STAR, x, y, 18, 170, 0.7, 2, c, drag=3.5)
                self.parts.add(RING, x, y, life=0.4, size=46, color=c)
            elif kind == "block":
                x, y = data
                self._sound("shield", x)
                self.parts.burst(SHARD, x, y, 16, 260, 0.6, 3, (120, 200, 255), drag=3)
                self.parts.add(FLASH, x, y, life=0.25, size=45, color=(90, 170, 255))
            elif kind == "freeze":
                x, y = data
                self._sound("freeze", x)
                self.parts.burst(SHARD, x, y, 12, 200, 0.5, 3, (200, 235, 255), drag=4)
                self.parts.burst(STAR, x, y, 12, 90, 0.9, 2, (170, 225, 255), drag=2)
            elif kind == "nuke":
                x, y = data
                self.focus = (x, y)
                self._sound("nuke", x)
                self._nuke_burst(x, y)
            elif kind == "ann":
                self._sound("ann")
        for tk in snap["tk"]:
            slot, taunt = tk[7], tk[11]
            if taunt and self.taunts.get(slot) != taunt:
                self._sound("blip", tk[0])
            self.taunts[slot] = taunt
        if snap.get("msg") and snap["msg"] != self.last_msg:
            self._sound("win")
            win = snap.get("win")
            if win and win[0] is not None and win[0] in tanks:
                tk = tanks[win[0]]
                self._confetti(tk[0], tk[1], COLORS[win[0]])
        self.last_msg = snap.get("msg")
        if snap.get("ts", 1) < 0.9 and not self.slow_seen:
            self.slow_seen = True
            self._sound("slowmo")
        for i, s in enumerate(snap["sc"]):
            if s > self.scores_seen.get(i, s):
                self.score_pop[i] = now
            self.scores_seen[i] = s

    def _muzzle(self, tk, kind, now):
        """Mündungsfeuer, Funken, Qualm und Rückstoß beim Schießen."""
        x, y, a, eff = tk[0], tk[1], tk[2], tk[10]
        k = MINI_SCALE if "mini" in eff else 1.0
        mx, my = x + math.cos(a) * MUZZLE * k, y + math.sin(a) * MUZZLE * k
        P = self.parts
        if kind == "fl":
            if random.random() < 0.35:
                vx, vy = math.cos(a) * 60, math.sin(a) * 60
                P.add(SMOKE, mx, my, vx, vy, 0.8, 5, (50, 48, 50), drag=2.5, grav=-12)
            return
        big = kind in ("rocket", "nuke", "hole", "frag", "sg")
        self.kick[tk[7]] = now
        P.add(FLASH, mx, my, life=0.09, size=30 if big else 20, color=(255, 214, 150))
        P.burst(
            SPARK, mx, my, 5 if big else 3, 280, 0.18, 2, drag=6, angle=a, spread=0.45
        )
        vx, vy = math.cos(a) * 35, math.sin(a) * 35
        P.add(
            SMOKE, mx, my, vx, vy, 0.7, 6 if big else 4, (96, 98, 106), drag=3, grav=-8
        )

    def _explosion(self, x, y, z):
        P = self.parts
        P.add(FLASH, x, y, life=0.15 + 0.06 * z, size=58 * z, color=(255, 236, 190))
        P.burst(FIRE, x, y, 8 + 9 * z, 100 * z, 0.6, 12 * z, drag=3.6)
        P.burst(SPARK, x, y, 8 + 12 * z, 420, 0.45, 2, drag=2.4)
        P.burst(SMOKE, x, y, 3 + 5 * z, 46 * z, 1.7, 11 * z, (58, 58, 64), 1.8, -16)
        P.burst(EMBER, x, y, 3 + 5 * z, 170, 1.1, 1.5, (255, 170, 70), 1.6, -24)
        P.add(RING, x, y, life=0.35 + 0.1 * z, size=70 * z, color=(255, 200, 140))
        self._scorch(x, y, 16 + 12 * z)

    def _wreck_burst(self, x, y, slot, a):
        """Panzer zerlegt: Trümmer in Flugrichtung, Funken, der Turm fliegt davon."""
        col, P = COLORS[slot], self.parts
        for _ in range(12):
            b = a + random.uniform(-1.4, 1.4)
            v = random.uniform(90, 310)
            c = mix(col, (30, 26, 24), random.uniform(0.15, 0.75))
            P.add(
                DEBRIS,
                x,
                y,
                math.cos(b) * v,
                math.sin(b) * v,
                random.uniform(1.0, 1.8),
                random.uniform(3, 7),
                c,
                drag=3.2,
                spin=random.uniform(-14, 14),
            )
        P.burst(SPARK, x, y, 24, 460, 0.55, 2, drag=2.4, angle=a, spread=1.7)
        P.burst(FIRE, x, y, 10, 80, 1.0, 15, drag=2.2)
        b = a + random.uniform(-0.7, 0.7)
        v = random.uniform(110, 170)
        spin = random.choice((-1, 1)) * random.uniform(7, 14)
        self.flying.append(
            [
                x,
                y,
                math.cos(b) * v,
                math.sin(b) * v,
                0.0,
                random.uniform(0, 6.3),
                spin,
                slot,
            ]
        )

    def _nuke_burst(self, x, y):
        P = self.parts
        self.flash = 1.0
        self.trauma = 1.0
        P.add(FLASH, x, y, life=0.6, size=260, color=(255, 250, 230))
        P.burst(FIRE, x, y, 60, 330, 1.4, 34, drag=2.6)
        P.burst(SPARK, x, y, 70, 750, 0.9, 3, drag=1.8)
        P.burst(SMOKE, x, y, 34, 230, 3.6, 30, (96, 88, 84), 1.4, -14)
        for j in range(24):  # Ring aus Rauch, der sich ausbreitet
            b = j * math.pi / 12
            P.add(
                SMOKE,
                x,
                y,
                math.cos(b) * 260,
                math.sin(b) * 260,
                3.0,
                22,
                (120, 110, 104),
                2.2,
                -6,
            )
        P.burst(EMBER, x, y, 50, 420, 2.4, 2.5, (255, 160, 60), 1.2, -30)
        for i in range(3):
            P.add(
                RING,
                x,
                y,
                life=0.8 + 0.3 * i,
                size=NUKE_BLAST * (1.3 + 0.35 * i),
                color=(255, 220, 170),
            )
        self._scorch(x, y, 110)

    def _confetti(self, x, y, color):
        cols = [color, mix(color, (255, 255, 255), 0.5), ACCENT_C, (255, 255, 255)]
        for _ in range(90):
            a = random.uniform(0, 2 * math.pi)
            v = random.uniform(80, 330)
            self.parts.add(
                CONFETTI,
                x,
                y,
                math.cos(a) * v,
                math.sin(a) * v - 120,
                random.uniform(1.6, 2.6),
                random.uniform(4, 7),
                random.choice(cols),
                drag=2.2,
                grav=55,
                spin=random.uniform(4, 12),
            )

    def _scorch(self, x, y, r):
        if self.maze_surf is not None:
            spr = scorch_sprite(light_r(r))
            self.maze_surf.blit(spr, spr.get_rect(center=(x, y)))

    def _bounce(self, x, y, kind, owner):
        if self.maze_surf is not None and self.round_arena is not None:  # Einschlagspur
            dark = shade(self.round_arena.theme["track"], 0.8)
            pygame.draw.circle(self.maze_surf, dark, (x, y), 2)
        c = (
            (255, 120, 200)
            if kind == "bnc"
            else mix(COLORS[owner], (255, 230, 160), 0.6)
        )
        self.parts.burst(SPARK, x, y, 4, 190, 0.22, 2, drag=6)
        self.parts.add(FLASH, x, y, life=0.1, size=14, color=c)
        self._sound("bnc" if kind == "bnc" else "tink", x)

    # --- Pro Bild: Geschossverläufe, Emitter, Umgebung ---

    def _track_shots(self, snap, now):
        """Merkt sich die letzten Positionen jedes Geschosses (Leuchtschweif, Abpraller)."""
        live = set()
        for p in snap["p"]:
            sid, kind, x, y = p[0], p[1], p[2], p[3]
            live.add(sid)
            h = self.hist.get(sid)
            if h is None:
                self.hist[sid] = [(x, y, now)]
                continue
            lx, ly, _ = h[-1]
            dx, dy = x - lx, y - ly
            if dx * dx + dy * dy < 0.04:
                continue
            if len(h) >= 2 and kind in BOUNCE_FX:
                ox, oy = lx - h[-2][0], ly - h[-2][1]
                if (dx * ox < 0 and abs(ox) > 0.3) or (dy * oy < 0 and abs(oy) > 0.3):
                    self._bounce(lx, ly, kind, p[4])
            h.append((x, y, now))
            while len(h) > 3 and now - h[0][2] > 0.13:
                h.pop(0)
        for sid in [s for s in self.hist if s not in live]:
            del self.hist[sid]

    def _emit(self, snap, arena, vdt, now):
        """Partikel, die laufend entstehen: Wracks brennen, Staub, Flammen, Raketenrauch ..."""
        P, th = self.parts, arena.theme
        for tk in snap["tk"]:
            x, y, a, alive, dead_age, weapon, shield, i, ammo, reload, eff, _ = tk
            prev = self.prev.get(i)
            if not alive:
                if dead_age < 3.5 and random.random() < 28 * vdt:
                    P.add(
                        FIRE,
                        x + random.uniform(-9, 9),
                        y + random.uniform(-9, 9),
                        random.uniform(-10, 10),
                        random.uniform(-40, -12),
                        random.uniform(0.35, 0.8),
                        random.uniform(6, 11),
                        drag=1,
                        grav=-20,
                    )
                if (
                    dead_age < 9
                    and random.random() < (12 if dead_age < 3.5 else 5) * vdt
                ):
                    P.add(
                        SMOKE,
                        x + random.uniform(-6, 6),
                        y + random.uniform(-6, 6),
                        random.uniform(-8, 8),
                        random.uniform(-34, -16),
                        random.uniform(1.4, 2.4),
                        random.uniform(6, 10),
                        (46, 46, 52),
                        drag=0.6,
                        grav=-6,
                    )
                self.prev[i] = tk
                continue
            k = MINI_SCALE if "mini" in eff else 1.0
            tr = TANK_R * k
            hidden = "invis" in eff and self.me is not None and i != self.me
            if prev and prev[3] and not hidden and "ghost" not in eff:
                moved = math.hypot(x - prev[0], y - prev[1])
                if 0.3 < moved < 20 and random.random() < 0.45:
                    side = random.choice((-1, 1))
                    bx = x - math.cos(a) * 14 * k - math.sin(a) * 11 * k * side
                    by = y - math.sin(a) * 14 * k + math.cos(a) * 11 * k * side
                    vx, vy = random.uniform(-14, 14), random.uniform(-14, 14)
                    P.add(
                        DUST,
                        bx,
                        by,
                        vx,
                        vy,
                        random.uniform(0.5, 0.9),
                        3.5 * k,
                        th["dust"],
                        2,
                    )
            if hidden:
                self.prev[i] = tk
                continue
            if "potato" in eff and random.random() < 22 * vdt:
                px, py = x + tr + 10, y - tr - 6
                P.add(
                    FIRE,
                    px,
                    py - 4,
                    random.uniform(-8, 8),
                    -40,
                    0.4,
                    5,
                    drag=1,
                    grav=-30,
                )
                if random.random() < 0.4:
                    P.add(SMOKE, px, py - 6, 0, -30, 0.9, 4, (70, 70, 74), 1, -10)
            if "turbo" in eff and random.random() < 50 * vdt:
                bx, by = x - math.cos(a) * (tr + 6), y - math.sin(a) * (tr + 6)
                vx, vy = -math.cos(a) * 90, -math.sin(a) * 90
                P.add(FIRE, bx, by, vx, vy, 0.25, 5 * k, drag=4)
            if "ghost" in eff and random.random() < 14 * vdt:
                P.add(
                    STAR,
                    x + random.uniform(-tr, tr),
                    y + random.uniform(-tr, tr),
                    0,
                    -20,
                    0.8,
                    2,
                    (210, 220, 255),
                    1,
                )
            if "frozen" in eff and random.random() < 12 * vdt:
                P.add(
                    STAR,
                    x + random.uniform(-tr, tr),
                    y + random.uniform(-tr, tr),
                    0,
                    -8,
                    0.9,
                    2,
                    (190, 230, 255),
                    1,
                )
            if shield > 0 and random.random() < 8 * vdt:
                b = random.uniform(0, 2 * math.pi)
                sx, sy = x + math.cos(b) * (tr + 8), y + math.sin(b) * (tr + 8)
                P.add(STAR, sx, sy, 0, 0, 0.5, 2, (110, 190, 255))
            if prev and prev[3]:
                if "frozen" in prev[10] and "frozen" not in eff:  # Eis zerspringt
                    P.burst(SHARD, x, y, 14, 220, 0.5, 3, (200, 235, 255), drag=4)
                    self._sound("freeze", x, 0.6)
                for e in ("mini", "ghost", "invis"):
                    if (e in prev[10]) != (e in eff):  # Effekt an oder aus: Puff
                        P.burst(DUST, x, y, 10, 90, 0.6, 6, (200, 200, 220), drag=3)
                if "potato" in eff:  # tickt in den letzten Sekunden
                    left = eff["potato"]
                    if left < 4 and math.ceil(left) != math.ceil(
                        prev[10].get("potato", 99)
                    ):
                        self._sound("tick", x)
            if "turbo" in eff:
                trail = self.after.setdefault(i, [])
                if not trail or now - trail[-1][3] > 0.035:
                    trail.append((x, y, a, now))
                    del trail[:-3]
            else:
                self.after.pop(i, None)
            self.prev[i] = tk
        for p in snap["p"]:
            kind, x, y = p[1], p[2], p[3]
            if kind == "rocket":
                a = p[5]
                tx, ty = x - math.cos(a) * 13, y - math.sin(a) * 13
                if random.random() < 70 * vdt:
                    vx, vy = random.uniform(-10, 10), random.uniform(-10, 10)
                    P.add(
                        SMOKE, tx, ty, vx, vy, SMOKE_LIFE * 1.4, 4, (120, 120, 126), 1.5
                    )
                if random.random() < 40 * vdt:
                    P.add(
                        FIRE,
                        tx,
                        ty,
                        -math.cos(a) * 40,
                        -math.sin(a) * 40,
                        0.18,
                        5,
                        drag=3,
                    )
            elif kind == "fl":
                if random.random() < 30 * vdt:  # jede Flamme lodert weiter
                    vx, vy = random.uniform(-25, 25), random.uniform(-35, 5)
                    P.add(FIRE, x, y, vx, vy, 0.3, 10, drag=2, grav=-40)
                if random.random() < 7 * vdt:
                    vx, vy = random.uniform(-30, 30), random.uniform(-50, -10)
                    P.add(EMBER, x, y, vx, vy, 0.7, 1.5, (255, 150, 50), 1.5, -20)
                    if random.random() < 0.5:
                        P.add(SMOKE, x, y, 0, -20, 1.0, 6, (40, 38, 40), 1.2, -14)
            elif kind == "ice" and random.random() < 35 * vdt:
                P.add(
                    STAR,
                    x,
                    y,
                    random.uniform(-20, 20),
                    random.uniform(-20, 20),
                    0.5,
                    1.6,
                    (180, 230, 255),
                    2,
                )
            elif kind in ("swap", "bnc") and random.random() < 18 * vdt:
                c = (200, 130, 255) if kind == "swap" else (255, 140, 210)
                P.add(
                    STAR,
                    x,
                    y,
                    random.uniform(-15, 15),
                    random.uniform(-15, 15),
                    0.45,
                    1.6,
                    c,
                    2,
                )
            elif kind == "nuke" and random.random() < 6 * vdt:
                P.add(
                    SPARK,
                    x,
                    y,
                    random.uniform(-80, 80),
                    random.uniform(-80, 80),
                    0.25,
                    2,
                    drag=4,
                )
        self._ambient(arena, vdt)

    def _ambient(self, arena, vdt):
        """Umgebung je nach Look: Staub, Sand, Glut, Schnee, Glühwürmchen, Neon-Pixel."""
        th = arena.theme
        kind = th["amb"]
        if self.amb_arena is not arena:
            self.amb, self.amb_arena = [], arena
        x0, y0 = arena.ox, arena.oy
        x1, y1 = x0 + arena.cols * CELL, y0 + arena.rows * CELL
        want = AMBIENT[kind] * max(1, (arena.cols * arena.rows) // 48)
        while len(self.amb) < want:
            x, y = random.uniform(x0, x1), random.uniform(y0, y1)
            if kind == "sand":
                v = (random.uniform(50, 110), random.uniform(-8, 8))
            elif kind == "ember":
                v = (random.uniform(-10, 10), random.uniform(-40, -15))
            elif kind == "snow":
                v = (random.uniform(5, 25), random.uniform(20, 50))
            elif kind == "neon":
                v = (0, random.uniform(-25, -8))
            else:
                v = (random.uniform(-8, 8), random.uniform(-8, 8))
            self.amb.append(
                [x, y, *v, 0.0, random.uniform(3, 8), random.uniform(0, 6.3)]
            )
        keep = []
        for p in self.amb:
            p[4] += vdt
            p[0] += p[2] * vdt
            p[1] += p[3] * vdt
            if kind == "firefly":  # schwirren herum
                p[2] += math.sin(p[4] * 1.7 + p[6]) * 30 * vdt
                p[3] += math.cos(p[4] * 1.3 + p[6]) * 30 * vdt
            if p[4] < p[5] and x0 - 20 < p[0] < x1 + 20 and y0 - 20 < p[1] < y1 + 20:
                keep.append(p)
        self.amb = keep

    def _draw_ambient(self, low):
        th = self.round_arena.theme if self.round_arena else THEMES[0]
        kind = th["amb"]
        for x, y, _, _, age, life, ph in self.amb:
            fade = min(1.0, age / 0.8, (life - age) / 0.8)
            if kind in ("ember", "firefly", "neon"):
                if low:
                    continue
                if kind == "firefly":
                    fade *= max(0.0, math.sin(age * 2.2 + ph))
                c = {
                    "ember": (255, 120, 40),
                    "firefly": (190, 255, 90),
                    "neon": th["accent"] if int(ph * 10) % 2 else th["hi"],
                }[kind]
                self._light(c, x, y, 5 if kind != "neon" else 4, 7 * fade)
            elif low == (kind != "snow"):
                c = (235, 245, 255) if kind == "snow" else th["dust"]
                a = int((150 if kind == "snow" else 70) * fade) // 30 * 30
                if a > 0:
                    r = 2 if kind == "snow" else 1
                    self.screen.blit(puff(r, a, c), (x - r - 1, y - r - 1))

    def _update_flying(self, dt, maze, arena):
        keep = []
        x0, y0 = arena.ox + 10, arena.oy + 10
        x1, y1 = arena.ox + arena.cols * CELL - 10, arena.oy + arena.rows * CELL - 10
        for f in self.flying:
            f[4] += dt
            f[0] += f[2] * dt
            f[1] += f[3] * dt
            f[5] += f[6] * dt
            if not x0 < f[0] < x1:  # prallt an der Außenwand ab
                f[2], f[0] = -f[2], min(max(f[0], x0), x1)
            if not y0 < f[1] < y1:
                f[3], f[1] = -f[3], min(max(f[1], y0), y1)
            if f[4] < TURRET_AIR:
                keep.append(f)
                continue
            x, y = push_out(f[0], f[1], 9, arena.near(f[0], f[1]))
            spr = pygame.transform.rotozoom(
                self.turret_wrecks[f[7]], -math.degrees(f[5]), 1
            )
            maze.blit(spr, spr.get_rect(center=(x, y)))
            self.parts.burst(DUST, x, y, 8, 70, 0.7, 6, arena.theme["dust"], drag=3)
            self.parts.burst(SPARK, x, y, 5, 160, 0.25, 2, drag=5)
            self._sound("clank", x)
        self.flying = keep

    # --- Zeichnen: Partikel ---

    def _collect(self):
        """Sortiert die Partikel: unter den Panzern, darüber, ins Licht."""
        low, high, polys_low, polys_high = [], [], [], []
        lines, rings = self.lines, self.rings
        for p in self.parts.items:
            kind = p[6]
            k = p[4] / p[5]
            x, y = p[0], p[1]
            if kind == FIRE:
                self._light(
                    FIRE_GRAD[min(6, int(k * 7))], x, y, p[7] * (0.55 + 0.9 * k), 7
                )
            elif kind == SPARK:
                c = FIRE_GRAD[min(5, int(k * 6))]
                lines.append((c, (x, y), (x - p[2] * 0.03, y - p[3] * 0.03), 2))
            elif kind == SMOKE:
                r = max(2, int(p[7] * (0.6 + 1.1 * k)))
                a = int(150 * (1 - k) * min(1.0, k * 8 + 0.25)) // 15 * 15
                if a > 0:
                    high.append((puff(r, a, p[8]), (x - r - 1, y - r - 1)))
            elif kind in (EMBER, STAR):
                tw = 1.0 if kind == EMBER else 0.55 + 0.45 * math.sin(p[4] * 18 + p[11])
                self._light(p[8], x, y, p[7] * 2.2 + 2, int(8 * (1 - k) * tw))
            elif kind == FLASH:
                self._light(p[8], x, y, p[7] * (0.8 + 0.4 * k), int(8 * (1 - k) ** 1.5))
            elif kind == DUST:
                r = max(1, int(p[7] * (0.6 + 1.0 * k)))
                a = int(100 * (1 - k)) // 20 * 20
                if a > 0:
                    low.append((puff(r, a, p[8]), (x - r - 1, y - r - 1)))
            elif kind == DEBRIS:
                polys_low.append((p[8], rot_rect(x, y, p[7], p[7] * 0.65, p[11])))
            elif kind == SHARD:
                c = shade(p[8], 1 - k)
                lines.append((c, (x, y), (x - p[2] * 0.035, y - p[3] * 0.035), 2))
            elif kind == CONFETTI:
                w = p[7] * (0.25 + 0.75 * abs(math.cos(p[4] * 7 + p[12])))
                polys_high.append((p[8], rot_rect(x, y, p[7], w * 0.6, p[11])))
            elif kind == RING:
                rings.append(
                    (
                        shade(p[8], (1 - k) ** 1.2),
                        (x, y),
                        p[7] * ease_out(k),
                        max(1, round(4 * (1 - k))),
                    )
                )
        return low, polys_low, high, polys_high

    def _flush_light(self):
        """Linien und Ringe kommen in einen Lichtpuffer, der additiv aufs Bild gelegt wird."""
        scr = self.screen
        if self.adds:
            scr.fblits(self.adds, pygame.BLEND_ADD)
        if self.lines or self.rings:
            light = self.light
            light.fill((0, 0, 0))
            for c, p0, p1, w in self.lines:
                pygame.draw.line(light, c, p0, p1, w)
            for c, center, r, w in self.rings:
                if r >= w:
                    pygame.draw.circle(light, c, center, r, w)
            scr.blit(light, (0, 0), special_flags=pygame.BLEND_ADD)
        self.adds, self.lines, self.rings = [], [], []

    def _polyline_light(self, color, pts, width):
        """Dicke, leuchtende Linie (Laser, Blitz) über den Lichtpuffer."""
        for p0, p1 in zip(pts, pts[1:]):
            self.lines.append((color, p0, p1, width))
            if (
                self.dark and width > 4
            ):  # Stromausfall: die Linie beleuchtet ihre Umgebung
                d = math.hypot(p1[0] - p0[0], p1[1] - p0[1])
                for j in range(int(d // 40) + 1):
                    f = j * 40 / d if d else 0
                    x, y = p0[0] + (p1[0] - p0[0]) * f, p0[1] + (p1[1] - p0[1]) * f
                    self.light_log.append((color, x, y, 30, 6))

    # --- Zeichnen: Spielwelt ---

    def _draw_boxes(self, snap, now):
        scr = self.screen
        live = set()
        for x, y, kind in snap["bx"]:
            key = (x, y, kind)
            live.add(key)
            born = self.box_seen.setdefault(key, now)
            k = (now - born) / 0.45
            col = box_color(kind)
            if k < 1:  # fällt von oben herein
                z = 1 + 2.2 * (1 - ease_out(k))
                sh = shadow_sprite(round(13 / z) + 2)
                scr.blit(sh, sh.get_rect(center=(x + 4, y + 6)))
                spr = pygame.transform.rotozoom(self.boxes[kind], (1 - k) * 90, z)
                spr.set_alpha(int(255 * min(1, k * 3)))
                scr.blit(spr, spr.get_rect(center=(x, y - 40 * (1 - ease_out(k)))))
                continue
            if key not in self.landed:
                self.landed.add(key)
                self.parts.burst(DUST, x, y, 12, 110, 0.6, 6, (170, 170, 180), drag=4)
                self.parts.add(RING, x, y, life=0.35, size=40, color=col)
                self._sound("drop", x)
            pulse = 0.5 + 0.5 * math.sin(now * 4 + x * 0.05)
            self._light(col, x, y, 34, 2 + 3 * pulse)
            bob = math.sin(now * 3 + x * 0.1) * 2
            sh = shadow_sprite(13)
            scr.blit(sh, sh.get_rect(center=(x + 3, y + 6)))
            spr = self.boxes[kind]
            scr.blit(spr, spr.get_rect(center=(x, y + bob - 2)))
            if kind == "mystery" and random.random() < 0.08:
                self.parts.add(
                    STAR,
                    x + random.uniform(-14, 14),
                    y + random.uniform(-14, 14),
                    0,
                    -12,
                    0.6,
                    1.6,
                    ACCENT_C,
                )
        for key in [k for k in self.box_seen if k not in live]:
            del self.box_seen[key]
            self.landed.discard(key)

    def _trail(self, sid, color, r, level=6):
        """Leuchtschweif aus den letzten Positionen eines Geschosses."""
        h = self.hist.get(sid)
        if not h or len(h) < 2:
            return
        pts = h[-10:]
        m = len(pts)
        for j, (hx, hy, _) in enumerate(pts[:-1]):
            f = (j + 1) / m
            self._light(color, hx, hy, r * (0.45 + 0.55 * f), level * f)

    def _draw_shots(self, snap, me, now, arena):
        scr = self.screen
        bullet_r = bullet_params(arena.mut)[1]
        for p in snap["p"]:
            sid, kind, x, y, owner = p[:5]
            extra = p[5:]
            col = COLORS[owner]
            if kind in ("b", "mg", "sg", "frg"):
                r = {"b": bullet_r, "mg": MG_R, "sg": 3}.get(kind, 2)
                hot = (
                    col
                    if kind == "b"
                    else (255, 190, 90) if kind != "mg" else (255, 220, 120)
                )
                bright = mix(hot, (255, 255, 255), 0.35)
                self._trail(sid, bright, r + 3, 5 if kind == "b" else 4)
                self._light(bright, x, y, r * 2.5 + 8, 6)
                pygame.draw.circle(scr, (14, 14, 18), (x, y), r + 1)
                pygame.draw.circle(scr, mix(hot, (255, 255, 255), 0.6), (x, y), r)
                if r > 2:
                    pygame.draw.circle(
                        scr, (255, 255, 255), (x - r * 0.3, y - r * 0.3), max(1, r // 2)
                    )
            elif kind == "frag":
                age = extra[0]
                blink = int(age * (3 + 12 * age / FRAG_FUSE)) % 2
                pygame.draw.circle(scr, (14, 14, 18), (x, y), FRAG_R + 1)
                pygame.draw.circle(scr, (62, 64, 74), (x, y), FRAG_R)
                for j in range(6):
                    b = j * math.pi / 3 + age * 2
                    pygame.draw.circle(
                        scr,
                        (110, 112, 124),
                        (x + math.cos(b) * FRAG_R, y + math.sin(b) * FRAG_R),
                        2,
                    )
                pygame.draw.circle(scr, (150, 152, 166), (x - 2, y - 2), 2)
                pygame.draw.circle(
                    scr, (255, 60, 60) if blink else (90, 20, 20), (x, y), 2
                )
                if blink:
                    self._light((255, 50, 40), x, y, 22, 6)
                self._light(col, x, y, 18, 2)
            elif kind == "rocket":
                a = extra[0]
                flame = 9 + 4 * math.sin(now * 45 + sid)
                draw_rocket(scr, x, y, a, col, 1.0, flame)
                fx, fy = x - math.cos(a) * 16, y - math.sin(a) * 16
                self._light((255, 140, 50), fx, fy, 30, 6)
                self._light((255, 230, 160), fx, fy, 10, 8)
            elif kind == "bnc":
                self._trail(sid, (255, 90, 190), 9, 5)
                self._light((255, 80, 180), x, y, 22, 5)
                pygame.draw.circle(scr, (14, 14, 18), (x, y), BOUNCY_R + 1)
                pygame.draw.circle(scr, (240, 80, 170), (x, y), BOUNCY_R)
                pygame.draw.circle(scr, (255, 210, 235), (x - 2, y - 2), 2)
            elif kind == "swap":
                self._trail(sid, (170, 90, 255), 9, 5)
                self._light((160, 70, 255), x, y, 24, 6)
                pygame.draw.circle(scr, (150, 60, 210), (x, y), SWAP_R)
                pygame.draw.circle(scr, (230, 200, 255), (x, y), 2)
                rr = SWAP_R + 4 + 2 * math.sin(now * 20)
                self.rings.append(((150, 90, 255), (x, y), rr, 1))
            elif kind == "ice":
                self._trail(sid, (120, 210, 255), 8, 5)
                self._light((90, 190, 255), x, y, 20, 6)
                pts = rot_rect(x, y, 12, 6, now * 9 + sid)
                pygame.draw.polygon(scr, (170, 225, 255), pts)
                pygame.draw.polygon(scr, (240, 252, 255), pts, 1)
            elif kind == "rang":
                a = now * 18 + sid
                self._light((255, 170, 80), x, y, 16, 2)
                for da in (0, 2.0):
                    end = (x + math.cos(a + da) * 9, y + math.sin(a + da) * 9)
                    pygame.draw.line(scr, (120, 70, 25), (x, y), end, 6)
                    pygame.draw.line(scr, (200, 130, 55), (x, y), end, 3)
            elif kind == "mine":
                age = extra[0]
                if age < MINE_ARM or me is None or owner == me:
                    alpha = 255  # eigene Minen und scharf werdende sieht man
                else:
                    alpha = 28  # die anderen nur, wenn sie genau hinschauen
                lit = age > MINE_ARM and int(now * 3 + sid) % 2
                mine = pygame.Surface((24, 24), pygame.SRCALPHA)
                draw_icon(mine, "mine", 12, 12, 0.95)
                if lit:
                    pygame.draw.circle(mine, (255, 120, 120), (12, 12), 3)
                pygame.draw.circle(mine, col, (12, 12), 10, 1)
                mine.set_alpha(alpha)
                scr.blit(mine, (x - 12, y - 12))
                if lit and alpha == 255:
                    self._light((255, 40, 30), x, y, 14, 5)
            elif kind == "hole":
                self._draw_hole(x, y, extra[0], now)
            elif kind == "strike":
                self._draw_strike(sid, x, y, extra[0], now)
            elif kind == "fl":
                k = min(1.0, extra[0] / FLAME_LIFE)
                c = FIRE_GRAD[min(6, int(k * 5.5))]
                self._light(c, x, y, 12 + 34 * k, 8 - 3 * k)
                if k < 0.5:
                    self._light((255, 240, 200), x, y, 8 + 10 * k, 6)
            elif kind == "nuke":
                self._draw_nuke(sid, x, y, extra[0], now)

    def _draw_hole(self, x, y, age, now):
        scr = self.screen
        if age < HOLE_TRAVEL:
            self._light((150, 60, 255), x, y, 22, 5)
            pygame.draw.circle(scr, (6, 4, 10), (x, y), HOLE_R)
            pygame.draw.circle(scr, (170, 90, 255), (x, y), HOLE_R, 2)
            return
        g = min(1, (age - HOLE_TRAVEL) / 0.4, (HOLE_TRAVEL + HOLE_TIME - age) / 0.4)
        if g <= 0:
            return
        self._light((90, 30, 170), x, y, 130 * g, 8)
        self._light((200, 120, 255), x, y, 50 * g, 6)
        self.rings.append(((70, 40, 110), (x, y), HOLE_RANGE * g, 1))
        for j in range(26):  # Teilchen, die spiralförmig hineinstürzen
            ph = (now * 0.55 + j / 26) % 1
            rr = HOLE_RANGE * 0.85 * (1 - ph) * g
            b = j * 2.39 + now * (1.5 + 5 * ph)
            self._light(
                (210, 150, 255), x + math.cos(b) * rr, y + math.sin(b) * rr, 4, 7 * ph
            )
        for j in range(3):
            rr = (16 + 11 * j) * g
            rect = pygame.Rect(0, 0, 2 * rr, 2 * rr)
            rect.center = (x, y)
            a0 = now * (6 - j) + j * 2
            pygame.draw.arc(scr, (190, 110, 255), rect, a0, a0 + 2.2, 2)
        pygame.draw.circle(scr, (2, 1, 4), (x, y), max(1, round(HOLE_KILL * g)))
        pygame.draw.circle(
            scr, (230, 190, 255), (x, y), max(1, round(HOLE_KILL * g)), 1
        )

    def _draw_strike(self, sid, x, y, age, now):
        scr = self.screen
        k = min(age / STRIKE_DELAY, 1)
        blink = int(age * (4 + 14 * k)) % 2
        c = (255, 50, 50) if blink else (255, 150, 150)
        self._light((255, 30, 20), x, y, STRIKE_BLAST, 2 + 3 * blink)
        pygame.draw.circle(scr, c, (x, y), STRIKE_BLAST, 2)
        pygame.draw.circle(scr, c, (x, y), max(4, round(STRIKE_BLAST * (1 - k))), 2)
        for j in range(4):
            b = now * 2 + j * math.pi / 2
            p0 = (
                x + math.cos(b) * (STRIKE_BLAST - 9),
                y + math.sin(b) * (STRIKE_BLAST - 9),
            )
            p1 = (
                x + math.cos(b) * (STRIKE_BLAST + 9),
                y + math.sin(b) * (STRIKE_BLAST + 9),
            )
            pygame.draw.line(scr, c, p0, p1, 3)
        pygame.draw.line(scr, c, (x - 12, y), (x + 12, y), 2)
        pygame.draw.line(scr, c, (x, y - 12), (x, y + 12), 2)
        # Der Jet überfliegt das Ziel, danach fällt die Bombe (ihr Schatten wird größer)
        pk = (age - STRIKE_DELAY * 0.15) / (STRIKE_DELAY * 0.6)
        if 0 <= pk <= 1:
            ang = (sid * 2.39) % (2 * math.pi)
            d = (pk - 0.5) * 1100
            self.jets.append((x + math.cos(ang) * d, y + math.sin(ang) * d, ang))
            if (sid, "jet") not in self.marks:
                self.marks.add((sid, "jet"))
                self._sound("jet", x)
        if age > STRIKE_DELAY - 0.95 and (sid, "pfeif") not in self.marks:
            self.marks.add((sid, "pfeif"))
            self._sound("whistle", x)
        if age > STRIKE_DELAY - 0.45:
            fk = (age - (STRIKE_DELAY - 0.45)) / 0.45
            sh = puff(
                max(2, round(3 + 10 * fk)), int(60 + 150 * fk) // 15 * 15, (8, 8, 8)
            )
            scr.blit(sh, sh.get_rect(center=(x, y)))

    def _draw_nuke(self, sid, x, y, age, now):
        scr = self.screen
        left = NUKE_FUSE - age
        fast = left < 1.2
        blink = int(age * (6 if fast else 2.5)) % 2
        warn = (255, 40, 30) if blink else (255, 170, 60)
        pulse = 0.5 + 0.5 * math.sin(now * (14 if fast else 6))
        self._light((255, 40, 20), x, y, NUKE_BLAST * 0.9, 1 + 2 * pulse)
        pygame.draw.circle(scr, warn, (x, y), NUKE_BLAST, 2)
        for j in range(24):  # gestrichelter Warnkreis, der sich dreht
            if j % 2:
                continue
            b0 = j * math.pi / 12 + now * 0.6
            rect = pygame.Rect(0, 0, 2 * NUKE_BLAST - 16, 2 * NUKE_BLAST - 16)
            rect.center = (x, y)
            pygame.draw.arc(scr, warn, rect, b0, b0 + math.pi / 14, 3)
        sh = shadow_sprite(12)
        scr.blit(sh, sh.get_rect(center=(x + 3, y + 5)))
        draw_icon(scr, "nuke", round(x), round(y), 1.2)
        self._light((255, 220, 60), x, y, 26, 4 + 3 * blink)
        n = max(1, math.ceil(left))
        num = fancy(str(n), 30, (255, 80, 60) if fast else ACCENT_C)
        scr.blit(num, num.get_rect(center=(x, y - 30)))
        if (sid, n) not in self.marks:
            self.marks.add((sid, n))
            self._sound("siren", x)

    def _tank_alpha(self, tk, me, now):
        """Deckkraft eines Panzers (Tarnkappe, Geist, Einblenden am Rundenstart)."""
        eff, i = tk[10], tk[7]
        alpha = 255
        if "invis" in eff:
            if me is not None and i != me:
                return 0  # für die anderen ganz weg
            alpha = 35 if me is None else 100
        if "ghost" in eff:
            g = eff["ghost"]
            alpha = min(alpha, 110 if g > 1.2 or int(g * 8) % 2 else 220)
        spawn = now - self.round_start
        if spawn < 0.5:
            alpha = min(alpha, int(255 * max(0.2, spawn / 0.5)))
        return alpha

    def _draw_tank(self, tk, me, now):
        scr = self.screen
        x, y, a, alive, dead_age, weapon, shield, i, ammo, reload, eff, taunt = tk
        if not alive:
            rot = self.rotated(i + DEAD_SPRITE, a)
            scr.blit(rot, rot.get_rect(center=(x, y)))
            return
        alpha = self._tank_alpha(tk, me, now)
        if alpha == 0:
            return
        k = MINI_SCALE if "mini" in eff else 1.0
        tr = TANK_R * k
        kick = self.kick.get(i)
        if kick is not None and 0 <= now - kick < 0.14:
            off = 3.5 * (1 - (now - kick) / 0.14)
            x, y = x - math.cos(a) * off, y - math.sin(a) * off
        spawn = now - self.round_start
        if spawn < 0.9:  # Lichtsäule beim Erscheinen
            f = 1 - spawn / 0.9
            self._light(COLORS[i], x, y, 46, 8 * f)
            self.rings.append((shade(COLORS[i], f), (x, y), 14 + 40 * (1 - f), 2))
        if alpha > 90:
            sh = shadow_sprite(round(17 * k))
            scr.blit(sh, sh.get_rect(center=(x + 3, y + 5)))
        if "turbo" in eff:
            for j, (hx, hy, ha, _) in enumerate(self.after.get(i, ())):
                ghost = self.rotated(i, ha, k, 45 + 30 * j)
                scr.blit(ghost, ghost.get_rect(center=(hx, hy)))
            for c, rr in (((255, 140, 25), 5), ((255, 235, 120), 2.5)):
                fl = tr + 4 + 2 * math.sin(now * 40 + i)
                pos = (x - math.cos(a) * fl, y - math.sin(a) * fl)
                pygame.draw.circle(scr, c, pos, rr * k + 1)
            self._light(
                (255, 150, 50),
                x - math.cos(a) * (tr + 6),
                y - math.sin(a) * (tr + 6),
                20,
                5,
            )
        rot = self.rotated(i, a, k, alpha)
        scr.blit(rot, rot.get_rect(center=(x, y)))
        if alpha < 100:
            return
        if self.dark:  # Scheinwerfer vorne am Panzer
            fx, fy = x + math.cos(a) * 15 * k, y + math.sin(a) * 15 * k
            self._light((255, 245, 215), fx, fy, 12, 6)
        if "frozen" in eff:
            ice = self.ice[k]
            scr.blit(ice, ice.get_rect(center=(x, y)))
            self._light((120, 200, 255), x, y, 30, 3)
        if shield > 0 and (shield > 2 or int(shield * 6) % 2):  # blinkt kurz vor Ablauf
            self._light((40, 110, 255), x, y, tr + 22, 3)
            self.rings.append(((60, 150, 255), (x, y), tr + 8, 3))
            pygame.draw.circle(scr, (170, 220, 255), (x, y), tr + 8, 1)
            for j in range(6):  # Wabenmuster wandert über den Schild
                b = now * 1.6 + j * math.pi / 3
                self._light(
                    (140, 210, 255),
                    x + math.cos(b) * (tr + 8),
                    y + math.sin(b) * (tr + 8),
                    5,
                    6,
                )
        if "ghost" in eff:
            self._light((180, 190, 255), x, y, 30, 2)

    def _draw_tank_ui(self, tk, arena, me, leader, now, bots):
        """Alles über dem Panzer: Waffe, Krone, Kartoffel, Sprüche, Nachladen, Markierung."""
        scr = self.screen
        x, y, a, alive, dead_age, weapon, shield, i, ammo, reload, eff, taunt = tk
        if not alive or self._tank_alpha(tk, me, now) < 100:
            return  # getarnt: nichts am Panzer, das ihn verraten würde
        k = MINI_SCALE if "mini" in eff else 1.0
        tr = TANK_R * k
        if reload > 0 and not weapon:
            # lokal: Sekunden bei allen, im Netz nur beim eigenen Panzer
            life = bullet_params(arena.mut)[2]
            self.reload_ring(x, y, reload, me is None or i == me, life, tr, COLORS[i])
        if "confuse" in eff:
            for j in range(3):
                b = now * 5 + j * 2.09
                sx, sy = x + math.cos(b) * 12, y - tr - 10 + math.sin(b) * 3
                pts = [
                    (
                        sx + math.cos(now * 6 + q * 1.257) * (4 if q % 2 == 0 else 1.8),
                        sy + math.sin(now * 6 + q * 1.257) * (4 if q % 2 == 0 else 1.8),
                    )
                    for q in range(10)
                ]
                pygame.draw.polygon(scr, (255, 220, 40), pts)
        if i == leader:
            draw_crown(scr, x, y - tr - 4, 0.9)
            if int(now * 1.3 + i) % 4 == 0:  # Glitzern
                self._light((255, 230, 120), x, y - tr - 9, 7, 6)
        if weapon:
            bx, by = int(x), int(y - tr - 17)
            pygame.draw.circle(scr, (14, 15, 19), (bx, by), 12)
            pygame.draw.circle(scr, (228, 232, 240), (bx, by), 11)
            pygame.draw.circle(scr, box_color(weapon), (bx, by), 11, 2)
            draw_icon(scr, weapon, bx, by, 0.72)
            if i == me and weapon == "laser":  # Zielhilfe
                self.dotted(arena.raycast(x, y, a, 320)[0], (255, 90, 90))
            elif i == me and weapon == "strike":
                tx, ty = strike_target(arena, x, y, a)
                self.dotted([(x, y), (tx, ty)], (255, 90, 90))
                pygame.draw.circle(scr, (255, 90, 90), (tx, ty), STRIKE_BLAST, 1)
            elif i == me and weapon == "zap":
                rect = pygame.Rect(0, 0, 2 * ZAP_RANGE, 2 * ZAP_RANGE)
                rect.center = (x, y)
                pygame.draw.arc(
                    scr, (90, 160, 255), rect, -a - ZAP_CONE, -a + ZAP_CONE, 1
                )
        if "potato" in eff:
            left = eff["potato"]
            jig = math.sin(now * 40) * 2 if left < 3 else 0
            px, py = x + tr + 10 + jig, y - tr - 6
            draw_icon(scr, "potato", int(px), int(py), 1.0)
            pygame.draw.circle(
                scr, (255, 200, 40), (px + 9, py - 6), 2 + int(now * 12) % 2
            )
            self._light((255, 120, 30), px, py, 22, 4 + (3 if left < 3 else 0))
            num = fancy(
                f"{math.ceil(left)}", 22, (255, 80, 60) if left < 3 else TEXT_C, 2
            )
            scr.blit(num, num.get_rect(center=(px, py - 17)))
        if taunt:
            txt = TAUNTS[taunt - 1]
            surf = self.small.render(txt, True, INK_C)
            box = surf.get_rect(center=(x, y - tr - 40)).inflate(16, 9)
            pygame.draw.rect(scr, (14, 15, 19), box.inflate(2, 2), border_radius=9)
            pygame.draw.rect(scr, (250, 250, 252), box, border_radius=8)
            pygame.draw.rect(scr, COLORS[i], box, 2, border_radius=8)
            pygame.draw.polygon(
                scr,
                COLORS[i],
                [(x - 5, box.bottom), (x + 5, box.bottom), (x, box.bottom + 7)],
            )
            scr.blit(surf, surf.get_rect(center=box.center))
        spawn = now - self.round_start
        if spawn < 2.6:  # am Rundenstart: wer ist wer?
            fade = min(1.0, (2.6 - spawn) / 0.5)
            bob = math.sin(now * 7) * 3
            if me is not None and i == me:
                lbl = fancy("DU", 24, mix(COLORS[i], (255, 255, 255), 0.4), 2)
                lbl.set_alpha(int(255 * fade))
                if y - tr - 60 > 0:  # Pfeil von oben auf den Panzer
                    ty = y - tr - 30 + bob
                    pts = [(x - 9, ty - 8), (x + 9, ty - 8), (x, ty + 4)]
                    pos = lbl.get_rect(midbottom=(x, ty - 9))
                else:  # Panzer am oberen Rand: Pfeil von unten
                    ty = y + tr + 30 - bob
                    pts = [(x - 9, ty + 8), (x + 9, ty + 8), (x, ty - 4)]
                    pos = lbl.get_rect(midtop=(x, ty + 9))
                pygame.draw.polygon(scr, (14, 15, 19), [(px, py + 1) for px, py in pts])
                pygame.draw.polygon(scr, COLORS[i], pts)
                scr.blit(lbl, pos)
            elif me is None and not self.quiet:
                name = NAMES[i] + (" (Bot)" if i in bots else "")
                lbl = fancy(name, 18, mix(COLORS[i], (255, 255, 255), 0.35), 2, False)
                lbl.set_alpha(int(255 * fade))
                scr.blit(lbl, lbl.get_rect(midbottom=(x, y - tr - 12 + bob)))

    def reload_ring(self, x, y, reload, show_time, life, tr, color=(90, 90, 90)):
        """Ring, der sich leert, bis wieder eine Kugel frei ist."""
        frac = min(reload / life, 1)
        rect = pygame.Rect(0, 0, 2 * tr + 20, 2 * tr + 20)
        rect.center = (round(x), round(y))
        pygame.draw.circle(self.screen, (60, 64, 76), rect.center, rect.w // 2, 2)
        pygame.draw.arc(
            self.screen,
            color,
            rect,
            math.pi / 2,
            math.pi / 2 + 2 * math.pi * frac,
            3,
        )
        if show_time:
            num = fancy(f"{reload:.1f}", 18, TEXT_C, 2, False)
            self.screen.blit(num, num.get_rect(center=(x, y - tr - 22)))

    def _draw_fx(self, snap, now):
        """Effekte, die der Host verschickt, zeichnen sich nach ihrem Alter."""
        for fid, kind, age, life, data in snap["fx"]:
            k = min(age / life, 1)
            if kind == "boom":
                x, y, *z = data
                z = z[0] if z else 1
                self._light(
                    (255, 150, 60),
                    x,
                    y,
                    (20 + 44 * ease_out(k)) * z,
                    8 * (1 - k) ** 1.4,
                )
                self._light((255, 244, 210), x, y, (10 + 18 * k) * z, 8 * (1 - k) ** 2)
            elif kind == "laser":
                slot, pts = data
                if len(pts) < 2:
                    continue
                f = (1 - k) ** 0.8
                col = mix(COLORS[slot], (255, 60, 60), 0.35)
                self._polyline_light(shade(col, 0.45 * f), pts, 11)
                self._polyline_light(shade(col, f), pts, 6)
                self._polyline_light(shade((255, 255, 255), f), pts, 2)
                for x, y in pts[1:]:
                    self._light(col, x, y, 26, 6 * f)
            elif kind == "zap":
                slot, pts = data
                if int(age * 30) % 4 == 3:  # flackert
                    continue
                f = 1 - k
                rnd = random.Random(fid * 31 + int(age * 24))
                for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
                    bolt = bolt_points(x0, y0, x1, y1, rnd)
                    self._polyline_light(shade((60, 120, 255), f), bolt, 7)
                    self._polyline_light(shade((150, 210, 255), f), bolt, 3)
                    self._polyline_light(shade((255, 255, 255), f), bolt, 1)
                    for _ in range(2):  # Seitenäste
                        j = rnd.randrange(len(bolt))
                        bx, by = bolt[j]
                        b = rnd.uniform(0, 2 * math.pi)
                        end = (bx + math.cos(b) * 22, by + math.sin(b) * 22)
                        self._polyline_light(
                            shade((120, 180, 255), f),
                            bolt_points(bx, by, *end, rnd, 2),
                            2,
                        )
                for x, y in pts:
                    self._light((110, 170, 255), x, y, 40, 7 * f)
            elif kind == "tele":
                x, y = data
                self._light((170, 80, 255), x, y, 40, 7 * (1 - k))
                for j in range(2):
                    r = max(2, round((28 - 20 * k) * (1 - 0.4 * j)))
                    self.rings.append((shade((190, 110, 255), 1 - k), (x, y), r, 2))
            elif kind == "pick" and k < 0.3:
                x, y, item = data[:3]
                self._light(box_color(item), x, y, 40, 6 * (1 - k / 0.3))
            elif kind == "block":
                x, y = data
                self.rings.append(
                    (shade((90, 170, 255), 1 - k), (x, y), 16 + 30 * ease_out(k), 3)
                )
            elif kind == "nuke":
                x, y = data
                e = ease_out(min(1, k * 3))
                self._light((255, 140, 50), x, y, 60 + 300 * e, 8 * (1 - k))
                self._light(
                    (255, 250, 225), x, y, 40 + 120 * min(1, k * 4), 8 * (1 - k) ** 2
                )
                rr = NUKE_BLAST * 1.6 * ease_out(min(1, k * 2.2))
                self.rings.append(
                    (
                        shade((255, 230, 180), 1 - k),
                        (x, y),
                        rr,
                        max(1, round(8 * (1 - k))),
                    )
                )

    def _draw_fx_ui(self, snap, now):
        """Schwebende Texte: aufgehobenes Power-up, Eigentor, Block."""
        scr = self.screen
        for fid, kind, age, life, data in snap["fx"]:
            k = min(age / life, 1)
            if kind == "pick":
                x, y, item, mystery = data
                if mystery and age < 0.5:  # Glücksrad
                    shown = POWERUPS[int(age * 22 + fid) % len(POWERUPS)]
                    spr = self.boxes.get(shown)
                    if spr:
                        scr.blit(spr, spr.get_rect(center=(x, y - 40)))
                    continue
                name = POWERUP_NAMES.get(item, item)
                if mystery:
                    spr = self.boxes.get(item)
                    if spr and age < 0.9:
                        z = ease_back((age - 0.5) / 0.25)
                        big = pygame.transform.rotozoom(spr, 0, max(0.1, z * 1.3))
                        scr.blit(big, big.get_rect(center=(x, y - 40)))
                surf = fancy(
                    name + "!",
                    24,
                    mix(box_color(item), (255, 255, 255), 0.25),
                    2,
                    False,
                )
                surf.set_alpha(int(255 * min(1, 2.5 * (1 - k))))
                scr.blit(
                    surf,
                    surf.get_rect(center=(x, y - 32 - 26 * k - (18 if mystery else 0))),
                )
            elif kind == "pop":
                x, y, txt, slot = data
                z = ease_back(min(1, age / 0.25))
                surf = fancy(txt, 30, (255, 90, 70), 3)
                if z != 1:
                    surf = pygame.transform.rotozoom(surf, 0, max(0.05, z))
                surf.set_alpha(int(255 * min(1, 3 * (1 - k))))
                scr.blit(surf, surf.get_rect(center=(x, y - 30 * k)))
            elif kind == "block":
                x, y = data
                surf = fancy("BLOCK!", 22, (120, 200, 255), 2)
                surf.set_alpha(int(255 * min(1, 3 * (1 - k))))
                scr.blit(surf, surf.get_rect(center=(x, y - 30 - 22 * k)))

    def _draw_flying(self):
        scr = self.screen
        for x, y, _, _, age, a, _, slot in self.flying:
            k = age / TURRET_AIR
            h = 4 * k * (1 - k)  # Höhe 0..1..0
            sh = shadow_sprite(9)
            scr.blit(sh, sh.get_rect(center=(x + 6 + 14 * h, y + 6 + 22 * h)))
            spr = pygame.transform.rotozoom(
                self.turrets[slot], -math.degrees(a), 1 + 0.6 * h
            )
            scr.blit(spr, spr.get_rect(center=(x, y - 30 * h)))

    def _stamp_tracks(self, surf, tanks, arena):
        """Kettenspuren direkt auf die Labyrinth-Fläche malen (bleiben bis zur nächsten Runde)."""
        col = arena.theme["track"]
        for tk in tanks:
            x, y, a, alive, *_, eff, _ = tk
            i = tk[7]
            if not alive or "ghost" in eff or "invis" in eff:
                continue
            last = self.tracks.get(i)
            if last and math.hypot(x - last[0], y - last[1]) < 5:
                continue
            self.tracks[i] = (x, y, a)
            if last is None or math.hypot(x - last[0], y - last[1]) > 40:  # Teleport
                continue
            k = MINI_SCALE if "mini" in eff else 1
            lx, ly, la = last
            for side in (-1, 1):
                p0 = (
                    lx - math.sin(la) * 12 * k * side,
                    ly + math.cos(la) * 12 * k * side,
                )
                p1 = (x - math.sin(a) * 12 * k * side, y + math.cos(a) * 12 * k * side)
                pygame.draw.line(surf, col, p0, p1, max(2, round(6 * k)))

    @staticmethod
    def _leader(snap):
        scores = [snap["sc"][t[7]] for t in snap["tk"]]
        best = max(scores, default=0)
        if best > 0 and scores.count(best) == 1:
            return snap["tk"][scores.index(best)][7]
        return None

    def _draw_fog(self, arena, snap, me):
        if me is None:  # lokal: um alle Panzer herum sichtbar
            centers = [(t[0], t[1]) for t in snap["tk"] if t[3]]
        else:
            mine = next((t for t in snap["tk"] if t[7] == me), None)
            if not mine or not mine[3]:
                return  # tot -> zuschauen ohne Nebel
            centers = [(mine[0], mine[1])]
        if self.fog is None or self.fog.get_size() != (arena.ww, arena.wh):
            self.fog = pygame.Surface((arena.ww, arena.wh), pygame.SRCALPHA)
        c = (9, 10, 15)
        self.fog.fill((*c, 255))
        wall = shade(arena.theme["wall"], 0.45)
        for w in arena.walls:  # Wände schwach sichtbar, Panzer und Kugeln nicht
            pygame.draw.rect(self.fog, (*wall, 255), w)
        # erst alle äußeren Ringe, dann die inneren, sonst überdeckt ein Panzer das Loch des anderen
        for extra, alpha in ((40, 215), (24, 150), (10, 70), (0, 0)):
            for x, y in centers:
                pygame.draw.circle(self.fog, (*c, alpha), (x, y), FOG_R + extra)
        self.screen.blit(self.fog, (0, 0))

    # --- Zeichnen: Bildschirm (HUD, Banner, Übergänge) ---

    def _feed_surf(self, txt, slot, victim, how):
        key = (txt, slot, victim, how)
        surf = self.feed_cache.get(key)
        if surf is not None:
            return surf
        if len(self.feed_cache) > 64:
            self.feed_cache.clear()
        names = [(NAMES[slot], mix(COLORS[slot], (255, 255, 255), 0.3))]
        if victim != slot:
            names.append((NAMES[victim], mix(COLORS[victim], (255, 255, 255), 0.3)))
        parts = [
            self.small.render(piece, True, c or (215, 220, 230))
            for piece, c in rich_parts(txt, names)
        ]
        tw = sum(p.get_width() for p in parts)
        h = 24
        surf = pygame.Surface((tw + 46, h), pygame.SRCALPHA)
        pygame.draw.rect(surf, (12, 13, 18, 205), surf.get_rect(), border_radius=6)
        pygame.draw.rect(
            surf,
            (*COLORS[slot], 255),
            (0, 0, 4, h),
            border_top_left_radius=6,
            border_bottom_left_radius=6,
        )
        icon = {
            "b": None,
            "frg": "frag",
            "sg": "shotgun",
            "bnc": "bouncy",
            "fl": "flame",
        }.get(how, how)
        if icon in self.boxes:
            pygame.draw.circle(surf, (226, 231, 240), (20, h // 2), 9)
            draw_icon(surf, icon, 20, h // 2, 0.6)
        else:
            pygame.draw.circle(surf, (226, 231, 240), (20, h // 2), 3)
        x = 36
        for p in parts:
            surf.blit(p, (x, (h - p.get_height()) // 2 + 1))
            x += p.get_width()
        self.feed_cache[key] = surf
        return surf

    def _draw_feed(self, snap):
        y = 26
        for txt, slot, age, victim, how in snap.get("kf", []):
            surf = self._feed_surf(txt, slot, victim, how)
            slide = 1 - ease_out(age / 0.25)
            surf.set_alpha(int(255 * max(0.0, min(1.0, (FEED_TIME - age) / 0.8))))
            self.screen.blit(surf, (W - 10 - surf.get_width() + 90 * slide, y))
            y += 28

    def _banner(self, text, size, color, center, scale=1.0, alpha=255, glow_on=False):
        surf = fancy(text, size, color)
        level = round(8 * alpha / 255)
        if glow_on and level:
            g = fancy_glow(text, size, color, 3, 10, level)
            if scale != 1.0:
                g = pygame.transform.smoothscale_by(g, max(0.05, scale))
            self.screen.blit(
                g, g.get_rect(center=center), special_flags=pygame.BLEND_ADD
            )
        if scale != 1.0:
            surf = pygame.transform.smoothscale_by(surf, max(0.05, scale))
        surf.set_alpha(alpha)
        self.screen.blit(surf, surf.get_rect(center=center))

    def _draw_banners(self, snap, arena, now):
        cx, cy = W // 2, ARENA_TOP + ARENA_H // 2
        t = now - self.round_start
        th = arena.theme
        if arena.mut:
            name, desc = MUTATORS[arena.mut]
            lbl = fancy(
                f"Sonderregel: {name}",
                18,
                mix(th["accent"], (255, 255, 255), 0.3),
                2,
                False,
            )
            self.screen.blit(lbl, lbl.get_rect(center=(W // 2, 9)))
        if t < 2.0 and not snap.get("msg"):  # Rundenstart
            k_in, fade = ease_back(t / 0.35), max(0.0, min(1.0, (2.0 - t) / 0.5))
            a = int(255 * fade)
            self._banner(
                f"RUNDE {snap['m']}",
                64,
                (255, 255, 255),
                (cx, cy - 40),
                0.6 + 0.4 * k_in,
                a,
                True,
            )
            arena_lbl = fancy(th["name"].upper(), 22, th["accent"], 2)
            arena_lbl.set_alpha(a)
            self.screen.blit(arena_lbl, arena_lbl.get_rect(center=(cx, cy + 2)))
            if arena.mut:
                name, desc = MUTATORS[arena.mut]
                self._banner(
                    f"{name.upper()}!",
                    44,
                    (200, 120, 255),
                    (cx, cy + 48),
                    0.6 + 0.4 * ease_back((t - 0.25) / 0.35),
                    a,
                    True,
                )
                d = fancy(desc, 20, (235, 235, 245), 2, False)
                d.set_alpha(a)
                self.screen.blit(d, d.get_rect(center=(cx, cy + 84)))
        ann = [f for f in snap["fx"] if f[1] == "ann"]
        if ann and not snap.get("msg"):
            fid, _, age, life, (text, slot, sub) = ann[-1]
            z = ease_back(age / 0.28)
            fade = max(0.0, min(1.0, (life - age) / 0.4))
            col = mix(COLORS[slot], (255, 220, 120), 0.35)
            wob = math.sin(age * 30) * 3 * max(0.0, 1 - age / 0.4)
            self._banner(text, 58, col, (cx + wob, 130), z, int(255 * fade), True)
            if sub:
                s = fancy(sub, 22, (240, 240, 248), 2, False)
                s.set_alpha(int(255 * fade))
                self.screen.blit(s, s.get_rect(center=(cx, 174)))
        if snap.get("msg"):
            win = snap.get("win") or [None, ""]
            et = snap.get("et") or 0
            k = ease_out((ROUND_END_DELAY - 1 - et) / 0.3)
            band = pygame.Rect(0, 0, W, 120)
            band.center = (cx, cy)
            veil = pygame.Surface(band.size, pygame.SRCALPHA)
            veil.fill((6, 7, 11, int(185 * k)))
            self.screen.blit(veil, band)
            col = COLORS[win[0]] if win[0] is not None else (200, 205, 215)
            pygame.draw.line(
                self.screen,
                col,
                (cx - W * k / 2, band.top),
                (cx + W * k / 2, band.top),
                2,
            )
            pygame.draw.line(
                self.screen,
                col,
                (cx - W * k / 2, band.bottom),
                (cx + W * k / 2, band.bottom),
                2,
            )
            text = snap["msg"].upper()
            self._banner(
                text,
                56,
                mix(col, (255, 255, 255), 0.25),
                (cx, cy - (12 if win[1] else 0)),
                0.5 + 0.5 * ease_back((ROUND_END_DELAY - 1 - et) / 0.35),
                255,
                True,
            )
            if win[1]:
                s = fancy(win[1], 26, ACCENT_C, 2)
                self.screen.blit(s, s.get_rect(center=(cx, cy + 34)))

    def _draw_transition(self, snap, now):
        """Jalousie: schließt sich kurz vor der neuen Runde und öffnet sich danach."""
        cover = max(0.0, 1 - (now - self.round_start) / 0.5)
        et = snap.get("et") if snap else None
        if et is not None and et < 0.3:
            cover = max(cover, 1 - et / 0.3)
        if cover <= 0:
            return
        n = 12
        sw = W / n
        accent = self.round_arena.theme["accent"] if self.round_arena else ACCENT_C
        for j in range(n):
            c = max(0.0, min(1.0, cover * 1.6 - j * 0.05))
            if c <= 0:
                continue
            w = sw * c
            x = j * sw + (sw - w) / 2
            pygame.draw.rect(self.screen, (9, 10, 14), (x, 0, w + 1, VIEW_H))
            if c < 1:
                pygame.draw.line(self.screen, accent, (x, 0), (x, VIEW_H), 1)
                pygame.draw.line(self.screen, accent, (x + w, 0), (x + w, VIEW_H), 1)

    def _make_hud_bg(self):
        s = pygame.Surface((W, H - VIEW_H))
        for y in range(s.get_height()):
            k = y / s.get_height()
            pygame.draw.line(s, mix((22, 24, 32), (10, 11, 15), k), (0, y), (W, y))
        pygame.draw.line(s, (56, 62, 80), (0, 0), (W, 0))
        return s.convert()

    def _card_bg(self, size, slot, mine):
        key = (size, slot, mine)
        bg = self.card_cache.get(key)
        if bg is None:
            w, h = size
            col = COLORS[slot]
            bg = pygame.Surface(size, pygame.SRCALPHA)
            pygame.draw.rect(bg, (26, 29, 38), (0, 0, w, h), border_radius=8)
            for y in range(h // 2):  # Glanz oben
                pygame.draw.line(
                    bg,
                    (255, 255, 255, int(10 * (1 - y / (h / 2)))),
                    (6, y + 2),
                    (w - 6, y + 2),
                )
            pygame.draw.rect(
                bg,
                shade(col, 0.9) if mine else (50, 55, 70),
                (0, 0, w, h),
                2 if mine else 1,
                border_radius=8,
            )
            pygame.draw.rect(bg, col, (8, 0, w - 16, 3), border_radius=2)
            self.card_cache[key] = bg = bg.convert_alpha()
        return bg

    def _hud(self, snap, me, arena, now):
        scr = self.screen
        scr.blit(self.hud_bg, (0, VIEW_H))
        tanks = snap["tk"]
        n = len(tanks)
        mut = arena.mut if arena else None
        most, life = bullet_params(mut)[3], bullet_params(mut)[2]
        leader = self._leader(snap)
        bots = set(snap.get("bots", ()))
        gap = 8 if n <= 4 else 5
        cw = (W - 20 - gap * (n - 1)) / n
        top, ch = VIEW_H + 7, H - VIEW_H - 28
        for k, tank in enumerate(tanks):
            rect = pygame.Rect(round(10 + k * (cw + gap)), top, round(cw), ch)
            self._card(
                rect, tank, snap["sc"][tank[7]], me, leader, bots, most, life, now
            )

    def _card(self, rect, tank, score, me, leader, bots, most, life, now):
        scr = self.screen
        _, _, _, alive, _, weapon, shield, i, ammo, reload, eff, _ = tank
        col = COLORS[i]
        compact = rect.w < 200
        scr.blit(self._card_bg(rect.size, i, i == me), rect)
        icon = self.hud_icons[i if alive else i + DEAD_SPRITE]
        ix = rect.x + (25 if compact else 31)
        scr.blit(icon, icon.get_rect(center=(ix, rect.centery + 1)))
        if i == leader:
            draw_crown(scr, ix, rect.y + 9, 0.85)
        pop = self.score_pop.get(i)
        num = fancy(
            str(score), 34 if compact else 42, (255, 255, 255) if alive else DIM_C, 3
        )
        if pop is not None and now - pop < 0.7:
            z = 1 + 0.8 * (1 - ease_out((now - pop) / 0.7))
            num = pygame.transform.smoothscale_by(num, z)
            self._light_screen(
                ACCENT_C, rect.right - 30, rect.centery, 40, 5 * (1 - (now - pop) / 0.7)
            )
        scr.blit(num, num.get_rect(midright=(rect.right - 10, rect.centery + 2)))
        tags = (["DU"] if i == me else []) + (["BOT"] if i in bots else [])
        if compact:
            row_x, row_y = rect.x + 52, rect.y + 44
            for j, tag in enumerate(tags):
                self._tag(tag, (rect.x + 8 + 30 * j, rect.y + 6), col)
        else:
            name = self.small.render(NAMES[i], True, mix(col, (255, 255, 255), 0.35))
            scr.blit(name, (rect.x + 60, rect.y + 7))
            tx = rect.x + 66 + name.get_width()
            for tag in tags:
                tx += self._tag(tag, (tx, rect.y + 7), col) + 4
            row_x, row_y = rect.x + 60, rect.y + 31
        if not alive:
            if not compact:  # schmale Karten: zu wenig Platz neben der Punktzahl
                lbl = self.tiny.render("zerstört", True, DIM_C)
                scr.blit(lbl, (row_x, row_y - 5))
        elif weapon:  # Waffe mit Restmunition
            pygame.draw.circle(scr, (228, 232, 240), (row_x + 9, row_y + 3), 10)
            pygame.draw.circle(scr, box_color(weapon), (row_x + 9, row_y + 3), 10, 2)
            draw_icon(scr, weapon, row_x + 9, row_y + 3, 0.62)
            if weapon == "flame":
                bw = 40 if compact else 70
                pygame.draw.rect(
                    scr, (46, 50, 62), (row_x + 24, row_y, bw, 6), border_radius=3
                )
                pygame.draw.rect(
                    scr,
                    (255, 140, 40),
                    (row_x + 24, row_y, bw * ammo / AMMO["flame"], 6),
                    border_radius=3,
                )
            else:
                label = POWERUP_NAMES[weapon] if not compact else ""
                if weapon in AMMO:
                    label += f" {ammo}×"
                if label.strip():
                    t = self.tiny.render(label.strip(), True, (200, 205, 215))
                    scr.blit(t, (row_x + 23, row_y - 3))
        else:  # freie Kugeln als leuchtende Punkte
            for b in range(most):
                c = (row_x + 4 + b * 11, row_y + 3)
                if b < ammo:
                    pygame.draw.circle(scr, mix(col, (255, 255, 255), 0.5), c, 4)
                    pygame.draw.circle(scr, (255, 255, 255), c, 2)
                else:
                    pygame.draw.circle(scr, (70, 75, 90), c, 4, 1)
        if alive and (eff or shield > 0):  # Effekte mit Restzeit
            ex = row_x + (0 if compact else 0)
            ey = row_y + 21
            items = ([("shield", shield, SHIELD_TIME)] if shield > 0 else []) + [
                (
                    e,
                    v,
                    EFFECT_TIME.get(e, POTATO_TIME if e == "potato" else FREEZE_TIME),
                )
                for e, v in eff.items()
            ]
            for e, v, full in items[: 2 if compact else 4]:
                pygame.draw.circle(scr, (228, 232, 240), (ex + 8, ey), 8)
                draw_icon(scr, e if e != "frozen" else "ice", ex + 8, ey, 0.5)
                rr = pygame.Rect(0, 0, 20, 20)
                rr.center = (ex + 8, ey)
                pygame.draw.arc(
                    scr,
                    ACCENT_C,
                    rr,
                    math.pi / 2,
                    math.pi / 2 + 2 * math.pi * min(1, v / full),
                    2,
                )
                ex += 24
        if reload > 0 and alive and not weapon:
            frac = 1 - min(reload / life, 1)
            bar = pygame.Rect(rect.x + 8, rect.bottom - 6, rect.w - 16, 3)
            pygame.draw.rect(scr, (40, 44, 56), bar)
            pygame.draw.rect(scr, col, (bar.x, bar.y, bar.w * frac, 3))
        if not alive:
            dim = pygame.Surface(rect.size, pygame.SRCALPHA)
            dim.fill((8, 9, 12, 110))
            scr.blit(dim, rect)

    def _tag(self, text, pos, color):
        t = self.tiny.render(text, True, (14, 15, 19))
        box = pygame.Rect(pos[0], pos[1], t.get_width() + 8, 15)
        pygame.draw.rect(
            self.screen, mix(color, (255, 255, 255), 0.35), box, border_radius=4
        )
        self.screen.blit(t, (box.x + 4, box.y + 2))
        return box.w

    def _light_screen(self, color, x, y, r, level):
        r = light_r(r)
        if level >= 1:
            self.screen.blit(
                glow(color, r, min(8, int(level))),
                (x - r, y - r),
                special_flags=pygame.BLEND_ADD,
            )

    def _overlays(self, snap, me, now, dt):
        """Bildschirm-Effekte: Blitz, Zeitlupen-Balken, Gefahr am Rand."""
        scr = self.screen
        view = pygame.Rect(0, 0, W, VIEW_H)
        k = self.cine
        if k > 0.005:  # Zeitlupe: kühler Farbton und Kinobalken
            scr.fill(
                mix((255, 255, 255), (205, 220, 255), k),
                view,
                special_flags=pygame.BLEND_RGB_MULT,
            )
            bar = round(36 * k)
            scr.fill((0, 0, 0), (0, 0, W, bar))
            scr.fill((0, 0, 0), (0, VIEW_H - bar, W, bar))
        if self.flash > 0.01:
            v = int(255 * min(1.0, self.flash))
            scr.fill((v, v, v), view, special_flags=pygame.BLEND_RGB_ADD)
        self.flash *= math.exp(-7 * dt)
        mine = (
            next((t for t in snap["tk"] if t[7] == me), None)
            if snap and me is not None
            else None
        )
        if mine and mine[3]:
            eff = mine[10]
            danger = None
            if "potato" in eff and eff["potato"] < 3:
                danger = ((255, 40, 20), 0.5 + 0.5 * math.sin(now * 14))
            elif "frozen" in eff:
                danger = ((120, 200, 255), 0.6)
            if danger:
                tint = edge_light().copy()
                tint.fill(shade(*danger), special_flags=pygame.BLEND_RGB_MULT)
                scr.blit(tint, (0, 0), special_flags=pygame.BLEND_RGB_ADD)

    def _darkness(self, snap, me, now, arena):
        """Stromausfall: alles dunkel bis auf Scheinwerfer und alles, was glüht. Die Lichtkarte
        entsteht in Viertel-Auflösung, wird weich hochskaliert und mit dem Bild multipliziert.
        """
        q = LIGHT_SCALE
        size = (arena.ww // q + 1, arena.wh // q + 1)
        if self.lightmap is None or self.lightmap.get_size() != size:
            self.lightmap = pygame.Surface(size).convert()
        lm = self.lightmap
        lm.fill((13, 15, 22))
        blits = []
        for tk in snap["tk"]:
            if tk[3] and self._tank_alpha(tk, me, now) >= 100:
                cone = cone_sprite(round(math.degrees(tk[2]) / 6) % 60)
                blits.append((cone, cone.get_rect(center=(tk[0] / q, tk[1] / q))))
        for color, x, y, r, level in self.light_log:
            rr = light_r(r * 2 / q)
            blits.append(
                (glow(color, rr, min(8, int(level) + 2)), (x / q - rr, y / q - rr))
            )
        lm.fblits(blits, pygame.BLEND_ADD)
        big = pygame.transform.smoothscale(lm, (size[0] * q, size[1] * q))
        self.screen.blit(big, (0, 0), special_flags=pygame.BLEND_RGB_MULT)

    def _walls_changed(self, old, new):
        """Mitten in der Runde sind Wände weggesprengt worden: nur die betroffenen Stellen neu
        zeichnen (Spuren und Brandflecken bleiben) und Trümmer fliegen lassen."""
        fresh = build_maze_surface(new)
        before, after = set(old.walls), set(new.walls)
        for x, y, w, h in before ^ after:
            r = pygame.Rect(x - 6, y - 6, w + 16, h + 18)
            self.maze_surf.blit(fresh, r, area=r)
        th = new.theme
        for x, y, w, h in before - after:
            for _ in range(max(2, (w + h) // 25)):
                a = random.uniform(0, 2 * math.pi)
                v = random.uniform(60, 240)
                self.parts.add(
                    DEBRIS,
                    x + random.uniform(0, w),
                    y + random.uniform(0, h),
                    math.cos(a) * v,
                    math.sin(a) * v,
                    random.uniform(0.8, 1.6),
                    random.uniform(3, 6),
                    mix(th["wall"], th["hi"], random.random() * 0.5),
                    drag=3,
                    spin=random.uniform(-10, 10),
                )
            self.parts.burst(
                DUST, x + w / 2, y + h / 2, 5, 80, 1.0, 9, th["dust"], drag=2
            )
        self.maze_arena = self.round_arena = new

    # --- Hauptfunktion ---

    def backdrop(self):
        """Hintergrund ohne eigenes Spiel: Bots kämpfen, darüber ein dunkler Schleier."""
        if self.demo is None:
            self.demo = Demo()
        self.demo.step()
        g = self.demo.game
        self.draw(g.arena, g.snapshot(), quiet=True, hud=False)
        self.veil.fill((7, 8, 12, 150))
        self.screen.blit(self.veil, (0, 0))

    def draw(self, arena, snap, me=None, hint="", info="", quiet=False, hud=True):
        real = self.screen
        if arena is None:
            real.fill(BG)
            self.backdrop()
            if hint:
                self.text(hint, (W // 2, H - 12), self.small, DIM_C)
            return
        now = pygame.time.get_ticks() / 1000
        frame = time.perf_counter()
        dt = min(0.05, max(0.0, frame - self.last_frame))
        self.last_frame = frame
        self.quiet, self.me = quiet, me
        live = snap is not None and snap["m"] == arena.id
        ts = snap.get("ts", 1.0) if live else 1.0
        vdt = dt * ts
        if arena is not self.round_arena:
            old = self.round_arena
            if old is not None and old.id == arena.id and self.maze_arena is old:
                self._walls_changed(old, arena)  # Atombombe hat Wände weggesprengt
            else:
                self._new_round(arena, now)
        self.dark = live and arena.mut == "dunkel"
        self.light_log = []
        if self.world is None or self.world.get_size() != (arena.ww, arena.wh):
            self.world = pygame.Surface((arena.ww, arena.wh)).convert()
            self.light = pygame.Surface((arena.ww, arena.wh)).convert()
        self.world_w = arena.ww
        scr = self.screen = self.world
        maze = self._maze_surface(arena)
        mine = None
        if live:
            mine = next((t for t in snap["tk"] if t[7] == me), None)
            self.me_pos = (mine[0], mine[1]) if mine else None
            self._stamp_tracks(maze, snap["tk"], arena)
            self._events(snap, me, arena, now)
            if quiet:  # Hintergrund im Menü: kein Blitzen und Wackeln
                self.flash = self.trauma = 0.0
            self._track_shots(snap, now)
            self._emit(snap, arena, vdt, now)
        for p in self.parts.update(vdt, arena):  # liegen gebliebene Trümmer
            pygame.draw.polygon(
                maze, shade(p[8], 0.55), rot_rect(p[0], p[1], p[7], p[7] * 0.65, p[11])
            )
        self._update_flying(vdt, maze, arena)
        scr.blit(maze, (0, 0))
        low, polys_low, high, polys_high = self._collect()
        scr.fblits(low)
        for c, pts in polys_low:
            pygame.draw.polygon(scr, c, pts)
        self._draw_ambient(True)
        leader = None
        if live:
            leader = self._leader(snap)
            self._draw_boxes(snap, now)
            self._draw_shots(snap, me, now, arena)
            for tk in sorted(snap["tk"], key=lambda t: t[3]):  # Wracks zuerst
                self._draw_tank(tk, me, now)
            self._draw_fx(snap, now)
        self._draw_flying()
        for jx, jy, ang in self.jets:
            deg = -math.degrees(ang)
            sh = pygame.transform.rotate(self.jet_shadow, deg)
            scr.blit(sh, sh.get_rect(center=(jx + 34, jy + 52)))
            spr = pygame.transform.rotate(self.jet, deg)
            scr.blit(spr, spr.get_rect(center=(jx, jy)))
            tx, ty = jx - math.cos(ang) * 33, jy - math.sin(ang) * 33
            self._light((255, 150, 60), tx, ty, 18, 7)
        self.jets = []
        scr.fblits(high)
        for c, pts in polys_high:
            pygame.draw.polygon(scr, c, pts)
        self._draw_ambient(False)
        self._flush_light()
        if live:
            bots = set(snap.get("bots", ()))
            for tk in snap["tk"]:
                self._draw_tank_ui(tk, arena, me, leader, now, bots)
            self._draw_fx_ui(snap, now)
            self._flush_light()  # Lichter der Anzeigen (Krone, Kartoffel)
            if arena.mut == "nebel":
                self._draw_fog(arena, snap, me)
            elif self.dark:
                self._darkness(snap, me, now, arena)
        # Zeitlupe: Kinobalken und Zoom auf den letzten Abschuss, weich nachgeführt
        target = min(1.0, (1 - ts) / (1 - SLOWMO_SCALE)) if live and not quiet else 0.0
        self.cine += (target - self.cine) * min(1.0, dt * 7)
        self.music.update(self.sfx.on, 1.0 if quiet else ts)
        # aufs Fenster: große Welten verkleinert, mit Wackeln
        self.screen = real
        self.trauma = max(0.0, self.trauma - dt * 1.5)
        s = self.trauma**2 * 15
        ox = s * (math.sin(now * 53.1) * 0.6 + math.sin(now * 91.7 + 1.3) * 0.4)
        oy = s * (math.sin(now * 61.3 + 2.1) * 0.6 + math.sin(now * 83.9 + 0.7) * 0.4)
        real.fill(BG)
        zoom = 1 + 0.16 * self.cine if self.focus and not quiet else 1.0
        if zoom > 1.002:
            src = pygame.Rect(0, 0, arena.ww / zoom, arena.wh / zoom)
            src.center = self.focus
            src.clamp_ip(self.world.get_rect())
            view = self.world.subsurface(src)
            real.blit(pygame.transform.smoothscale(view, (W, VIEW_H)), (ox, oy))
        elif (arena.ww, arena.wh) != (W, VIEW_H):
            real.blit(pygame.transform.smoothscale(self.world, (W, VIEW_H)), (ox, oy))
        else:
            real.blit(self.world, (ox, oy))
        self._overlays(snap if live else None, me, now, dt)
        if live and hud:
            self._draw_banners(snap, arena, now)
            self._draw_feed(snap)
            self._hud(snap, me, arena, now)
        self._draw_transition(snap if live else None, now)
        if hud:
            rd = fancy(f"Runde {arena.id}", 16, DIM_C, 2, False)
            real.blit(rd, (8, 2))
            if hint:
                self.text(hint, (W // 2, H - 11), self.tiny, (110, 116, 132))
        if self.debug and info:
            self.text(info, (8, 20), self.small, (255, 90, 180), center=False)


# --- Eingabe -----------------------------------------------------------------


class KeyInput:
    """Tastenbelegung -> (bits, schuss_zähler)."""

    def __init__(self, up, down, left, right, fire, taunts=()):
        self.taunts = taunts  # Tasten für die Sprüche 1..n
        self.map = (
            [(k, UP) for k in up]
            + [(k, DOWN) for k in down]
            + [(k, LEFT) for k in left]
            + [(k, RIGHT) for k in right]
            + [(k, FIRE) for k in fire]
        )
        self.fire_keys = fire
        self.fires = 0

    def handle(self, event):
        if event.type == pygame.KEYDOWN and event.key in self.fire_keys:
            self.fires += 1

    def state(self, pressed):
        bits = 0
        for k, bit in self.map:
            if pressed[k]:
                bits |= bit
        for n, k in enumerate(self.taunts, 1):
            if pressed[k]:
                bits |= n << TAUNT_SHIFT
                break
        return bits, self.fires


def net_input():
    return KeyInput(
        (pygame.K_UP, pygame.K_w),
        (pygame.K_DOWN, pygame.K_s),
        (pygame.K_LEFT, pygame.K_a),
        (pygame.K_RIGHT, pygame.K_d),
        (pygame.K_SPACE, pygame.K_m, pygame.K_q),
        (pygame.K_1, pygame.K_2, pygame.K_3, pygame.K_4, pygame.K_5),
    )


NET_HINT = "Pfeile/WASD fahren  ·  Leertaste schießen  ·  1–5 Sprüche  ·  F2 Ton  ·  F4 Musik  ·  F3 Ping  ·  F11 Vollbild  ·  Esc Menü"


def common_events(event, rend):
    """True, wenn zurück ins Menü."""
    if event.type == pygame.QUIT:
        pygame.quit()
        sys.exit()
    if event.type == pygame.KEYDOWN:
        if event.key == pygame.K_F11:
            pygame.display.toggle_fullscreen()
        elif event.key == pygame.K_F3:
            rend.debug = not rend.debug
        elif event.key == pygame.K_F2:
            rend.sfx.on = not rend.sfx.on
        elif event.key == pygame.K_F4:
            rend.music.on = not rend.music.on
        elif event.key == pygame.K_ESCAPE:
            return True
    return False


# --- Modi --------------------------------------------------------------------


def run_local(screen, clock, rend, players=2, bots=0):
    """players an einer Tastatur (1 = allein gegen Bots), dazu bots Computergegner."""
    n = min(players + bots, MAX_PLAYERS)
    game = Game(range(n))
    robots = [Bot(slot) for slot in range(players, n)]
    game.bots = [b.slot for b in robots]
    if players == 1:  # allein: Steuerung wie im Netzwerkspiel
        inputs = [net_input()]
        hint = NET_HINT.replace("  ·  F3 Ping", "")
    else:
        inputs = [
            KeyInput(
                (pygame.K_e,),
                (pygame.K_d,),
                (pygame.K_s,),
                (pygame.K_f,),
                (pygame.K_q,),
            ),
            KeyInput(
                (pygame.K_UP,),
                (pygame.K_DOWN,),
                (pygame.K_LEFT,),
                (pygame.K_RIGHT,),
                (pygame.K_m,),
            ),
            KeyInput(
                (pygame.K_i,),
                (pygame.K_k,),
                (pygame.K_j,),
                (pygame.K_l,),
                (pygame.K_u,),
            ),
            KeyInput(
                (pygame.K_KP8,),
                (pygame.K_KP5,),
                (pygame.K_KP4,),
                (pygame.K_KP6,),
                (pygame.K_KP0, pygame.K_KP_ENTER),
            ),
        ][:players]
        hint = "  ·  ".join(
            [
                "Rot: ESDF + Q",
                "Grün: Pfeile + M",
                "Blau: IJKL + U",
                "Orange: Numpad 8456 + 0",
            ][:players]
            + ["F2 Ton", "F4 Musik", "Esc Menü"]
        )
    me = 0 if players == 1 else None
    pygame.mouse.set_visible(False)
    acc = 0.0
    while True:
        acc = min(acc + clock.tick(FPS) / 1000, 0.25)
        for e in pygame.event.get():
            if common_events(e, rend):
                return
            for inp in inputs:
                inp.handle(e)
        pressed = pygame.key.get_pressed()
        while acc >= TICK:
            acc -= TICK
            for slot, inp in enumerate(inputs):
                game.apply_input(slot, *inp.state(pressed), TICK)
            for bot in robots:
                game.apply_input(bot.slot, *bot.think(game), TICK)
            game.step(TICK)
        rend.draw(
            game.arena,
            game.snapshot(),
            me=me,
            hint=hint,
            info=f"{clock.get_fps():.0f} FPS",
        )
        pygame.display.flip()


def run_host(screen, clock, rend, port=PORT, bots=0):
    try:
        net = HostNet(port)
    except OSError as e:
        return message(screen, clock, rend, f"Port {port} geht nicht: {e}")
    ip = local_ip()
    inp = net_input()
    game, gen, sent_maze, peers, robots = None, -1, None, {}, {}
    acc, ticks = 0.0, 0
    pygame.mouse.set_visible(False)
    try:
        while True:
            acc = min(acc + clock.tick(FPS) / 1000, 0.25)
            for e in pygame.event.get():
                if common_events(e, rend):
                    return
                inp.handle(e)

            if gen != net.gen:  # jemand ist beigetreten oder gegangen
                gen, old, old_bots = net.gen, peers, robots
                peers = net.clients()
                # Bots nehmen die hohen freien Plätze, Clients bekommen die niedrigen
                free = [s for s in range(MAX_PLAYERS - 1, 0, -1) if s not in peers]
                robots = {s: old_bots.get(s) or Bot(s) for s in free[:bots]}
                players = sorted([0, *peers, *robots])
                fresh = [s for s, p in peers.items() if old.get(s) is not p]
                fresh += [s for s in robots if s not in old_bots]
                if len(players) < 2:
                    game = None
                elif game is None:
                    game, acc = Game(players), 0.0
                else:
                    game.set_players(players, fresh)
                if game:
                    game.bots = sorted(robots)
                sent_maze = None

            if game is None:
                rend.draw(None, None, hint="Esc Menü")
                title = fancy("Warte auf Mitspieler …", 44, (255, 255, 255))
                screen.blit(title, title.get_rect(center=(W // 2, 250)))
                addr = fancy(f"Deine IP:  {ip}   Port: {port}", 28, ACCENT_C, 2, False)
                screen.blit(addr, addr.get_rect(center=(W // 2, 320)))
                rend.text(
                    f'Bis zu {MAX_PLAYERS - 1} Freunde wählen "Beitreten" und geben diese IP ein.',
                    (W // 2, 362),
                    rend.small,
                    DIM_C,
                )
                rend.text(
                    "Nicht im selben Netz? Tailscale/ZeroTier-IP oder Port-Forwarding (TCP).",
                    (W // 2, 387),
                    rend.small,
                    DIM_C,
                )
                pygame.display.flip()
                continue

            local = inp.state(pygame.key.get_pressed())
            while acc >= TICK:
                acc -= TICK
                ticks += 1
                game.apply_input(0, *local, TICK)
                # Client-Inputs im Takt abarbeiten (kleiner Jitter-Puffer), bei Rückstand aufholen
                for slot, peer in peers.items():
                    cmds = peer.cmds
                    peer.budget = max(-0.1, min(peer.budget + TICK, 0.1))
                    while cmds and (cmds[0][3] <= peer.budget + 1e-6 or len(cmds) > 6):
                        seq, keys, fires, dt = cmds.popleft()
                        dt = max(0.0, min(float(dt), 0.05))
                        game.apply_input(slot, int(keys), int(fires), dt)
                        peer.budget -= dt
                        peer.ack = seq
                for slot, bot in robots.items():
                    game.apply_input(slot, *bot.think(game), TICK)
                game.step(TICK)
                # neues Labyrinth oder (nach einer Atombombe) neue Wände
                maze_key = (game.maze_id, game.maze.get("rev", 0))
                if sent_maze != maze_key:
                    net.broadcast({"t": "m", "maze": game.maze})
                    sent_maze = maze_key
                if ticks % SEND_EVERY == 0:
                    snap = game.snapshot()
                    for peer in peers.values():
                        net.send(peer, dict(snap, ack=peer.ack))
            free = MAX_PLAYERS - 1 - len(peers)
            rend.draw(
                game.arena,
                game.snapshot(),
                me=0,
                hint=NET_HINT,
                info=f"{clock.get_fps():.0f} FPS  ·  Host",
            )
            if free:
                rend.text(
                    f"{free} Platz frei  ·  {ip}:{port}",
                    (W - 8, 3),
                    rend.tiny,
                    DIM_C,
                    center=False,
                    right=True,
                )
            pygame.display.flip()
    finally:
        net.close()


class Predicted:
    def __init__(self, x, y, a, eff):
        self.x, self.y, self.a, self.eff = x, y, a, eff


def run_client(screen, clock, rend, addr):
    host, _, port = addr.strip().partition(":")
    port = int(port) if port.isdigit() else PORT
    rend.draw(None, None)
    title = fancy(f"Verbinde mit {host}:{port} …", 40, (255, 255, 255))
    screen.blit(title, title.get_rect(center=(W // 2, 300)))
    pygame.display.flip()
    try:
        net = ClientNet(host, port)
    except (OSError, ValueError) as e:
        return message(screen, clock, rend, f"Verbindung fehlgeschlagen: {e}")
    inp = net_input()
    seq, frame = 0, 0
    pending = (
        deque()
    )  # gesendete, vom Host noch nicht bestätigte Inputs: (seq, keys, dt, t_send)
    outbox = []
    ping = None
    pygame.mouse.set_visible(False)
    try:
        while True:
            dt = round(min(clock.tick(FPS) / 1000, 0.05), 4)
            for e in pygame.event.get():
                if common_events(e, rend):
                    return
                inp.handle(e)
            if net.error:
                return message(screen, clock, rend, net.error)
            if not net.connected:
                return message(screen, clock, rend, "Verbindung zum Host getrennt.")

            keys, fires = inp.state(pygame.key.get_pressed())
            seq += 1
            frame += 1
            outbox.append([seq, keys, fires, dt])
            pending.append((seq, keys, dt, time.perf_counter()))
            if frame % SEND_EVERY == 0:
                net.send({"t": "i", "c": outbox})
                outbox = []

            snap, arena = (net.snaps[-1] if net.snaps else None), net.arena
            view = None
            if snap is not None and arena is not None:
                ack = snap["ack"] or 0
                while pending and pending[0][0] <= ack:
                    s = pending.popleft()
                    if s[0] == ack:
                        sample = (time.perf_counter() - s[3]) * 1000
                        ping = sample if ping is None else ping * 0.9 + sample * 0.1
                while len(pending) > 180:
                    pending.popleft()
                view = interpolate(
                    net.snaps, time.perf_counter() + net.offset - INTERP_DELAY
                )
                me = next((t for t in snap["tk"] if t[7] == net.slot), None)
                # Eigener Panzer: letzter Serverzustand + noch unbestätigte Inputs -> sofortige Reaktion
                if me and me[3] and snap["m"] == arena.id:
                    p = Predicted(me[0], me[1], me[2], me[10])
                    for _, k, d, _ in pending:
                        move_tank(p, k, d, arena)
                    me = [p.x, p.y, p.a] + me[3:]
                if me:
                    view = dict(
                        view, tk=[me if t[7] == net.slot else t for t in view["tk"]]
                    )
            info = f"{clock.get_fps():.0f} FPS  ·  Ping {ping:.0f} ms" if ping else ""
            if arena is None:
                rend.draw(None, None, hint=NET_HINT)
            else:
                rend.draw(arena, view, me=net.slot, hint=NET_HINT, info=info)
            if view is None:
                wait = fancy("Warte auf Spielstart …", 44, (255, 255, 255))
                screen.blit(wait, wait.get_rect(center=(W // 2, 300)))
            pygame.display.flip()
    finally:
        net.close()


def message(screen, clock, rend, txt):
    pygame.mouse.set_visible(True)
    while True:
        clock.tick(FPS)
        for e in pygame.event.get():
            common_events(e, rend)
            if e.type in (pygame.KEYDOWN, pygame.MOUSEBUTTONDOWN):
                return
        rend.draw(None, None)
        msg = fancy(txt, 30, (255, 255, 255), 3, False)
        screen.blit(msg, msg.get_rect(center=(W // 2, 300)))
        rend.text("Beliebige Taste …", (W // 2, 350), rend.small, DIM_C)
        pygame.display.flip()


MENU_BOTS = [3]  # zuletzt gewählte Bot-Anzahl (gilt bis zum Beenden)


def menu_button(rend, rect, label, hover, accent=ACCENT_C, size=24):
    """Knopf im Menü: dunkles Glas, beim Drüberfahren leuchtet der Rand."""
    scr = rend.screen
    base = mix((30, 34, 46), (255, 255, 255), 0.08 if hover else 0)
    pygame.draw.rect(scr, (5, 6, 9), rect.move(0, 4), border_radius=10)
    pygame.draw.rect(scr, base, rect, border_radius=10)
    edge = accent if hover else mix(accent, (34, 38, 50), 0.55)
    pygame.draw.rect(scr, edge, rect, 2, border_radius=10)
    if hover:
        rend._light_screen(accent, rect.centerx, rect.centery, rect.w * 0.42, 2)
    lbl = fancy(label, size, (255, 255, 255), 2, False)
    scr.blit(lbl, lbl.get_rect(center=rect.center))


def menu(screen, clock, rend):
    """Gibt ("host", port, bots) / ("join", addr) / ("local", spieler, bots) zurück."""
    try:
        ip_text = LAST_IP_FILE.read_text().strip()
    except OSError:
        ip_text = ""
    cx = W // 2
    panel = pygame.Rect(cx - 215, 194, 430, 400)
    host_b = pygame.Rect(cx - 190, 214, 380, 52)
    ip_box = pygame.Rect(cx - 190, 286, 380, 46)
    join_b = pygame.Rect(cx - 190, 340, 380, 52)
    local_b = {
        n: pygame.Rect(cx - 190 + (n - 1) * 97, 444, 89, 46) for n in range(1, 5)
    }
    minus_b = pygame.Rect(cx + 44, 512, 40, 34)
    plus_b = pygame.Rect(cx + 150, 512, 40, 34)
    labels = {1: "Solo", 2: "2 Spieler", 3: "3 Spieler", 4: "4 Spieler"}
    glass = pygame.Surface(panel.size, pygame.SRCALPHA)
    pygame.draw.rect(glass, (13, 15, 21, 215), glass.get_rect(), border_radius=16)
    pygame.draw.rect(glass, (70, 78, 100, 255), glass.get_rect(), 1, border_radius=16)
    pygame.key.start_text_input()
    pygame.mouse.set_visible(True)

    def join():
        if ip_text:
            try:
                LAST_IP_FILE.write_text(ip_text)
            except OSError:
                pass
            return ("join", ip_text)

    def local(n):
        bots = MENU_BOTS[0]
        if n == 1:
            return ("local", 1, max(1, min(bots, MAX_PLAYERS - 1)))
        return ("local", n, min(bots, MAX_PLAYERS - n))

    while True:
        clock.tick(FPS)
        now = pygame.time.get_ticks() / 1000
        mouse = pygame.mouse.get_pos()
        for e in pygame.event.get():
            if e.type == pygame.QUIT or (
                e.type == pygame.KEYDOWN and e.key == pygame.K_ESCAPE
            ):
                pygame.quit()
                sys.exit()
            if e.type == pygame.KEYDOWN:
                if e.key == pygame.K_F11:
                    pygame.display.toggle_fullscreen()
                elif e.key == pygame.K_F2:
                    rend.sfx.on = not rend.sfx.on
                elif e.key == pygame.K_F4:
                    rend.music.on = not rend.music.on
                elif e.key == pygame.K_BACKSPACE:
                    ip_text = ip_text[:-1]
                elif e.key in (pygame.K_RETURN, pygame.K_KP_ENTER) and join():
                    return join()
            elif e.type == pygame.TEXTINPUT:
                ip_text = (
                    ip_text
                    + "".join(ch for ch in e.text if ch.isalnum() or ch in ".:-")
                )[:60]
            elif e.type == pygame.MOUSEBUTTONDOWN and e.button == 1:
                if host_b.collidepoint(e.pos):
                    return ("host", PORT, min(MENU_BOTS[0], MAX_PLAYERS - 1))
                if join_b.collidepoint(e.pos) and join():
                    return join()
                for n, rect in local_b.items():
                    if rect.collidepoint(e.pos):
                        return local(n)
                for rect, d in ((minus_b, -1), (plus_b, 1)):
                    if rect.collidepoint(e.pos):
                        MENU_BOTS[0] = min(max(MENU_BOTS[0] + d, 0), MAX_PLAYERS - 1)
                        rend.sfx.play("blip")

        rend.draw(None, None)  # Bots kämpfen im Hintergrund
        pulse = 0.5 + 0.5 * math.sin(now * 2.2)
        title = "PANZER"
        g = fancy_glow(title, 120, (255, 150, 40), 4, 14, 5 + round(3 * pulse))
        screen.blit(g, g.get_rect(center=(cx, 96)), special_flags=pygame.BLEND_ADD)
        logo = fancy(title, 120, (255, 196, 60), 4)
        screen.blit(logo, logo.get_rect(center=(cx, 96)))
        sub = fancy(
            "Labyrinth-Panzerschlacht  ·  1–8 Spieler  ·  Bots  ·  LAN & Internet",
            20,
            (225, 230, 240),
            2,
            False,
        )
        screen.blit(sub, sub.get_rect(center=(cx, 166)))
        screen.blit(glass, panel)
        menu_button(rend, host_b, "Spiel hosten", host_b.collidepoint(mouse))
        focus = ip_box.collidepoint(mouse)
        pygame.draw.rect(screen, (8, 9, 13), ip_box, border_radius=8)
        pygame.draw.rect(
            screen, ACCENT_C if focus else (70, 78, 100), ip_box, 2, border_radius=8
        )
        if ip_text:
            caret = "|" if pygame.time.get_ticks() // 500 % 2 else ""
            rend.text(ip_text + caret, ip_box.center, color=TEXT_C)
        else:
            rend.text(
                "IP-Adresse des Hosts eingeben", ip_box.center, color=(110, 116, 132)
            )
        menu_button(rend, join_b, "Beitreten", join_b.collidepoint(mouse))
        lbl = fancy("LOKAL AN EINER TASTATUR", 16, DIM_C, 2, False)
        screen.blit(lbl, lbl.get_rect(center=(cx, 428)))
        for n, rect in local_b.items():
            accent = (110, 200, 255) if n == 1 else ACCENT_C
            menu_button(rend, rect, labels[n], rect.collidepoint(mouse), accent, 20)
        lbl = fancy("BOTS DAZU", 20, TEXT_C, 2, False)
        screen.blit(lbl, lbl.get_rect(midleft=(cx - 186, 529)))
        menu_button(rend, minus_b, "–", minus_b.collidepoint(mouse), size=26)
        menu_button(rend, plus_b, "+", plus_b.collidepoint(mouse), size=26)
        num = fancy(str(MENU_BOTS[0]), 34, ACCENT_C, 3)
        screen.blit(num, num.get_rect(center=((minus_b.right + plus_b.left) // 2, 529)))
        bots = MENU_BOTS[0]
        info = (
            f"Solo: du gegen {max(1, bots)} Bot{'s' if max(1, bots) > 1 else ''}  ·  "
            "beim Hosten und lokal füllen Bots freie Plätze"
        )
        rend.text(info, (cx, 568), rend.tiny, DIM_C)
        rend.text(
            "Esc beendet  ·  F11 Vollbild  ·  F2 Ton  ·  F4 Musik",
            (cx, H - 16),
            rend.small,
            (120, 126, 142),
        )
        pygame.display.flip()


def main():
    pygame.mixer.pre_init(22050, -16, 2, 512)
    pygame.init()
    pygame.display.set_caption("Panzer")
    screen = pygame.display.set_mode((W, H), pygame.SCALED | pygame.RESIZABLE)
    clock = pygame.time.Clock()
    rend = Renderer(screen)
    pygame.display.set_icon(rend.sprites[0])

    args = sys.argv[1:]

    def num(i, default):
        return int(args[i]) if args[i : i + 1] and args[i].isdigit() else default

    action = None
    if args[:1] == ["host"]:
        action = ("host", num(1, PORT), min(num(2, 0), MAX_PLAYERS - 1))
    elif args[:1] == ["join"] and len(args) > 1:
        action = ("join", args[1])
    elif args[:1] == ["local"]:
        n = min(max(num(1, 2), 2), MAX_LOCAL)
        action = ("local", n, min(num(2, 0), MAX_PLAYERS - n))
    elif args[:1] == ["solo"]:
        action = ("local", 1, min(max(num(1, 3), 1), MAX_PLAYERS - 1))

    while True:
        if action is None:
            action = menu(screen, clock, rend)
        pygame.key.stop_text_input()
        if action[0] == "host":
            run_host(screen, clock, rend, *action[1:])
        elif action[0] == "join":
            run_client(screen, clock, rend, action[1])
        else:
            run_local(screen, clock, rend, *action[1:])
        action = None


if __name__ == "__main__":
    main()
