#!/usr/bin/env python3
"""Panzer – AZ/Tank-Trouble-Klon mit Multiplayer über IP.

    python panzer.py                    Menü
    python panzer.py host [port]        Spiel hosten (Standard-Port 5555)
    python panzer.py join <ip[:port]>   Einem Spiel beitreten
    python panzer.py local              2 Spieler an einer Tastatur

Der Host ist autoritativ: er simuliert das Spiel, der Client schickt nur seine
Tasten und bekommt jeden Frame den kompletten Zustand zurück (JSON-Zeilen über TCP).
"""
import json
import math
import random
import socket
import sys
import threading
from pathlib import Path

import pygame

# --- Konstanten -------------------------------------------------------------

PORT = 5555
W, H = 1000, 680
ARENA_TOP, ARENA_H = 15, 560
FPS = 60

CELL = 90
WALL = 8
TANK_R = 15
TANK_SPEED = 130          # px/s
TANK_ROT = math.radians(200)  # rad/s
MUZZLE = 20               # Abstand Mitte -> Rohrende
BULLET_R = 4
BULLET_SPEED = 220
BULLET_LIFE = 10.0
MAX_BULLETS = 5
FIRE_COOLDOWN = 0.12
OWNER_GRACE = 0.1         # so lange kann man sich nicht selbst treffen
ROUND_END_DELAY = 3.0

COLORS = [(220, 35, 35), (35, 190, 45)]
NAMES = ["Rot", "Grün"]
BG = (255, 255, 255)
FLOOR = (228, 228, 228)
WALL_C = (77, 77, 77)
TEXT_C = (40, 40, 40)

UP, DOWN, LEFT, RIGHT = 1, 2, 4, 8

LAST_IP_FILE = Path(__file__).with_name(".panzer_last_ip")


# --- Labyrinth --------------------------------------------------------------

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
        nbrs = [(r + dr, c + dc, dr, dc) for dr, dc in ((-1, 0), (1, 0), (0, -1), (0, 1))
                if 0 <= r + dr < rows and 0 <= c + dc < cols and not seen[r + dr][c + dc]]
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
                walls.append([ox + start * CELL - half, oy + r * CELL - half,
                              (c - start) * CELL + WALL, WALL])
            else:
                c += 1
    for c in range(cols + 1):
        r = 0
        while r < rows:
            if v[r][c]:
                start = r
                while r < rows and v[r][c]:
                    r += 1
                walls.append([ox + c * CELL - half, oy + start * CELL - half,
                              WALL, (r - start) * CELL + WALL])
            else:
                r += 1
    return {"id": maze_id, "cols": cols, "rows": rows, "ox": ox, "oy": oy, "walls": walls}


def circle_hits_rect(cx, cy, r, rect):
    x, y, w, h = rect
    nx = min(max(cx, x), x + w)
    ny = min(max(cy, y), y + h)
    return (cx - nx) ** 2 + (cy - ny) ** 2 < r * r


def circle_hits_walls(cx, cy, r, walls):
    return any(circle_hits_rect(cx, cy, r, w) for w in walls)


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


# --- Spiellogik (läuft nur beim Host bzw. lokal) -----------------------------

class Tank:
    def __init__(self, x, y, a):
        self.x, self.y, self.a = x, y, a
        self.alive = True
        self.dead_age = 0.0
        self.cooldown = 0.0
        self.fire_seen = None


class Bullet:
    def __init__(self, owner, x, y, a):
        self.owner = owner
        self.x, self.y = x, y
        self.vx, self.vy = math.cos(a) * BULLET_SPEED, math.sin(a) * BULLET_SPEED
        self.age = 0.0

    def travel(self, dist, walls):
        """Bewegt die Kugel um dist Pixel, prallt achsweise an Wänden ab."""
        steps = max(1, math.ceil(dist / 2))
        f = dist / BULLET_SPEED / steps
        for _ in range(steps):
            self.x += self.vx * f
            if circle_hits_walls(self.x, self.y, BULLET_R, walls):
                self.x -= self.vx * f
                self.vx = -self.vx
            self.y += self.vy * f
            if circle_hits_walls(self.x, self.y, BULLET_R, walls):
                self.y -= self.vy * f
                self.vy = -self.vy


class Game:
    def __init__(self):
        self.scores = [0, 0]
        self.maze_id = 0
        self.fire_seen = [None, None]
        self.new_round()

    def new_round(self):
        self.maze_id += 1
        self.maze = generate_maze(self.maze_id)
        self.walls = self.maze["walls"]
        m = self.maze
        cells = [(r, c) for r in range(m["rows"]) for c in range(m["cols"])]
        min_dist = (m["rows"] + m["cols"]) // 2
        for _ in range(200):
            a, b = random.sample(cells, 2)
            if abs(a[0] - b[0]) + abs(a[1] - b[1]) >= min_dist:
                break
        self.tanks = []
        for i, (r, c) in enumerate((a, b)):
            t = Tank(m["ox"] + c * CELL + CELL / 2, m["oy"] + r * CELL + CELL / 2,
                     random.choice((0, 0.5, 1, 1.5)) * math.pi)
            t.fire_seen = self.fire_seen[i]
            self.tanks.append(t)
        self.bullets = []
        self.end_timer = None
        self.msg = None

    def update(self, dt, inputs):
        """inputs: pro Spieler (tasten_bits, schuss_zähler)."""
        for i, (t, (keys, fires)) in enumerate(zip(self.tanks, inputs)):
            # Schuss-Zähler: jeder Tastendruck erhöht ihn -> keine verlorenen Taps
            new_shot = t.fire_seen is not None and fires > t.fire_seen
            t.fire_seen = self.fire_seen[i] = fires
            if not t.alive:
                t.dead_age += dt
                continue
            t.cooldown = max(0.0, t.cooldown - dt)
            if keys & LEFT:
                t.a -= TANK_ROT * dt
            if keys & RIGHT:
                t.a += TANK_ROT * dt
            t.a %= 2 * math.pi
            move = (1 if keys & UP else 0) - (0.7 if keys & DOWN else 0)
            if move:
                t.x += math.cos(t.a) * TANK_SPEED * move * dt
                t.y += math.sin(t.a) * TANK_SPEED * move * dt
                t.x, t.y = push_out(t.x, t.y, TANK_R, self.walls)
            if new_shot and t.cooldown == 0 and \
                    sum(1 for b in self.bullets if b.owner == i) < MAX_BULLETS:
                b = Bullet(i, t.x, t.y, t.a)
                b.travel(MUZZLE, self.walls)
                self.bullets.append(b)
                t.cooldown = FIRE_COOLDOWN

        # Panzer gegeneinander
        t1, t2 = self.tanks
        if t1.alive and t2.alive:
            dx, dy = t2.x - t1.x, t2.y - t1.y
            d = math.hypot(dx, dy)
            if 1e-6 < d < 2 * TANK_R:
                p = (2 * TANK_R - d) / 2
                t1.x -= dx / d * p
                t1.y -= dy / d * p
                t2.x += dx / d * p
                t2.y += dy / d * p
                for t in (t1, t2):
                    t.x, t.y = push_out(t.x, t.y, TANK_R, self.walls)

        # Kugeln
        for b in self.bullets[:]:
            b.age += dt
            if b.age > BULLET_LIFE:
                self.bullets.remove(b)
                continue
            b.travel(BULLET_SPEED * dt, self.walls)
            for i, t in enumerate(self.tanks):
                if not t.alive or (i == b.owner and b.age < OWNER_GRACE):
                    continue
                if math.hypot(t.x - b.x, t.y - b.y) < TANK_R + BULLET_R - 1:
                    t.alive = False
                    self.bullets.remove(b)
                    break

        # Rundenende
        alive = [i for i, t in enumerate(self.tanks) if t.alive]
        if self.end_timer is None and len(alive) <= 1:
            self.end_timer = ROUND_END_DELAY
        if self.end_timer is not None:
            self.end_timer -= dt
            if self.end_timer < ROUND_END_DELAY - 1:
                self.msg = f"{NAMES[alive[0]]} gewinnt die Runde!" if alive else "Unentschieden!"
            if self.end_timer <= 0:
                if len(alive) == 1:
                    self.scores[alive[0]] += 1
                self.new_round()

    def snapshot(self):
        return {
            "t": "s",
            "m": self.maze_id,
            "tk": [[round(t.x, 1), round(t.y, 1), round(t.a, 3), t.alive, round(t.dead_age, 2)]
                   for t in self.tanks],
            "b": [[round(b.x, 1), round(b.y, 1)] for b in self.bullets],
            "sc": self.scores,
            "msg": self.msg,
        }


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
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
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


class HostNet:
    def __init__(self, port):
        self.srv = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        self.srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.srv.bind(("", port))
        self.srv.listen(1)
        self.lock = threading.Lock()
        self.conn = None
        self.peer = None
        self.gen = 0  # zählt Verbindungen hoch -> Host erkennt neuen Mitspieler
        self.remote_input = (0, 0)
        threading.Thread(target=self._accept_loop, daemon=True).start()

    def _accept_loop(self):
        while True:
            try:
                conn, addr = self.srv.accept()
            except OSError:
                return
            with self.lock:
                if self.conn is not None:
                    try:
                        send_msg(conn, {"t": "full"})
                    except OSError:
                        pass
                    conn.close()
                    continue
                conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                self.remote_input = (0, 0)
                self.conn, self.peer = conn, addr[0]
                self.gen += 1
            threading.Thread(target=self._recv_loop, args=(conn,), daemon=True).start()

    def _recv_loop(self, conn):
        def on_msg(msg):
            if msg.get("t") == "i":
                self.remote_input = (int(msg["k"]), int(msg["f"]))
        read_lines(conn, on_msg)
        self._drop(conn)

    def _drop(self, conn):
        with self.lock:
            if self.conn is conn:
                self.conn = None
        close_sock(conn)

    def send(self, obj):
        conn = self.conn
        if conn is None:
            return
        try:
            send_msg(conn, obj)
        except OSError:
            self._drop(conn)

    def close(self):
        close_sock(self.srv)
        if self.conn:
            self._drop(self.conn)


class ClientNet:
    def __init__(self, host, port):
        self.sock = socket.create_connection((host, port), timeout=5)
        self.sock.settimeout(None)
        self.sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.maze = None
        self.state = None
        self.connected = True
        self.error = None
        threading.Thread(target=self._recv_loop, daemon=True).start()

    def _recv_loop(self):
        def on_msg(msg):
            t = msg.get("t")
            if t == "m":
                self.maze = msg["maze"]
            elif t == "s":
                self.state = msg
            elif t == "full":
                self.error = "Das Spiel ist schon voll."
        read_lines(self.sock, on_msg)
        self.connected = False

    def send_input(self, keys, fires):
        try:
            send_msg(self.sock, {"t": "i", "k": keys, "f": fires})
        except OSError:
            self.connected = False

    def close(self):
        close_sock(self.sock)


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
    return s


class Renderer:
    def __init__(self, screen):
        self.screen = screen
        self.font = pygame.font.Font(None, 28)
        self.big = pygame.font.Font(None, 48)
        self.score_font = pygame.font.Font(None, 64)
        self.small = pygame.font.Font(None, 22)
        self.sprites = [make_tank_sprite(c) for c in COLORS]
        self.dead_sprite = make_tank_sprite((90, 90, 90))
        self.icons = [pygame.transform.rotozoom(s, 0, 1.6) for s in self.sprites]
        self.maze_surf = None
        self.maze_surf_id = None

    def text(self, txt, pos, font=None, color=TEXT_C, center=True):
        surf = (font or self.font).render(txt, True, color)
        rect = surf.get_rect(center=pos) if center else surf.get_rect(topleft=pos)
        self.screen.blit(surf, rect)
        return rect

    def _maze_surface(self, maze):
        if self.maze_surf_id != maze["id"]:
            surf = pygame.Surface((W, ARENA_TOP + ARENA_H + 10), pygame.SRCALPHA)
            pygame.draw.rect(surf, FLOOR, (maze["ox"], maze["oy"],
                                           maze["cols"] * CELL, maze["rows"] * CELL))
            for w in maze["walls"]:
                pygame.draw.rect(surf, WALL_C, w)
            self.maze_surf, self.maze_surf_id = surf, maze["id"]
        return self.maze_surf

    def draw(self, maze, snap, me=None, hint=""):
        scr = self.screen
        scr.fill(BG)
        if maze:
            scr.blit(self._maze_surface(maze), (0, 0))
        if snap and maze and snap["m"] == maze["id"]:
            for x, y in snap["b"]:
                pygame.draw.circle(scr, (30, 30, 30), (x, y), BULLET_R)
            for i, (x, y, a, alive, dead_age) in enumerate(snap["tk"]):
                sprite = self.sprites[i] if alive else self.dead_sprite
                rot = pygame.transform.rotozoom(sprite, -math.degrees(a), 1)
                scr.blit(rot, rot.get_rect(center=(x, y)))
                if not alive and dead_age < 0.7:
                    k = dead_age / 0.7
                    boom = pygame.Surface((160, 160), pygame.SRCALPHA)
                    pygame.draw.circle(boom, (255, 150, 30, int(220 * (1 - k))), (80, 80),
                                       int(12 + 60 * k))
                    pygame.draw.circle(boom, (255, 230, 90, int(255 * (1 - k))), (80, 80),
                                       int(6 + 30 * k))
                    scr.blit(boom, (x - 80, y - 80))
            if snap.get("msg"):
                r = self.text(snap["msg"], (W // 2, ARENA_TOP + ARENA_H // 2), self.big)
                pad = r.inflate(30, 16)
                box = pygame.Surface(pad.size, pygame.SRCALPHA)
                box.fill((255, 255, 255, 210))
                scr.blit(box, pad)
                self.text(snap["msg"], (W // 2, ARENA_TOP + ARENA_H // 2), self.big)
        if snap:
            self._hud(snap["sc"], me)
        if hint:
            self.text(hint, (W // 2, H - 14), self.small, (140, 140, 140))

    def _hud(self, scores, me):
        y = ARENA_TOP + ARENA_H + 42
        for i, x in enumerate((W // 2 - 220, W // 2 + 220)):
            icon = self.icons[i]
            self.screen.blit(icon, icon.get_rect(center=(x - 40, y)))
            self.text(str(scores[i]), (x + 30, y), self.score_font)
            if me == i:
                self.text("Du", (x + 85, y + 4), self.font, COLORS[i])


# --- Eingabe -----------------------------------------------------------------

class KeyInput:
    """Tastenbelegung -> (bits, schuss_zähler)."""

    def __init__(self, up, down, left, right, fire):
        self.map = [(k, UP) for k in up] + [(k, DOWN) for k in down] + \
                   [(k, LEFT) for k in left] + [(k, RIGHT) for k in right]
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
    return KeyInput((pygame.K_UP, pygame.K_w), (pygame.K_DOWN, pygame.K_s),
                    (pygame.K_LEFT, pygame.K_a), (pygame.K_RIGHT, pygame.K_d),
                    (pygame.K_SPACE, pygame.K_m, pygame.K_q))


NET_HINT = "Pfeiltasten/WASD fahren  ·  Leertaste schießen  ·  F11 Vollbild  ·  Esc Menü"


def common_events(event):
    """True, wenn zurück ins Menü."""
    if event.type == pygame.QUIT:
        pygame.quit()
        sys.exit()
    if event.type == pygame.KEYDOWN:
        if event.key == pygame.K_F11:
            pygame.display.toggle_fullscreen()
        elif event.key == pygame.K_ESCAPE:
            return True
    return False


# --- Modi --------------------------------------------------------------------

def run_local(screen, clock, rend):
    game = Game()
    p_red = KeyInput((pygame.K_e,), (pygame.K_d,), (pygame.K_s,), (pygame.K_f,), (pygame.K_q,))
    p_green = KeyInput((pygame.K_UP,), (pygame.K_DOWN,), (pygame.K_LEFT,), (pygame.K_RIGHT,),
                       (pygame.K_m,))
    hint = "Rot: ESDF + Q   ·   Grün: Pfeiltasten + M   ·   Esc Menü"
    while True:
        dt = min(clock.tick(FPS) / 1000, 0.05)
        for e in pygame.event.get():
            if common_events(e):
                return
            p_red.handle(e)
            p_green.handle(e)
        pressed = pygame.key.get_pressed()
        game.update(dt, [p_red.state(pressed), p_green.state(pressed)])
        rend.draw(game.maze, game.snapshot(), hint=hint)
        pygame.display.flip()


def run_host(screen, clock, rend, port=PORT):
    try:
        net = HostNet(port)
    except OSError as e:
        return message(screen, clock, rend, f"Port {port} geht nicht: {e}")
    ip = local_ip()
    inp = net_input()
    game, gen, sent_maze = None, 0, None
    try:
        while True:
            dt = min(clock.tick(FPS) / 1000, 0.05)
            for e in pygame.event.get():
                if common_events(e):
                    return
                inp.handle(e)

            if net.conn is None:
                game = None
                rend.draw(None, None, hint="Esc Menü")
                rend.text("Warte auf Mitspieler …", (W // 2, 250), rend.big)
                rend.text(f"Deine IP:  {ip}   Port: {port}", (W // 2, 320))
                rend.text("Dein Freund wählt \"Beitreten\" und gibt diese IP ein.",
                          (W // 2, 360), rend.small, (110, 110, 110))
                rend.text("Nicht im selben Netz? Tailscale/ZeroTier-IP oder Port-Forwarding (TCP).",
                          (W // 2, 385), rend.small, (110, 110, 110))
                pygame.display.flip()
                continue

            if game is None or gen != net.gen:  # neuer Mitspieler -> neues Spiel
                game, gen, sent_maze = Game(), net.gen, None

            game.update(dt, [inp.state(pygame.key.get_pressed()), net.remote_input])
            if sent_maze != game.maze_id:
                net.send({"t": "m", "maze": game.maze})
                sent_maze = game.maze_id
            snap = game.snapshot()
            net.send(snap)
            rend.draw(game.maze, snap, me=0, hint=NET_HINT)
            pygame.display.flip()
    finally:
        net.close()


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
    try:
        while True:
            clock.tick(FPS)
            for e in pygame.event.get():
                if common_events(e):
                    return
                inp.handle(e)
            if net.error:
                return message(screen, clock, rend, net.error)
            if not net.connected:
                return message(screen, clock, rend, "Verbindung zum Host getrennt.")
            net.send_input(*inp.state(pygame.key.get_pressed()))
            rend.draw(net.maze, net.state, me=1, hint=NET_HINT)
            if net.state is None:
                rend.text("Warte auf Spielstart …", (W // 2, 300), rend.big)
            pygame.display.flip()
    finally:
        net.close()


def message(screen, clock, rend, txt):
    while True:
        clock.tick(30)
        for e in pygame.event.get():
            common_events(e)
            if e.type in (pygame.KEYDOWN, pygame.MOUSEBUTTONDOWN):
                return
        rend.draw(None, None)
        rend.text(txt, (W // 2, 300))
        rend.text("Beliebige Taste …", (W // 2, 350), rend.small, (130, 130, 130))
        pygame.display.flip()


def menu(screen, clock, rend):
    """Gibt ("host",) / ("join", addr) / ("local",) zurück."""
    try:
        ip_text = LAST_IP_FILE.read_text().strip()
    except OSError:
        ip_text = ""
    buttons = {
        "host": pygame.Rect(W // 2 - 160, 230, 320, 56),
        "join": pygame.Rect(W // 2 - 160, 390, 320, 56),
        "local": pygame.Rect(W // 2 - 160, 490, 320, 56),
    }
    labels = {"host": "Spiel hosten", "join": "Beitreten", "local": "Lokal (1 Tastatur)"}
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
            if e.type == pygame.QUIT or (e.type == pygame.KEYDOWN and e.key == pygame.K_ESCAPE):
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
                ip_text = (ip_text + "".join(ch for ch in e.text if ch.isalnum() or ch in ".:-"))[:60]
            elif e.type == pygame.MOUSEBUTTONDOWN and e.button == 1:
                for key, rect in buttons.items():
                    if rect.collidepoint(e.pos):
                        if key == "join":
                            if join():
                                return join()
                        else:
                            return (key,)

        screen.fill(BG)
        rend.text("PANZER", (W // 2, 110), rend.score_font)
        for i, spr in enumerate(rend.icons):
            screen.blit(spr, spr.get_rect(center=(W // 2 + (-170 if i == 0 else 170), 110)))
        rend.text("Host spielt Rot, wer beitritt Grün", (W // 2, 302), rend.small, (130, 130, 130))
        pygame.draw.rect(screen, (245, 245, 245), ip_box)
        pygame.draw.rect(screen, WALL_C, ip_box, 2)
        if ip_text:
            rend.text(ip_text + ("|" if pygame.time.get_ticks() // 500 % 2 else ""),
                      ip_box.center)
        else:
            rend.text("IP-Adresse des Hosts eingeben", ip_box.center, color=(160, 160, 160))
        for key, rect in buttons.items():
            hover = rect.collidepoint(mouse)
            pygame.draw.rect(screen, (95, 95, 95) if hover else WALL_C, rect, border_radius=6)
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
        action = ("local",)

    while True:
        if action is None:
            action = menu(screen, clock, rend)
        pygame.key.stop_text_input()
        if action[0] == "host":
            run_host(screen, clock, rend, *action[1:])
        elif action[0] == "join":
            run_client(screen, clock, rend, action[1])
        else:
            run_local(screen, clock, rend)
        action = None


if __name__ == "__main__":
    main()
