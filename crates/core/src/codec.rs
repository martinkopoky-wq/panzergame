//! Binäres Kodieren: Varints, Strings, Maze und Snapshots mit Delta gegen eine Basis.
//!
//! Statt ~3–6 KB JSON pro Snapshot (alte Version) gehen typischerweise 40–300 Byte raus:
//! Positionen als u16, Winkel als u16, Alter als feste Geburtszeit (ändert sich nie) und
//! alles, was gleich geblieben ist wie im letzten bestätigten Snapshot, als 1 Bit/Byte.

use crate::consts::{Mutator, MAX_PLAYERS};
use crate::items::{Item, EFF_N};
use crate::maze::Maze;
use crate::snap::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError(pub &'static str);

pub type Res<T> = Result<T, DecodeError>;

#[derive(Default)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Writer { buf: Vec::with_capacity(256) }
    }
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }
    pub fn i8(&mut self, v: i8) {
        self.buf.push(v as u8);
    }
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn i16(&mut self, v: i16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }
    pub fn var(&mut self, mut v: u64) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                self.buf.push(b);
                return;
            }
            self.buf.push(b | 0x80);
        }
    }
    /// Varint mit Vorzeichen (zigzag)
    pub fn svar(&mut self, v: i64) {
        self.var(((v << 1) ^ (v >> 63)) as u64);
    }
    pub fn bytes(&mut self, b: &[u8]) {
        self.var(b.len() as u64);
        self.buf.extend_from_slice(b);
    }
    pub fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }
    pub fn raw(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }
    pub fn left(&self) -> usize {
        self.buf.len() - self.pos
    }
    pub fn rest(&self) -> &'a [u8] {
        &self.buf[self.pos..]
    }
    fn take(&mut self, n: usize) -> Res<&'a [u8]> {
        if self.left() < n {
            return Err(DecodeError("zu kurz"));
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub fn u8(&mut self) -> Res<u8> {
        Ok(self.take(1)?[0])
    }
    pub fn i8(&mut self) -> Res<i8> {
        Ok(self.u8()? as i8)
    }
    pub fn u16(&mut self) -> Res<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn i16(&mut self) -> Res<i16> {
        Ok(self.u16()? as i16)
    }
    pub fn u32(&mut self) -> Res<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn u64(&mut self) -> Res<u64> {
        let b = self.take(8)?;
        let mut a = [0u8; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }
    pub fn var(&mut self) -> Res<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.u8()?;
            v |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(DecodeError("Varint zu lang"))
    }
    pub fn var32(&mut self) -> Res<u32> {
        u32::try_from(self.var()?).map_err(|_| DecodeError("Zahl zu groß"))
    }
    pub fn svar(&mut self) -> Res<i64> {
        let v = self.var()?;
        Ok(((v >> 1) as i64) ^ -((v & 1) as i64))
    }
    /// Länge mit Obergrenze (gegen Speicher-Angriffe mit riesigen Längen)
    pub fn len(&mut self, max: usize) -> Res<usize> {
        let n = self.var()? as usize;
        if n > max || n > self.left() * 8 + 8 {
            return Err(DecodeError("Länge unplausibel"));
        }
        Ok(n)
    }
    pub fn bytes(&mut self, max: usize) -> Res<&'a [u8]> {
        let n = self.len(max)?;
        self.take(n)
    }
    pub fn str(&mut self, max: usize) -> Res<String> {
        let b = self.bytes(max)?;
        String::from_utf8(b.to_vec()).map_err(|_| DecodeError("kein UTF-8"))
    }
}

// ---------------------------------------------------------------------------
// Maze

fn write_bits(w: &mut Writer, grid: &[Vec<bool>]) {
    let mut acc = 0u8;
    let mut n = 0;
    for row in grid {
        for &b in row {
            if b {
                acc |= 1 << n;
            }
            n += 1;
            if n == 8 {
                w.u8(acc);
                acc = 0;
                n = 0;
            }
        }
    }
    if n > 0 {
        w.u8(acc);
    }
}

fn read_bits(r: &mut Reader, rows: usize, cols: usize) -> Res<Vec<Vec<bool>>> {
    let total = rows * cols;
    let bytes = r.take(total.div_ceil(8))?;
    let mut out = vec![vec![false; cols]; rows];
    for i in 0..total {
        out[i / cols][i % cols] = bytes[i / 8] & (1 << (i % 8)) != 0;
    }
    Ok(out)
}

pub fn write_maze(w: &mut Writer, m: &Maze) {
    w.var(m.id as u64);
    w.u16(m.rev);
    w.u8(m.cols as u8);
    w.u8(m.rows as u8);
    w.i16(m.ox as i16);
    w.i16(m.oy as i16);
    w.u16(m.ww as u16);
    w.u16(m.wh as u16);
    w.u8(m.theme);
    w.u8(m.mutator.map_or(255, |x| x as u8));
    write_bits(w, &m.h);
    write_bits(w, &m.v);
    w.var(m.extra.len() as u64);
    for e in &m.extra {
        w.i16(e[0] as i16);
        w.i16(e[1] as i16);
        w.u8(e[2] as u8);
        w.u8(e[3] as u8);
    }
}

pub fn read_maze(r: &mut Reader) -> Res<Maze> {
    let id = r.var32()?;
    let rev = r.u16()?;
    let cols = r.u8()? as usize;
    let rows = r.u8()? as usize;
    if !(2..=40).contains(&cols) || !(2..=30).contains(&rows) {
        return Err(DecodeError("Labyrinthgröße"));
    }
    let ox = r.i16()? as i32;
    let oy = r.i16()? as i32;
    let ww = r.u16()? as i32;
    let wh = r.u16()? as i32;
    let theme = r.u8()?;
    let mutator = Mutator::from_u8(r.u8()?);
    let h = read_bits(r, rows + 1, cols)?;
    let v = read_bits(r, rows, cols + 1)?;
    let n = r.len(64)?;
    let mut extra = Vec::with_capacity(n);
    for _ in 0..n {
        extra.push([r.i16()? as i32, r.i16()? as i32, r.u8()? as i32, r.u8()? as i32]);
    }
    if ww <= 0 || wh <= 0 {
        return Err(DecodeError("Weltgröße"));
    }
    Ok(Maze { id, rev, cols, rows, ox, oy, ww, wh, h, v, extra, mutator, theme })
}

// ---------------------------------------------------------------------------
// Effekte

fn write_pts(w: &mut Writer, pts: &[P16]) {
    w.var(pts.len() as u64);
    let mut last = (0i16, 0i16);
    for &(x, y) in pts {
        w.svar(x as i64 - last.0 as i64);
        w.svar(y as i64 - last.1 as i64);
        last = (x, y);
    }
}

fn read_pts(r: &mut Reader) -> Res<Vec<P16>> {
    let n = r.len(64)?;
    let mut out = Vec::with_capacity(n);
    let mut last = (0i64, 0i64);
    for _ in 0..n {
        last.0 += r.svar()?;
        last.1 += r.svar()?;
        out.push((last.0.clamp(-32768, 32767) as i16, last.1.clamp(-32768, 32767) as i16));
    }
    Ok(out)
}

fn write_ann(w: &mut Writer, a: Ann) {
    match a {
        Ann::Multi(n) => {
            w.u8(0);
            w.u8(n);
        }
        Ann::Trick(n) => {
            w.u8(1);
            w.u8(n);
        }
        Ann::FirstBlood => w.u8(2),
        Ann::Disco => w.u8(3),
        Ann::Meteors => w.u8(4),
    }
}

fn read_ann(r: &mut Reader) -> Res<Ann> {
    Ok(match r.u8()? {
        0 => Ann::Multi(r.u8()?),
        1 => Ann::Trick(r.u8()?),
        2 => Ann::FirstBlood,
        3 => Ann::Disco,
        4 => Ann::Meteors,
        _ => return Err(DecodeError("Ansage")),
    })
}

fn write_fx(w: &mut Writer, d: &FxData) {
    use FxData::*;
    let xy = |w: &mut Writer, x: i16, y: i16| {
        w.i16(x);
        w.i16(y);
    };
    match d {
        Boom { x, y, size } => {
            w.u8(0);
            xy(w, *x, *y);
            w.u8(*size);
        }
        Die { x, y, slot, angle } => {
            w.u8(1);
            xy(w, *x, *y);
            w.u8(*slot);
            w.i16(*angle);
        }
        Laser { slot, pts } => {
            w.u8(2);
            w.u8(*slot);
            write_pts(w, pts);
        }
        Zap { slot, pts } => {
            w.u8(3);
            w.u8(*slot);
            write_pts(w, pts);
        }
        Tele { x, y } => {
            w.u8(4);
            xy(w, *x, *y);
        }
        Pick { x, y, item, mystery } => {
            w.u8(5);
            xy(w, *x, *y);
            w.u8(*item as u8);
            w.u8(*mystery as u8);
        }
        Block { x, y } => {
            w.u8(6);
            xy(w, *x, *y);
        }
        Freeze { x, y } => {
            w.u8(7);
            xy(w, *x, *y);
        }
        Nuke { x, y } => {
            w.u8(8);
            xy(w, *x, *y);
        }
        Ann { ann, slot } => {
            w.u8(9);
            write_ann(w, *ann);
            w.u8(*slot);
        }
        Pop { x, y, text, slot } => {
            w.u8(10);
            xy(w, *x, *y);
            w.u8(*text as u8);
            w.u8(*slot);
        }
        Rail { slot, p0, p1, holes } => {
            w.u8(11);
            w.u8(*slot);
            xy(w, p0.0, p0.1);
            xy(w, p1.0, p1.1);
            write_pts(w, holes);
        }
        Prism { slot, beams } => {
            w.u8(12);
            w.u8(*slot);
            w.var(beams.len() as u64);
            for b in beams {
                write_pts(w, b);
            }
        }
        Push { slot, x, y, a } => {
            w.u8(13);
            w.u8(*slot);
            xy(w, *x, *y);
            w.i16(*a);
        }
        Frost { x, y } => {
            w.u8(14);
            xy(w, *x, *y);
        }
        Jump { x0, y0, x1, y1 } => {
            w.u8(15);
            xy(w, *x0, *y0);
            xy(w, *x1, *y1);
        }
        Disco { x, y } => {
            w.u8(16);
            xy(w, *x, *y);
        }
        Mirror { x, y } => {
            w.u8(17);
            xy(w, *x, *y);
        }
        Build { x, y } => {
            w.u8(18);
            xy(w, *x, *y);
        }
        Trap { x, y } => {
            w.u8(19);
            xy(w, *x, *y);
        }
        Fw { x, y } => {
            w.u8(20);
            xy(w, *x, *y);
        }
    }
}

fn read_fx(r: &mut Reader) -> Res<FxData> {
    use FxData::*;
    let tag = r.u8()?;
    let xy = |r: &mut Reader| -> Res<(i16, i16)> { Ok((r.i16()?, r.i16()?)) };
    let slot = |r: &mut Reader| -> Res<u8> {
        let s = r.u8()?;
        if s as usize >= MAX_PLAYERS {
            return Err(DecodeError("Slot"));
        }
        Ok(s)
    };
    Ok(match tag {
        0 => {
            let (x, y) = xy(r)?;
            Boom { x, y, size: r.u8()? }
        }
        1 => {
            let (x, y) = xy(r)?;
            Die { x, y, slot: slot(r)?, angle: r.i16()? }
        }
        2 => Laser { slot: slot(r)?, pts: read_pts(r)? },
        3 => Zap { slot: slot(r)?, pts: read_pts(r)? },
        4 => {
            let (x, y) = xy(r)?;
            Tele { x, y }
        }
        5 => {
            let (x, y) = xy(r)?;
            let item = Item::from_u8(r.u8()?).ok_or(DecodeError("Item"))?;
            Pick { x, y, item, mystery: r.u8()? != 0 }
        }
        6 => {
            let (x, y) = xy(r)?;
            Block { x, y }
        }
        7 => {
            let (x, y) = xy(r)?;
            Freeze { x, y }
        }
        8 => {
            let (x, y) = xy(r)?;
            Nuke { x, y }
        }
        9 => {
            let ann = read_ann(r)?;
            Ann { ann, slot: slot(r)? }
        }
        10 => {
            let (x, y) = xy(r)?;
            let text = if r.u8()? == 0 { PopText::OwnGoal } else { PopText::Spare };
            Pop { x, y, text, slot: slot(r)? }
        }
        11 => {
            let s = slot(r)?;
            let p0 = xy(r)?;
            let p1 = xy(r)?;
            Rail { slot: s, p0, p1, holes: read_pts(r)? }
        }
        12 => {
            let s = slot(r)?;
            let n = r.len(8)?;
            let mut beams = Vec::with_capacity(n);
            for _ in 0..n {
                beams.push(read_pts(r)?);
            }
            Prism { slot: s, beams }
        }
        13 => {
            let s = slot(r)?;
            let (x, y) = xy(r)?;
            Push { slot: s, x, y, a: r.i16()? }
        }
        14 => {
            let (x, y) = xy(r)?;
            Frost { x, y }
        }
        15 => {
            let (x0, y0) = xy(r)?;
            let (x1, y1) = xy(r)?;
            Jump { x0, y0, x1, y1 }
        }
        16 => {
            let (x, y) = xy(r)?;
            Disco { x, y }
        }
        17 => {
            let (x, y) = xy(r)?;
            Mirror { x, y }
        }
        18 => {
            let (x, y) = xy(r)?;
            Build { x, y }
        }
        19 => {
            let (x, y) = xy(r)?;
            Trap { x, y }
        }
        20 => {
            let (x, y) = xy(r)?;
            Fw { x, y }
        }
        _ => return Err(DecodeError("Effekt")),
    })
}

// ---------------------------------------------------------------------------
// Snapshot mit Delta

const F_SCORES: u8 = 1;
const F_FEED: u8 = 2;
const F_BOXES: u8 = 4;
const F_WIN: u8 = 8;
const F_BOTS: u8 = 16;

// Felder eines Panzers (Bits in der Änderungsmaske)
const TK_FLAGS: u16 = 1;
const TK_POS: u16 = 2;
const TK_ANG: u16 = 4;
const TK_DIED: u16 = 8;
const TK_WEAPON: u16 = 16;
const TK_SHIELD: u16 = 32;
const TK_AMMO: u16 = 64;
const TK_RELOAD: u16 = 128;
const TK_EFF: u16 = 256;
const TK_CHARGE: u16 = 512;
const TK_TAUNT: u16 = 1024;

// Geschoss-Flags
const SH_NEW: u8 = 0x80;
const SH_POS: u8 = 1;
const SH_POS_SMALL: u8 = 2;
const SH_ANG: u8 = 4;
const SH_BORN: u8 = 8;
const SH_LIFE: u8 = 16;
const SH_OWNER: u8 = 32;

fn write_tank(w: &mut Writer, t: &NetTank, base: Option<&NetTank>) {
    // Maske direkt vergleichen: vorher 11 &dyn-Fn-Closures pro Panzer und Paket
    let m: u16 = match base {
        None => {
            TK_FLAGS | TK_POS | TK_ANG | TK_DIED | TK_WEAPON | TK_SHIELD | TK_AMMO | TK_RELOAD
                | TK_EFF | TK_CHARGE | TK_TAUNT
        }
        Some(b) => {
            let mut m = 0u16;
            if b.flags != t.flags {
                m |= TK_FLAGS;
            }
            if b.x != t.x || b.y != t.y {
                m |= TK_POS;
            }
            if b.a != t.a {
                m |= TK_ANG;
            }
            if b.died != t.died {
                m |= TK_DIED;
            }
            if b.weapon != t.weapon {
                m |= TK_WEAPON;
            }
            if b.shield != t.shield {
                m |= TK_SHIELD;
            }
            if b.ammo != t.ammo {
                m |= TK_AMMO;
            }
            if b.reload != t.reload {
                m |= TK_RELOAD;
            }
            if b.eff_mask != t.eff_mask || b.eff_exp != t.eff_exp {
                m |= TK_EFF;
            }
            if b.charge != t.charge {
                m |= TK_CHARGE;
            }
            if b.taunt != t.taunt {
                m |= TK_TAUNT;
            }
            m
        }
    };
    w.u8(t.slot);
    w.var(m as u64);
    if m & TK_FLAGS != 0 {
        w.u8(t.flags);
    }
    if m & TK_POS != 0 {
        match base {
            Some(b) if (t.x as i32 - b.x as i32).abs() < 64 && (t.y as i32 - b.y as i32).abs() < 64 => {
                // kleine Bewegung: 2 Byte statt 4 (Bit 7 markiert die Kurzform)
                w.u8(0x80 | ((t.x as i32 - b.x as i32 + 64) as u8 & 0x7f));
                w.u8((t.y as i32 - b.y as i32 + 64) as u8);
            }
            _ => {
                w.u8(0);
                w.u16(t.x);
                w.u16(t.y);
            }
        }
    }
    if m & TK_ANG != 0 {
        w.u16(t.a);
    }
    if m & TK_DIED != 0 {
        w.u32(t.died);
    }
    if m & TK_WEAPON != 0 {
        w.u8(t.weapon);
    }
    if m & TK_SHIELD != 0 {
        w.var(t.shield as u64);
    }
    if m & TK_AMMO != 0 {
        w.u8(t.ammo);
    }
    if m & TK_RELOAD != 0 {
        w.u8(t.reload);
    }
    if m & TK_EFF != 0 {
        w.var(t.eff_mask as u64);
        for &e in &t.eff_exp {
            w.var(e as u64);
        }
    }
    if m & TK_CHARGE != 0 {
        w.u8(t.charge);
    }
    if m & TK_TAUNT != 0 {
        w.u8(t.taunt);
    }
}

fn read_tank(r: &mut Reader, base: &NetSnap) -> Res<NetTank> {
    let slot = r.u8()?;
    if slot as usize >= MAX_PLAYERS {
        return Err(DecodeError("Slot"));
    }
    let m = r.var()? as u16;
    // Basisfelder kopieren statt den ganzen Panzer zu klonen: ohne TK_EFF wird die
    // Effektliste ohnehin übernommen, mit TK_EFF würde die geklonte Liste verworfen
    let mut t = NetTank::default();
    if let Some(b) = base.tanks.iter().find(|b| b.slot == slot) {
        t.flags = b.flags;
        t.x = b.x;
        t.y = b.y;
        t.a = b.a;
        t.died = b.died;
        t.weapon = b.weapon;
        t.shield = b.shield;
        t.ammo = b.ammo;
        t.reload = b.reload;
        t.eff_mask = b.eff_mask;
        if m & TK_EFF == 0 {
            t.eff_exp.extend_from_slice(&b.eff_exp);
        }
        t.charge = b.charge;
        t.taunt = b.taunt;
    }
    t.slot = slot;
    if m & TK_FLAGS != 0 {
        t.flags = r.u8()?;
    }
    if m & TK_POS != 0 {
        let first = r.u8()?;
        if first & 0x80 != 0 {
            let dx = (first & 0x7f) as i32 - 64;
            let dy = r.u8()? as i32 - 64;
            t.x = (t.x as i32 + dx).clamp(0, 65535) as u16;
            t.y = (t.y as i32 + dy).clamp(0, 65535) as u16;
        } else {
            t.x = r.u16()?;
            t.y = r.u16()?;
        }
    }
    if m & TK_ANG != 0 {
        t.a = r.u16()?;
    }
    if m & TK_DIED != 0 {
        t.died = r.u32()?;
    }
    if m & TK_WEAPON != 0 {
        t.weapon = r.u8()?;
    }
    if m & TK_SHIELD != 0 {
        t.shield = r.var32()?;
    }
    if m & TK_AMMO != 0 {
        t.ammo = r.u8()?;
    }
    if m & TK_RELOAD != 0 {
        t.reload = r.u8()?;
    }
    if m & TK_EFF != 0 {
        t.eff_mask = r.var32()? & ((1u32 << EFF_N) - 1);
        t.eff_exp.clear();
        for _ in 0..t.eff_mask.count_ones() {
            t.eff_exp.push(r.var32()?);
        }
    }
    if m & TK_CHARGE != 0 {
        t.charge = r.u8()?;
    }
    if m & TK_TAUNT != 0 {
        t.taunt = r.u8()?;
    }
    Ok(t)
}

/// Snapshot kodieren. base = der letzte Snapshot, den der Empfänger bestätigt hat.
pub fn write_snap(w: &mut Writer, s: &NetSnap, base: Option<&NetSnap>) {
    w.u32(s.time);
    w.u32(s.gtime);
    w.var(s.ack as u64);
    w.var(s.maze_id as u64);
    w.u16(s.maze_rev);
    w.u8(s.ts);
    w.u16(s.et);
    w.u8(s.queued as u8);
    let mut flags = 0;
    if base.is_none_or(|b| b.scores != s.scores) {
        flags |= F_SCORES;
    }
    if base.is_none_or(|b| b.feed != s.feed) {
        flags |= F_FEED;
    }
    if base.is_none_or(|b| b.boxes != s.boxes) {
        flags |= F_BOXES;
    }
    if base.is_none_or(|b| b.win != s.win) {
        flags |= F_WIN;
    }
    if base.is_none_or(|b| b.bots != s.bots) {
        flags |= F_BOTS;
    }
    w.u8(flags);
    if flags & F_SCORES != 0 {
        for &sc in &s.scores {
            w.var(sc as u64);
        }
    }
    if flags & F_BOTS != 0 {
        w.u8(s.bots);
    }
    if flags & F_WIN != 0 {
        match &s.win {
            Some(win) => {
                w.u8(1);
                w.u8(win.slot);
                w.u8(win.streak);
            }
            None => w.u8(0),
        }
    }
    if flags & F_FEED != 0 {
        w.u8(s.feed.len() as u8);
        for f in &s.feed {
            w.u8(f.by);
            w.u8(f.victim);
            w.u8(f.how);
            w.u8(f.variant);
            w.u32(f.born);
        }
    }
    if flags & F_BOXES != 0 {
        w.var(s.boxes.len() as u64);
        for b in &s.boxes {
            w.u16(b.x);
            w.u16(b.y);
            w.u8(b.item);
        }
    }
    // Panzer
    w.u8(s.tanks.len() as u8);
    for t in &s.tanks {
        write_tank(w, t, base.and_then(|b| b.tanks.iter().find(|bt| bt.slot == t.slot)));
    }
    // Geschosse: nach id sortiert, id als Abstand zur vorigen
    w.var(s.shots.len() as u64);
    let mut last_id = 0u32;
    for sh in &s.shots {
        w.var(sh.id.wrapping_sub(last_id) as u64);
        last_id = sh.id;
        let old = base.and_then(|b| b.shots.binary_search_by_key(&sh.id, |x| x.id).ok().map(|i| &b.shots[i]));
        let Some(old) = old.filter(|o| o.kind == sh.kind) else {
            w.u8(SH_NEW);
            w.u8(sh.kind);
            w.u8(sh.owner);
            w.u16(sh.x);
            w.u16(sh.y);
            w.u16(sh.ang);
            w.u32(sh.born);
            w.u32(sh.life);
            continue;
        };
        let mut f = 0u8;
        let (dx, dy) = (sh.x as i32 - old.x as i32, sh.y as i32 - old.y as i32);
        if dx != 0 || dy != 0 {
            f |= if dx.abs() < 128 && dy.abs() < 128 { SH_POS_SMALL } else { SH_POS };
        }
        if sh.ang != old.ang {
            f |= SH_ANG;
        }
        if sh.born != old.born {
            f |= SH_BORN;
        }
        if sh.life != old.life {
            f |= SH_LIFE;
        }
        if sh.owner != old.owner {
            f |= SH_OWNER;
        }
        w.u8(f);
        if f & SH_POS_SMALL != 0 {
            w.i8(dx as i8);
            w.i8(dy as i8);
        }
        if f & SH_POS != 0 {
            w.u16(sh.x);
            w.u16(sh.y);
        }
        if f & SH_ANG != 0 {
            w.u16(sh.ang);
        }
        if f & SH_BORN != 0 {
            w.u32(sh.born);
        }
        if f & SH_LIFE != 0 {
            w.u32(sh.life);
        }
        if f & SH_OWNER != 0 {
            w.u8(sh.owner);
        }
    }
    // Effekte: Daten nur beim ersten Mal, danach reicht die id
    w.var(s.fx.len() as u64);
    let mut last_id = 0u32;
    for f in &s.fx {
        w.var(f.id.wrapping_sub(last_id) as u64);
        last_id = f.id;
        let known = base.is_some_and(|b| b.fx.binary_search_by_key(&f.id, |x| x.id).is_ok_and(|i| b.fx[i] == *f));
        if known {
            w.u8(0);
        } else {
            w.u8(1);
            w.u32(f.born);
            w.u16(f.life);
            write_fx(w, &f.data);
        }
    }
}

pub fn read_snap(r: &mut Reader, base: Option<&NetSnap>) -> Res<NetSnap> {
    let empty = NetSnap::default();
    let b = base.unwrap_or(&empty);
    let mut s = NetSnap {
        time: r.u32()?,
        gtime: r.u32()?,
        ack: r.var32()?,
        maze_id: r.var32()?,
        maze_rev: r.u16()?,
        ts: r.u8()?,
        et: r.u16()?,
        queued: r.u8()? != 0,
        win: b.win.clone(),
        scores: b.scores,
        bots: b.bots,
        tanks: Vec::new(),
        shots: Vec::new(),
        boxes: Vec::new(),
        fx: Vec::new(),
        feed: Vec::new(),
    };
    let flags = r.u8()?;
    if base.is_none() && flags & (F_SCORES | F_FEED | F_BOXES | F_WIN | F_BOTS) != 0x1f {
        return Err(DecodeError("Vollbild ohne alle Felder"));
    }
    if flags & F_SCORES != 0 {
        for sc in s.scores.iter_mut() {
            *sc = r.var()?.min(u16::MAX as u64) as u16;
        }
    }
    if flags & F_BOTS != 0 {
        s.bots = r.u8()?;
    }
    if flags & F_WIN != 0 {
        s.win = if r.u8()? != 0 { Some(NetWin { slot: r.u8()?, streak: r.u8()? }) } else { None };
    }
    if flags & F_FEED != 0 {
        let n = r.u8()? as usize;
        if n > 16 {
            return Err(DecodeError("Killfeed"));
        }
        for _ in 0..n {
            s.feed.push(NetFeed { by: r.u8()?, victim: r.u8()?, how: r.u8()?, variant: r.u8()?, born: r.u32()? });
        }
    } else {
        s.feed = b.feed.clone();
    }
    if flags & F_BOXES != 0 {
        let n = r.len(256)?;
        for _ in 0..n {
            s.boxes.push(NetBox { x: r.u16()?, y: r.u16()?, item: r.u8()? });
        }
    } else {
        s.boxes = b.boxes.clone();
    }
    let n = r.u8()? as usize;
    if n > MAX_PLAYERS {
        return Err(DecodeError("zu viele Panzer"));
    }
    for _ in 0..n {
        s.tanks.push(read_tank(r, b)?);
    }
    let n = r.len(4096)?;
    let mut id = 0u32;
    for _ in 0..n {
        id = id.wrapping_add(r.var32()?);
        let f = r.u8()?;
        if f & SH_NEW != 0 {
            s.shots.push(NetShot {
                id,
                kind: r.u8()?,
                owner: r.u8()?,
                x: r.u16()?,
                y: r.u16()?,
                ang: r.u16()?,
                born: r.u32()?,
                life: r.u32()?,
            });
            continue;
        }
        let old = b
            .shots
            .binary_search_by_key(&id, |x| x.id)
            .ok()
            .map(|i| b.shots[i].clone())
            .ok_or(DecodeError("Geschoss fehlt in der Basis"))?;
        let mut sh = old;
        if f & SH_POS_SMALL != 0 {
            let dx = r.i8()? as i32;
            let dy = r.i8()? as i32;
            sh.x = (sh.x as i32 + dx).clamp(0, 65535) as u16;
            sh.y = (sh.y as i32 + dy).clamp(0, 65535) as u16;
        }
        if f & SH_POS != 0 {
            sh.x = r.u16()?;
            sh.y = r.u16()?;
        }
        if f & SH_ANG != 0 {
            sh.ang = r.u16()?;
        }
        if f & SH_BORN != 0 {
            sh.born = r.u32()?;
        }
        if f & SH_LIFE != 0 {
            sh.life = r.u32()?;
        }
        if f & SH_OWNER != 0 {
            sh.owner = r.u8()?;
        }
        s.shots.push(sh);
    }
    let n = r.len(1024)?;
    let mut id = 0u32;
    for _ in 0..n {
        id = id.wrapping_add(r.var32()?);
        if r.u8()? == 0 {
            let old =
                b.fx.binary_search_by_key(&id, |x| x.id)
                    .ok()
                    .map(|i| b.fx[i].clone())
                    .ok_or(DecodeError("Effekt fehlt in der Basis"))?;
            s.fx.push(old);
        } else {
            let born = r.u32()?;
            let life = r.u16()?;
            s.fx.push(NetFx { id, born, life, data: read_fx(r)? });
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    #[test]
    fn snapshot_roundtrip_with_delta() {
        let mut g = Game::new(&[0, 1, 2]);
        let mut bots: Vec<crate::bot::Bot> = (0..3).map(|s| crate::bot::Bot::new(s, Some(0.8))).collect();
        let mut prev: Option<NetSnap> = None;
        let mut total = 0;
        for tick in 0..3000 {
            for b in bots.iter_mut() {
                let (k, f) = b.think(&g);
                g.apply_input(b.slot, k, f, crate::consts::TICK);
            }
            g.step(crate::consts::TICK);
            let s = g.net_snap(Some(0), true);
            let mut w = Writer::new();
            write_snap(&mut w, &s, prev.as_ref());
            total += w.len();
            let back = read_snap(&mut Reader::new(&w.buf), prev.as_ref()).expect("decode");
            assert_eq!(back, s, "tick {tick}");
            prev = Some(s);
        }
        // im Schnitt deutlich unter 200 Byte pro Snapshot
        assert!(total / 3000 < 200, "avg {}", total / 3000);
    }

    #[test]
    fn maze_roundtrip() {
        let mut rng = crate::rng::Rng::new(5);
        let m = Maze::generate(3, 8, &mut rng);
        let mut w = Writer::new();
        write_maze(&mut w, &m);
        let back = read_maze(&mut Reader::new(&w.buf)).unwrap();
        assert_eq!(back, m);
    }
}
