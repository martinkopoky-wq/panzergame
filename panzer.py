#!/usr/bin/env python3
"""Panzer – AZ/Tank-Trouble-Klon mit Multiplayer über IP.

    python panzer.py                    Menü
    python panzer.py host [port]        Spiel hosten (Standard-Port 5555)
    python panzer.py join <ip[:port]>   Einem Spiel beitreten
    python panzer.py local [2|3]        2 oder 3 Spieler an einer Tastatur

Netzwerk: Der Host ist autoritativ und simuliert mit festen 60 Ticks/s. Bis zu 2 Clients
können beitreten (3 Spieler). Der Host schickt 30 Snapshots/s, jeder Client schickt
30 Pakete/s mit allen Eingaben seit dem letzten Paket.
Der Client sagt seinen eigenen Panzer voraus (Prediction + Abgleich über "ack") und zeigt
den Rest leicht verzögert und interpoliert an, damit unregelmäßig ankommende Pakete nicht ruckeln.
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
from collections import deque
from pathlib import Path

import pygame

# --- Konstanten -------------------------------------------------------------

PORT = 5555
PROTO = 2  # Protokollversion, Host und Client müssen übereinstimmen
MAX_PLAYERS = 3
W, H = 1000, 680
ARENA_TOP, ARENA_H = 15, 560
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
POWERUPS = ["frag", "mg", "laser", "rocket", "shield"]
POWERUP_NAMES = {
    "frag": "Splitterbombe",
    "mg": "MG",
    "laser": "Laser",
    "rocket": "Lenkrakete",
    "shield": "Schild",
}
FRAG_R, FRAG_SPEED, FRAG_FUSE, FRAG_PIECES = 7, 170, 5.0, 26
MG_AMMO, MG_RATE, MG_SPREAD, MG_R, MG_LIFE = 25, 0.07, 0.12, 3, 4.0
LASER_LEN, LASER_SHOW = 1400, 0.45
ROCKET_R, ROCKET_SPEED, ROCKET_TURN, ROCKET_LIFE = 5, 170, 4.0, 10.0
ROCKET_DELAY, ROCKET_GRACE = 0.4, 0.8
SMOKE_LIFE = 0.7
SHIELD_TIME = 8.0

COLORS = [(220, 35, 35), (35, 190, 45), (45, 95, 225)]
NAMES = ["Rot", "Grün", "Blau"]
DEAD_SPRITE = len(COLORS)
BG = (255, 255, 255)
FLOOR = (228, 228, 228)
WALL_C = (77, 77, 77)
TEXT_C = (40, 40, 40)
SHOT_C = (30, 30, 30)

UP, DOWN, LEFT, RIGHT, FIRE = 1, 2, 4, 8, 16

LAST_IP_FILE = Path(__file__).with_name(".panzer_last_ip")


# --- Labyrinth & Geometrie --------------------------------------------------


def generate_maze(maze_id):
    cols, rows = random.randint(6, 10), random.randint(4, 6)
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

    ox = (W - cols * CELL) // 2
    oy = ARENA_TOP + (ARENA_H - rows * CELL) // 2
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
    return {
        "id": maze_id,
        "cols": cols,
        "rows": rows,
        "ox": ox,
        "oy": oy,
        "walls": walls,
    }


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
        self.walls = [tuple(w) for w in maze["walls"]]
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

    def raycast(self, x, y, a, length, circles=()):
        """Abprallender Strahl. Gibt (Punkte, Index des getroffenen Kreises oder None) zurück."""
        dx, dy = math.cos(a), math.sin(a)
        pts = [(x, y)]
        for _ in range(40):
            if length <= 0.5:
                break
            t_wall, axis = length, None
            for w in self.walls:
                t, ax = ray_box(x, y, dx, dy, *w)
                if t is not None and t < t_wall:
                    t_wall, axis = t, ax
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


def move_tank(t, keys, dt, arena):
    """Panzerbewegung – identisch auf Host und Client (Prediction)."""
    if keys & LEFT:
        t.a -= TANK_ROT * dt
    if keys & RIGHT:
        t.a += TANK_ROT * dt
    t.a %= 2 * math.pi
    move = (1 if keys & UP else 0) - (0.7 if keys & DOWN else 0)
    if move:
        t.x += math.cos(t.a) * TANK_SPEED * move * dt
        t.y += math.sin(t.a) * TANK_SPEED * move * dt
        t.x, t.y = push_out(t.x, t.y, TANK_R, arena.near(t.x, t.y))


# --- Spiellogik (läuft nur beim Host bzw. lokal) -----------------------------


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
    )

    def __init__(self, sid, kind, owner, x, y, a, speed, r, life, grace):
        self.id, self.kind, self.owner = sid, kind, owner
        self.x, self.y = x, y
        self.vx, self.vy = math.cos(a) * speed, math.sin(a) * speed
        self.speed, self.r, self.life, self.grace = speed, r, life, grace
        self.age = 0.0

    def travel(self, dt, arena):
        """Bewegt die Kugel, prallt achsweise an Wänden ab (Substeps von max. 3 px)."""
        steps = max(1, math.ceil(self.speed * dt / 3))
        f = dt / steps
        for _ in range(steps):
            self.x += self.vx * f
            if circle_hits_walls(self.x, self.y, self.r, arena.near(self.x, self.y)):
                self.x -= self.vx * f
                self.vx = -self.vx
            self.y += self.vy * f
            if circle_hits_walls(self.x, self.y, self.r, arena.near(self.x, self.y)):
                self.y -= self.vy * f
                self.vy = -self.vy


class Fx:
    def __init__(self, kind, data, life):
        self.kind, self.data, self.life, self.age = kind, data, life, 0.0


class Game:
    def __init__(self, players=(0, 1)):
        self.players = list(players)
        self.scores = [0] * MAX_PLAYERS
        self.maze_id = 0
        self.time = 0.0
        self.next_id = 0
        self.fire_seen = [None] * MAX_PLAYERS
        self.new_round()

    def set_players(self, players, fresh=()):
        """Neue Spielerliste -> sofort neue Runde. Neue Spieler (fresh) starten bei 0 Punkten."""
        for slot in fresh:
            self.scores[slot] = 0
            self.fire_seen[slot] = None  # neuer Client zählt seine Schüsse ab 0
        self.players = list(players)
        self.new_round()

    def new_round(self):
        self.maze_id += 1
        self.maze = generate_maze(self.maze_id)
        self.arena = Arena(self.maze)
        m = self.maze
        cells = [(r, c) for r in range(m["rows"]) for c in range(m["cols"])]
        # Startfelder mit möglichst großem Abstand, wenn es nicht klappt etwas lockerer
        min_dist = (m["rows"] + m["cols"]) // 2
        for attempt in range(400):
            spots = random.sample(cells, len(self.players))
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
        self.box_timer = BOX_FIRST
        self.end_timer = None
        self.msg = None

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
        move_tank(t, keys, dt, self.arena)
        t.cooldown = max(0.0, t.cooldown - dt)
        if new_shot:
            self._trigger(i, t)
        if t.weapon == "mg" and keys & FIRE and t.cooldown == 0:
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
            t.ammo -= 1
            if t.ammo <= 0:
                t.weapon = None

    def _shoot(self, kind, owner, t, a, speed, r, life, grace):
        s = Shot(self.next_id, kind, owner, t.x, t.y, a, speed, r, life, grace)
        self.next_id += 1
        s.travel(
            MUZZLE / speed, self.arena
        )  # von der Mitte bis zum Rohrende, prallt korrekt ab
        self.shots.append(s)

    def _trigger(self, i, t):
        bomb = next((s for s in self.shots if s.kind == "frag" and s.owner == i), None)
        if bomb:  # 2. Druck: Splitterbombe zünden
            self._explode(bomb)
            return
        if t.cooldown > 0 or t.weapon == "mg":
            return
        if t.weapon == "frag":
            self._shoot("frag", i, t, t.a, FRAG_SPEED, FRAG_R, FRAG_FUSE, 0.3)
        elif t.weapon == "rocket":
            self._shoot(
                "rocket", i, t, t.a, ROCKET_SPEED, ROCKET_R, ROCKET_LIFE, ROCKET_GRACE
            )
        elif t.weapon == "laser":
            self._laser(i, t)
        elif sum(1 for s in self.shots if s.owner == i and s.kind == "b") < MAX_BULLETS:
            self._shoot(
                "b", i, t, t.a, BULLET_SPEED, BULLET_R, BULLET_LIFE, OWNER_GRACE
            )
        else:
            return
        t.weapon = None
        t.cooldown = FIRE_COOLDOWN

    def _explode(self, bomb):
        self.shots.remove(bomb)
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
        self.fx.append(Fx("boom", [round(bomb.x), round(bomb.y)], 0.5))

    def _laser(self, i, t):
        targets = [o for o in self.tanks if o.alive]
        circles = [(o.x, o.y, TANK_R) for o in targets]
        pts, hit = self.arena.raycast(t.x, t.y, t.a, LASER_LEN, circles)
        if hit is not None:
            self._hit(targets[hit])
        self.fx.append(Fx("laser", [[round(x), round(y)] for x, y in pts], LASER_SHOW))

    def _hit(self, t):
        if t.shield > 0:
            t.shield = 0.0
            return
        t.alive = False
        t.weapon = None
        self.fx.append(Fx("boom", [round(t.x), round(t.y)], 0.7))

    def _rocket_steer(self, s, dt):
        enemies = [t for t in self.tanks if t.alive and t.slot != s.owner]
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
        cur = math.atan2(s.vy, s.vx)
        diff = (math.atan2(ty - s.y, tx - s.x) - cur + math.pi) % (
            2 * math.pi
        ) - math.pi
        cur += max(-ROCKET_TURN * dt, min(ROCKET_TURN * dt, diff))
        s.vx, s.vy = math.cos(cur) * s.speed, math.sin(cur) * s.speed

    def _spawn_box(self):
        cells = [(r, c) for r in range(self.arena.rows) for c in range(self.arena.cols)]
        random.shuffle(cells)
        for r, c in cells:
            x, y = self.arena.center(r, c)
            if any(t.alive and math.hypot(t.x - x, t.y - y) < 110 for t in self.tanks):
                continue
            if any(math.hypot(b[0] - x, b[1] - y) < 1 for b in self.boxes):
                continue
            self.boxes.append([x, y, random.choice(POWERUPS)])
            return

    def step(self, dt):
        self.time += dt
        for t in self.tanks:
            if not t.alive:
                t.dead_age += dt
            t.shield = max(0.0, t.shield - dt)

        # Panzer gegeneinander
        for t1, t2 in itertools.combinations(self.tanks, 2):
            if not (t1.alive and t2.alive):
                continue
            dx, dy = t2.x - t1.x, t2.y - t1.y
            d = math.hypot(dx, dy)
            if 1e-6 < d < 2 * TANK_R:
                p = (2 * TANK_R - d) / 2
                t1.x -= dx / d * p
                t1.y -= dy / d * p
                t2.x += dx / d * p
                t2.y += dy / d * p
                for t in (t1, t2):
                    t.x, t.y = push_out(t.x, t.y, TANK_R, self.arena.near(t.x, t.y))

        # Power-up-Kisten
        self.box_timer -= dt
        if self.box_timer <= 0:
            self.box_timer = random.uniform(*BOX_EVERY)
            if len(self.boxes) < BOX_MAX and self.end_timer is None:
                self._spawn_box()
        for b in self.boxes[:]:
            for t in self.tanks:
                if not t.alive or math.hypot(t.x - b[0], t.y - b[1]) > BOX_PICK:
                    continue
                if b[2] == "shield":
                    t.shield = SHIELD_TIME
                elif t.weapon is None:
                    t.weapon = b[2]
                    t.ammo = MG_AMMO
                else:
                    continue  # hat schon eine Waffe -> Kiste bleibt liegen
                self.boxes.remove(b)
                break

        # Geschosse
        for s in self.shots[:]:
            s.age += dt
            if s.age > s.life:
                if s.kind == "frag":
                    self._explode(s)
                else:
                    self.shots.remove(s)
                    if s.kind == "rocket":  # Treibstoff alle -> kleine Explosion
                        self.fx.append(Fx("boom", [round(s.x), round(s.y), 0.5], 0.4))
                continue
            if s.kind == "rocket" and s.age > ROCKET_DELAY:
                self._rocket_steer(s, dt)
            s.travel(dt, self.arena)
            for t in self.tanks:
                if not t.alive or (t.slot == s.owner and s.age < s.grace):
                    continue
                if math.hypot(t.x - s.x, t.y - s.y) < TANK_R + s.r - 1:
                    self.shots.remove(s)
                    self._hit(t)
                    break

        for f in self.fx[:]:
            f.age += dt
            if f.age > f.life:
                self.fx.remove(f)

        # Rundenende
        alive = [t.slot for t in self.tanks if t.alive]
        if self.end_timer is None and len(alive) <= 1:
            self.end_timer = ROUND_END_DELAY
        if self.end_timer is not None:
            self.end_timer -= dt
            if self.end_timer < ROUND_END_DELAY - 1:
                self.msg = (
                    f"{NAMES[alive[0]]} gewinnt die Runde!"
                    if alive
                    else "Unentschieden!"
                )
            if self.end_timer <= 0:
                if len(alive) == 1:
                    self.scores[alive[0]] += 1
                self.new_round()

    def reload_state(self, t):
        """(verfügbare Schuss, Sekunden bis wieder eine Kugel frei ist)."""
        if t.weapon == "mg":
            return t.ammo, 0.0
        left = [
            s.life - s.age for s in self.shots if s.owner == t.slot and s.kind == "b"
        ]
        free = MAX_BULLETS - len(left)
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
            "fx": [[f.kind, round(f.age, 2), f.life, f.data] for f in self.fx],
            "sc": self.scores,
            "msg": self.msg,
        }


    def _tank_state(self, t):
        # [x, y, a, alive, dead_age, weapon, shield, slot, ammo, reload]
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
        ]

    @staticmethod
    def _shot_state(s):
        p = [s.id, s.kind, round(s.x, 1), round(s.y, 1)]
        if s.kind == "rocket":  # Rakete braucht Flugrichtung und Besitzer zum Zeichnen
            p += [round(math.atan2(s.vy, s.vx), 3), s.owner]
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
        self.gen = 0  # zählt bei jedem Join/Leave hoch -> Host passt die Spielerliste an
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
                self.error = "Das Spiel ist schon voll (3 Spieler)."
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
        s = [p[0], p[1], q[2] + (p[2] - q[2]) * k, q[3] + (p[3] - q[3]) * k]
        if len(p) > 4:  # Rakete: Winkel und Besitzer
            s += [lerp_angle(q[4], p[4], k), p[5]]
        shots.append(s)
    return dict(b, tk=tanks, p=shots)


# --- Grafik ------------------------------------------------------------------


def make_tank_sprite(color):
    s = pygame.Surface((44, 44), pygame.SRCALPHA)
    dark = tuple(int(c * 0.55) for c in color)
    for y in (7, 30):  # Ketten
        pygame.draw.rect(s, dark, (6, y, 32, 7), border_radius=2)
    pygame.draw.rect(s, color, (8, 11, 28, 22))
    pygame.draw.rect(s, dark, (8, 11, 28, 22), 2)
    pygame.draw.rect(s, color, (22, 19, 20, 6))
    pygame.draw.rect(s, dark, (22, 19, 20, 6), 1)
    pygame.draw.circle(s, color, (22, 22), 8)
    pygame.draw.circle(s, dark, (22, 22), 8, 2)
    return s.convert_alpha()


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
    pygame.draw.polygon(big, (175, 178, 190), tf([(-10, 1.5), (7, 1.5), (7, 4), (-10, 4)]))
    pygame.draw.polygon(big, color, tf([(-7, -4), (-5, -4), (-5, 4), (-7, 4)]))  # Ring
    pygame.draw.polygon(big, color, tf(ROCKET_NOSE))
    pygame.draw.polygon(big, outline, tf(ROCKET_BODY[:1] + ROCKET_NOSE + ROCKET_BODY[3:]), ss)
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


def draw_icon(surf, kind, cx, cy, k=1.0):
    """Power-up-Symbol, k = Skalierung."""

    def p(v):
        return max(1, round(v * k))

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
        draw_rocket(surf, cx + 2 * k, cy - 2 * k, -math.pi / 4, (235, 120, 20), 0.7 * k, 4)
    elif kind == "shield":
        pygame.draw.circle(surf, (40, 120, 230), (cx, cy), p(8), p(2.5))


def make_box_sprite(kind):
    s = pygame.Surface((30, 30), pygame.SRCALPHA)
    pygame.draw.rect(s, (150, 150, 150), (2, 2, 26, 26), border_radius=4)
    pygame.draw.rect(s, (95, 95, 95), (2, 2, 26, 26), 2, border_radius=4)
    draw_icon(s, kind, 15, 15, 1.0)
    return s.convert_alpha()


def make_glow():
    s = pygame.Surface((70, 70), pygame.SRCALPHA)
    for r in range(35, 0, -3):
        pygame.draw.circle(s, (140, 80, 210, int(70 * (1 - r / 35)) + 8), (35, 35), r)
    return s.convert_alpha()


class Renderer:
    def __init__(self, screen):
        self.screen = screen
        self.font = pygame.font.Font(None, 28)
        self.big = pygame.font.Font(None, 48)
        self.score_font = pygame.font.Font(None, 64)
        self.small = pygame.font.Font(None, 22)
        self.sprites = [make_tank_sprite(c) for c in COLORS] + [
            make_tank_sprite((90, 90, 90))
        ]
        self.icons = [
            pygame.transform.rotozoom(s, 0, 1.6) for s in self.sprites[:DEAD_SPRITE]
        ]
        self.boxes = {k: make_box_sprite(k) for k in POWERUPS}
        self.glow = make_glow()
        self.rot_cache = {}
        self.text_cache = {}
        self.puffs = {}  # (radius, alpha-stufe) -> Rauchwolke
        self.trails = {}  # Raketen-id -> [(x, y, zeit), ...]
        self.trail_maze = None
        self.maze_surf = None
        self.maze_surf_id = None
        self.debug = False

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

    def rotated(self, idx, a):
        # auf 3° gerundet und gecacht -> rotozoom nur einmal pro Winkel statt jeden Frame
        step = round(math.degrees(a) / 3) % 120
        surf = self.rot_cache.get((idx, step))
        if surf is None:
            surf = self.rot_cache[(idx, step)] = pygame.transform.rotozoom(
                self.sprites[idx], -step * 3, 1
            )
        return surf

    def _maze_surface(self, arena):
        if self.maze_surf_id != arena.id:
            surf = pygame.Surface((W, ARENA_TOP + ARENA_H + 10))
            surf.fill(BG)
            pygame.draw.rect(
                surf, FLOOR, (arena.ox, arena.oy, arena.cols * CELL, arena.rows * CELL)
            )
            for w in arena.walls:
                pygame.draw.rect(surf, WALL_C, w)
            self.maze_surf, self.maze_surf_id = surf.convert(), arena.id
        return self.maze_surf

    def puff(self, r, alpha):
        key = (r, alpha // 16)
        surf = self.puffs.get(key)
        if surf is None:
            surf = pygame.Surface((2 * r + 2, 2 * r + 2), pygame.SRCALPHA)
            pygame.draw.circle(surf, (150, 150, 150, key[1] * 16), (r + 1, r + 1), r)
            surf = self.puffs[key] = surf.convert_alpha()
        return surf

    def smoke(self, arena, shots, now):
        """Rauchspur hinter Raketen. Läuft rein clientseitig, bleibt kurz nach dem Einschlag stehen."""
        if self.trail_maze != arena.id:
            self.trails.clear()
            self.trail_maze = arena.id
        for p in shots:
            if p[1] != "rocket":
                continue
            _, _, x, y, a, _ = p
            tx, ty = x - math.cos(a) * 13, y - math.sin(a) * 13
            trail = self.trails.setdefault(p[0], [])
            if not trail or math.hypot(trail[-1][0] - tx, trail[-1][1] - ty) > 3:
                trail.append((tx, ty, now))
        for sid in list(self.trails):
            trail = self.trails[sid] = [q for q in self.trails[sid] if now - q[2] < SMOKE_LIFE]
            if not trail:
                del self.trails[sid]
                continue
            for x, y, t0 in trail:
                k = (now - t0) / SMOKE_LIFE
                r = round(2 + 6 * k)
                self.screen.blit(self.puff(r, int(170 * (1 - k))), (x - r - 1, y - r - 1))

    def reload_ring(self, x, y, reload, show_time):
        """Ring, der sich leert, bis wieder eine Kugel frei ist."""
        frac = min(reload / BULLET_LIFE, 1)
        rect = pygame.Rect(0, 0, 2 * TANK_R + 20, 2 * TANK_R + 20)
        rect.center = (round(x), round(y))
        pygame.draw.circle(self.screen, (200, 200, 200), rect.center, rect.w // 2, 3)
        pygame.draw.arc(
            self.screen,
            (90, 90, 90),
            rect,
            math.pi / 2,
            math.pi / 2 + 2 * math.pi * frac,
            3,
        )
        if show_time:
            self.text(f"{reload:.1f}", (x, y - TANK_R - 22), self.small, (90, 90, 90))

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

    def draw(self, arena, snap, me=None, hint="", info=""):
        scr = self.screen
        scr.fill(BG)
        if arena:
            scr.blit(self._maze_surface(arena), (0, 0))
        if snap and arena and snap["m"] == arena.id:
            pulse = 0.5 + 0.5 * math.sin(pygame.time.get_ticks() / 250)
            self.glow.set_alpha(int(140 + 115 * pulse))
            for x, y, kind in snap["bx"]:
                scr.blit(self.glow, (x - 35, y - 35))
                scr.blit(self.boxes[kind], (x - 15, y - 15))

            now = pygame.time.get_ticks() / 1000
            self.smoke(arena, snap["p"], now)
            for sid, kind, x, y, *extra in snap["p"]:
                if kind == "frag":
                    pygame.draw.circle(scr, SHOT_C, (x, y), FRAG_R)
                    pygame.draw.circle(scr, (120, 120, 120), (x - 2, y - 2), 2)
                elif kind == "rocket":
                    a, owner = extra
                    flame = 9 + 4 * math.sin(now * 45 + sid)
                    draw_rocket(scr, x, y, a, COLORS[owner], 1.0, flame)
                else:
                    pygame.draw.circle(
                        scr, SHOT_C, (x, y), {"b": BULLET_R, "mg": MG_R}.get(kind, 2)
                    )

            for x, y, a, alive, dead_age, weapon, shield, i, ammo, reload in snap["tk"]:
                rot = self.rotated(i if alive else DEAD_SPRITE, a)
                scr.blit(rot, rot.get_rect(center=(x, y)))
                if not alive:
                    continue
                if reload > 0 and not weapon:
                    # lokal: Sekunden bei allen, im Netz nur beim eigenen Panzer
                    self.reload_ring(x, y, reload, me is None or i == me)
                if shield > 0 and (
                    shield > 2 or int(shield * 6) % 2
                ):  # blinkt kurz vor Ablauf
                    pygame.draw.circle(scr, (40, 120, 230), (x, y), TANK_R + 7, 2)
                if weapon:
                    draw_icon(scr, weapon, int(x), int(y - 30), 0.8)
                    if i == me and weapon == "laser":  # Zielhilfe
                        self.dotted(arena.raycast(x, y, a, 320)[0], (230, 90, 90))

            for kind, age, life, data in snap["fx"]:
                k = min(age / life, 1)
                if kind == "laser":
                    c = (255, int(40 + 200 * k), int(40 + 200 * k))
                    pygame.draw.lines(scr, c, False, data, max(1, round(4 * (1 - k))))
                elif kind == "boom":
                    x, y, *size = data
                    z = size[0] if size else 1
                    boom = pygame.Surface((160, 160), pygame.SRCALPHA)
                    pygame.draw.circle(
                        boom,
                        (255, 150, 30, int(220 * (1 - k))),
                        (80, 80),
                        int((12 + 60 * k) * z),
                    )
                    pygame.draw.circle(
                        boom,
                        (255, 230, 90, int(255 * (1 - k))),
                        (80, 80),
                        int((6 + 30 * k) * z),
                    )
                    scr.blit(boom, (x - 80, y - 80))

            if snap.get("msg"):
                center = (W // 2, ARENA_TOP + ARENA_H // 2)
                surf = self.big.render(snap["msg"], True, TEXT_C)
                pad = surf.get_rect(center=center).inflate(30, 16)
                box = pygame.Surface(pad.size, pygame.SRCALPHA)
                box.fill((255, 255, 255, 210))
                scr.blit(box, pad)
                self.text(snap["msg"], center, self.big)
        if snap:
            self._hud(snap, me)
        if hint:
            self.text(hint, (W // 2, H - 14), self.small, (140, 140, 140))
        if self.debug and info:
            self.text(info, (8, 4), self.small, (200, 0, 120), center=False)

    def _hud(self, snap, me):
        y = ARENA_TOP + ARENA_H + 42
        n = len(snap["tk"])
        gap = 440 if n <= 2 else 310
        for k, tank in enumerate(snap["tk"]):
            alive, weapon, i, ammo, reload = tank[3], tank[5], tank[7], tank[8], tank[9]
            x = round(W // 2 + (k - (n - 1) / 2) * gap)
            icon = self.icons[i]
            self.screen.blit(icon, icon.get_rect(center=(x - 50, y)))
            self.text(str(snap["sc"][i]), (x + 10, y), self.score_font)
            if weapon and alive:
                self.text(
                    POWERUP_NAMES[weapon], (x - 50, y - 34), self.small, (110, 110, 110)
                )
            # Munition: Kugeln als Punkte (voll = bereit), MG als Zahl, darunter Nachladezeit
            col = x + 42
            if weapon == "mg":
                self.text(f"MG {ammo}", (col, y - 22), self.small, TEXT_C, center=False)
            else:
                for b in range(MAX_BULLETS):
                    c = (col + 4 + b * 11, y - 15)
                    if b < ammo:
                        pygame.draw.circle(self.screen, SHOT_C, c, 4)
                    else:
                        pygame.draw.circle(self.screen, (170, 170, 170), c, 4, 1)
            if reload > 0 and alive:
                self.text(
                    f"Nachladen {reload:.1f} s",
                    (col, y - 6),
                    self.small,
                    (90, 90, 90),
                    center=False,
                )
            if me == i:
                self.text("Du", (col, y + 11), self.font, COLORS[i], center=False)


# --- Eingabe -----------------------------------------------------------------


class KeyInput:
    """Tastenbelegung -> (bits, schuss_zähler)."""

    def __init__(self, up, down, left, right, fire):
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
        return bits, self.fires


def net_input():
    return KeyInput(
        (pygame.K_UP, pygame.K_w),
        (pygame.K_DOWN, pygame.K_s),
        (pygame.K_LEFT, pygame.K_a),
        (pygame.K_RIGHT, pygame.K_d),
        (pygame.K_SPACE, pygame.K_m, pygame.K_q),
    )


NET_HINT = "Pfeiltasten/WASD fahren  ·  Leertaste schießen  ·  F3 Ping/FPS  ·  F11 Vollbild  ·  Esc Menü"


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
        elif event.key == pygame.K_ESCAPE:
            return True
    return False


# --- Modi --------------------------------------------------------------------


def run_local(screen, clock, rend, players=2):
    game = Game(range(players))
    inputs = [
        KeyInput(
            (pygame.K_e,), (pygame.K_d,), (pygame.K_s,), (pygame.K_f,), (pygame.K_q,)
        ),
        KeyInput(
            (pygame.K_UP,),
            (pygame.K_DOWN,),
            (pygame.K_LEFT,),
            (pygame.K_RIGHT,),
            (pygame.K_m,),
        ),
        KeyInput(
            (pygame.K_i, pygame.K_KP8),
            (pygame.K_k, pygame.K_KP5),
            (pygame.K_j, pygame.K_KP4),
            (pygame.K_l, pygame.K_KP6),
            (pygame.K_u, pygame.K_KP0),
        ),
    ][:players]
    hint = "Rot: ESDF + Q  ·  Grün: Pfeile + M"
    if players == 3:
        hint += "  ·  Blau: IJKL + U (Numpad 8456 + 0)"
    hint += "  ·  Esc Menü"
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
            game.step(TICK)
        rend.draw(
            game.arena, game.snapshot(), hint=hint, info=f"{clock.get_fps():.0f} FPS"
        )
        pygame.display.flip()


def run_host(screen, clock, rend, port=PORT):
    try:
        net = HostNet(port)
    except OSError as e:
        return message(screen, clock, rend, f"Port {port} geht nicht: {e}")
    ip = local_ip()
    inp = net_input()
    game, gen, sent_maze, peers = None, -1, None, {}
    acc, ticks = 0.0, 0
    try:
        while True:
            acc = min(acc + clock.tick(FPS) / 1000, 0.25)
            for e in pygame.event.get():
                if common_events(e, rend):
                    return
                inp.handle(e)

            if gen != net.gen:  # jemand ist beigetreten oder gegangen
                gen, old = net.gen, peers
                peers = net.clients()
                players = [0] + sorted(peers)
                fresh = [s for s, p in peers.items() if old.get(s) is not p]
                if not peers:
                    game = None
                elif game is None:
                    game, acc = Game(players), 0.0
                else:
                    game.set_players(players, fresh)
                sent_maze = None

            if game is None:
                rend.draw(None, None, hint="Esc Menü")
                rend.text("Warte auf Mitspieler …", (W // 2, 250), rend.big)
                rend.text(f"Deine IP:  {ip}   Port: {port}", (W // 2, 320))
                rend.text(
                    'Bis zu 2 Freunde wählen "Beitreten" und geben diese IP ein.',
                    (W // 2, 360),
                    rend.small,
                    (110, 110, 110),
                )
                rend.text(
                    "Nicht im selben Netz? Tailscale/ZeroTier-IP oder Port-Forwarding (TCP).",
                    (W // 2, 385),
                    rend.small,
                    (110, 110, 110),
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
                game.step(TICK)
                if sent_maze != game.maze_id:
                    net.broadcast({"t": "m", "maze": game.maze})
                    sent_maze = game.maze_id
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
                    rend.small,
                    (150, 150, 150),
                    center=False,
                    right=True,
                )
            pygame.display.flip()
    finally:
        net.close()


class Predicted:
    def __init__(self, x, y, a):
        self.x, self.y, self.a = x, y, a


def run_client(screen, clock, rend, addr):
    host, _, port = addr.strip().partition(":")
    port = int(port) if port.isdigit() else PORT
    rend.draw(None, None)
    rend.text(f"Verbinde mit {host}:{port} …", (W // 2, 300), rend.big)
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
                    p = Predicted(me[0], me[1], me[2])
                    for _, k, d, _ in pending:
                        move_tank(p, k, d, arena)
                    me = [p.x, p.y, p.a] + me[3:]
                if me:
                    view = dict(
                        view, tk=[me if t[7] == net.slot else t for t in view["tk"]]
                    )
            info = f"{clock.get_fps():.0f} FPS  ·  Ping {ping:.0f} ms" if ping else ""
            rend.draw(arena, view, me=net.slot, hint=NET_HINT, info=info)
            if view is None:
                rend.text("Warte auf Spielstart …", (W // 2, 300), rend.big)
            pygame.display.flip()
    finally:
        net.close()


def message(screen, clock, rend, txt):
    while True:
        clock.tick(30)
        for e in pygame.event.get():
            common_events(e, rend)
            if e.type in (pygame.KEYDOWN, pygame.MOUSEBUTTONDOWN):
                return
        rend.draw(None, None)
        rend.text(txt, (W // 2, 300))
        rend.text("Beliebige Taste …", (W // 2, 350), rend.small, (130, 130, 130))
        pygame.display.flip()


def menu(screen, clock, rend):
    """Gibt ("host",) / ("join", addr) / ("local", spieler) zurück."""
    try:
        ip_text = LAST_IP_FILE.read_text().strip()
    except OSError:
        ip_text = ""
    buttons = {
        "host": pygame.Rect(W // 2 - 160, 230, 320, 56),
        "join": pygame.Rect(W // 2 - 160, 390, 320, 56),
        "local2": pygame.Rect(W // 2 - 160, 500, 155, 56),
        "local3": pygame.Rect(W // 2 + 5, 500, 155, 56),
    }
    labels = {
        "host": "Spiel hosten",
        "join": "Beitreten",
        "local2": "2 Spieler",
        "local3": "3 Spieler",
    }
    ip_box = pygame.Rect(W // 2 - 160, 320, 320, 50)
    pygame.key.start_text_input()

    def join():
        if ip_text:
            try:
                LAST_IP_FILE.write_text(ip_text)
            except OSError:
                pass
            return ("join", ip_text)

    while True:
        clock.tick(30)
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
                for key, rect in buttons.items():
                    if rect.collidepoint(e.pos):
                        if key == "join":
                            if join():
                                return join()
                        elif key.startswith("local"):
                            return ("local", int(key[-1]))
                        else:
                            return (key,)

        screen.fill(BG)
        rend.text("PANZER", (W // 2, 90), rend.score_font)
        for i, spr in enumerate(rend.icons):
            screen.blit(spr, spr.get_rect(center=(W // 2 + (i - 1) * 85, 165)))
        rend.text(
            "Bis zu 3 Spieler: Host spielt Rot, wer beitritt Grün bzw. Blau",
            (W // 2, 302),
            rend.small,
            (130, 130, 130),
        )
        rend.text(
            "Lokal an einer Tastatur", (W // 2, 482), rend.small, (130, 130, 130)
        )
        pygame.draw.rect(screen, (245, 245, 245), ip_box)
        pygame.draw.rect(screen, WALL_C, ip_box, 2)
        if ip_text:
            rend.text(
                ip_text + ("|" if pygame.time.get_ticks() // 500 % 2 else ""),
                ip_box.center,
            )
        else:
            rend.text(
                "IP-Adresse des Hosts eingeben", ip_box.center, color=(160, 160, 160)
            )
        for key, rect in buttons.items():
            hover = rect.collidepoint(mouse)
            pygame.draw.rect(
                screen, (95, 95, 95) if hover else WALL_C, rect, border_radius=6
            )
            rend.text(labels[key], rect.center, color=(255, 255, 255))
        rend.text("Esc beendet", (W // 2, H - 30), rend.small, (150, 150, 150))
        pygame.display.flip()


def main():
    pygame.init()
    pygame.display.set_caption("Panzer")
    screen = pygame.display.set_mode((W, H), pygame.SCALED | pygame.RESIZABLE)
    clock = pygame.time.Clock()
    rend = Renderer(screen)

    args = sys.argv[1:]
    action = None
    if args[:1] == ["host"]:
        action = ("host", int(args[1]) if len(args) > 1 else PORT)
    elif args[:1] == ["join"] and len(args) > 1:
        action = ("join", args[1])
    elif args[:1] == ["local"]:
        n = int(args[1]) if len(args) > 1 and args[1] in ("2", "3") else 2
        action = ("local", n)

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
