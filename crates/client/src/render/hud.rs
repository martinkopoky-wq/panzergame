//! Bildschirm: Blitz, Zeitlupe, Banner, Killfeed, Spielerkarten, Übergang.

use std::f32::consts::{FRAC_PI_2, TAU};

use macroquad::prelude::*;
use panzer_core::consts::*;
use panzer_core::items::Eff;
use panzer_core::physics::bullet_params;
use panzer_core::snap::{active_effects, FxData, TankView, View};
use panzer_core::text::{ann_text, kill_text, streak_text, win_text};
use panzer_core::Arena;

use super::{leader, Renderer, Roster};
use crate::gfx::*;
use crate::raster::{mix, shade};
use crate::sprites::box_color;

impl Renderer {
    /// Bildschirm-Effekte: Blitz, Zeitlupen-Balken, Gefahr am Rand
    pub(super) fn overlays(&mut self, g: &mut Gfx, view: Option<&View>, me: Option<usize>, now: f64, dt: f32) {
        let k = self.cine;
        if k > 0.005 {
            // Zeitlupe: kühler Farbton und Kinobalken
            gl_use_material(&g.mul);
            draw_rectangle(0.0, 0.0, VW, VIEW, col(mix([255, 255, 255], [205, 220, 255], k)));
            gl_use_default_material();
            let bar = (36.0 * k).round();
            draw_rectangle(0.0, 0.0, VW, bar, BLACK);
            draw_rectangle(0.0, VIEW - bar, VW, bar, BLACK);
        }
        if self.flash > 0.01 {
            gl_use_material(&g.add);
            draw_rectangle(
                0.0,
                0.0,
                VW,
                VIEW,
                Color::new(self.flash.min(1.0), self.flash.min(1.0), self.flash.min(1.0), 1.0),
            );
            gl_use_default_material();
        }
        self.flash *= (-7.0 * dt).exp();
        let Some(mine) = view.and_then(|v| me.and_then(|m| v.tank(m))).filter(|t| t.alive) else { return };
        let eff = &mine.eff;
        let danger = if eff.has(Eff::Potato) && eff.get(Eff::Potato) < 3.0 {
            Some(([255, 40, 20], 0.5 + 0.5 * (now as f32 * 14.0).sin()))
        } else if eff.has(Eff::Frozen) {
            Some(([120, 200, 255], 0.6))
        } else {
            None
        };
        if let Some((c, kk)) = danger {
            gl_use_material(&g.add);
            draw_texture_ex(
                &g.spr.edge,
                0.0,
                0.0,
                colk(c, kk),
                DrawTextureParams { dest_size: Some(vec2(VW, VIEW)), ..Default::default() },
            );
            gl_use_default_material();
        }
    }

    fn banner(
        &self,
        g: &mut Gfx,
        text: &str,
        size: f32,
        c: [u8; 3],
        cx: f32,
        cy: f32,
        scale: f32,
        alpha: f32,
        glow: bool,
    ) {
        if glow && alpha > 0.05 {
            g.text_glow(text, cx, cy, size * scale.max(0.05), c, 8.0 * alpha, true);
            g.flush_light();
        }
        g.fancy(text, cx, cy, size, c, 3.0, true, alpha, scale.max(0.05));
    }

    pub(super) fn draw_banners(&mut self, g: &mut Gfx, view: &View, arena: &Arena, roster: &Roster, now: f64) {
        let (cx, cy) = (VW / 2.0, (ARENA_TOP + ARENA_H / 2.0) as f32);
        let t = (now - self.round_start) as f32;
        let th = arena.theme();
        let name = |s: usize| roster.name(s);
        if let Some(m) = arena.mutator {
            g.fancy(
                &format!("Sonderregel: {}", m.name()),
                VW / 2.0,
                9.0,
                18.0,
                mix(th.accent, [255, 255, 255], 0.3),
                2.0,
                false,
                1.0,
                1.0,
            );
        }
        if t < 2.0 && view.win.is_none() {
            // Rundenstart
            let k_in = ease_back(t / 0.35);
            let fade = ((2.0 - t) / 0.5).clamp(0.0, 1.0);
            self.banner(
                g,
                &format!("RUNDE {}", view.maze_id),
                64.0,
                [255, 255, 255],
                cx,
                cy - 40.0,
                0.6 + 0.4 * k_in,
                fade,
                true,
            );
            g.fancy(&th.name.to_uppercase(), cx, cy + 2.0, 22.0, th.accent, 2.0, true, fade, 1.0);
            if let Some(m) = arena.mutator {
                self.banner(
                    g,
                    &format!("{}!", m.name().to_uppercase()),
                    44.0,
                    [200, 120, 255],
                    cx,
                    cy + 48.0,
                    0.6 + 0.4 * ease_back((t - 0.25) / 0.35),
                    fade,
                    true,
                );
                g.fancy(m.desc(), cx, cy + 84.0, 20.0, [235, 235, 245], 2.0, false, fade, 1.0);
            }
        }
        if view.win.is_none() {
            if let Some((fx, ann, slot)) = view.fx.iter().rev().find_map(|f| match &f.data {
                FxData::Ann { ann, slot } => Some((f, *ann, *slot as usize)),
                _ => None,
            }) {
                let (text, sub) = ann_text(ann, slot, &name);
                let age = fx.age as f32;
                let z = ease_back(age / 0.28);
                let fade = ((fx.life as f32 - age) / 0.4).clamp(0.0, 1.0);
                let c = mix(COLORS[slot], [255, 220, 120], 0.35);
                let wob = (age * 30.0).sin() * 3.0 * (1.0 - age / 0.4).max(0.0);
                self.banner(g, &text, 58.0, c, cx + wob, 130.0, z, fade, true);
                if !sub.is_empty() {
                    g.fancy(&sub, cx, 174.0, 22.0, [240, 240, 248], 2.0, false, fade, 1.0);
                }
            }
        }
        if let Some(win) = &view.win {
            let et = view.et.unwrap_or(0.0) as f32;
            let k = ease_out((ROUND_END_DELAY as f32 - 1.0 - et) / 0.3);
            draw_rectangle(
                0.0,
                cy - 60.0,
                VW,
                120.0,
                Color::new(6.0 / 255.0, 7.0 / 255.0, 11.0 / 255.0, 185.0 / 255.0 * k),
            );
            let c = win.slot.map_or([200, 205, 215], |s| COLORS[s]);
            for yy in [cy - 60.0, cy + 60.0] {
                draw_line(cx - VW * k / 2.0, yy, cx + VW * k / 2.0, yy, 2.0, col(c));
            }
            let sub = streak_text(win.streak);
            let text = win_text(win.slot, &name).to_uppercase();
            let scale = 0.5 + 0.5 * ease_back((ROUND_END_DELAY as f32 - 1.0 - et) / 0.35);
            self.banner(
                g,
                &text,
                56.0,
                mix(c, [255, 255, 255], 0.25),
                cx,
                cy - if sub.is_empty() { 0.0 } else { 12.0 },
                scale,
                1.0,
                true,
            );
            if !sub.is_empty() {
                g.fancy(&sub, cx, cy + 34.0, 26.0, ACCENT_C, 2.0, true, 1.0, 1.0);
            }
        }
    }

    pub(super) fn draw_feed(&self, g: &mut Gfx, view: &View, roster: &Roster) {
        let mut y = 26.0;
        let name = |s: usize| roster.name(s);
        for f in &view.feed {
            let text = kill_text(f.by, f.victim, f.how, f.variant, &name);
            let slot = f.by.unwrap_or(f.victim).min(MAX_PLAYERS - 1);
            let age = f.age as f32;
            let slide = 1.0 - ease_out(age / 0.25);
            let alpha = ((FEED_TIME as f32 - age) / 0.8).clamp(0.0, 1.0);
            let tw = g.measure(&text, 15.0, false).width;
            let (w, h) = (tw + 46.0, 24.0);
            let x = VW - 10.0 - w + 90.0 * slide;
            draw_rectangle(x, y, w, h, Color::new(12.0 / 255.0, 13.0 / 255.0, 18.0 / 255.0, 205.0 / 255.0 * alpha));
            draw_rectangle(x, y, 4.0, h, cola(COLORS[slot], alpha));
            match f.how.icon() {
                Some(item) => {
                    draw_circle(x + 20.0, y + h / 2.0, 9.0, cola([226, 231, 240], alpha));
                    g.sprite(&g.spr.icons[&item], x + 20.0, y + h / 2.0, 0.0, 0.6, Color::new(1.0, 1.0, 1.0, alpha));
                }
                None => draw_circle(x + 20.0, y + h / 2.0, 3.0, cola([226, 231, 240], alpha)),
            }
            // Namen in ihrer Farbe
            let mut parts: Vec<(String, [u8; 3])> = vec![(text.clone(), [215, 220, 230])];
            let mut names: Vec<(String, [u8; 3])> = vec![(roster.name(slot), mix(COLORS[slot], [255, 255, 255], 0.3))];
            if f.victim != slot {
                names.push((roster.name(f.victim), mix(COLORS[f.victim.min(7)], [255, 255, 255], 0.3)));
            }
            for (n, c) in names {
                if n.is_empty() {
                    continue;
                }
                let mut out = Vec::new();
                for (piece, pc) in parts {
                    if pc != [215, 220, 230] {
                        out.push((piece, pc));
                        continue;
                    }
                    for (j, bit) in piece.split(n.as_str()).enumerate() {
                        if j > 0 {
                            out.push((n.clone(), c));
                        }
                        if !bit.is_empty() {
                            out.push((bit.to_string(), [215, 220, 230]));
                        }
                    }
                }
                parts = out;
            }
            let mut tx = x + 36.0;
            for (piece, c) in parts {
                tx += g.text(&piece, tx, y + h / 2.0 + 1.0, 15.0, cola(c, alpha), 0.0);
            }
            y += 28.0;
        }
    }

    /// Jalousie: schließt sich kurz vor der neuen Runde und öffnet sich danach
    pub(super) fn draw_transition(&self, view: Option<&View>, arena: &Arena, now: f64) {
        let mut cover = (1.0 - (now - self.round_start) as f32 / 0.5).max(0.0);
        if let Some(et) = view.and_then(|v| v.et) {
            if et < 0.3 {
                cover = cover.max(1.0 - et as f32 / 0.3);
            }
        }
        if cover <= 0.0 {
            return;
        }
        let n = 12;
        let sw = VW / n as f32;
        let accent = arena.theme().accent;
        for j in 0..n {
            let c = (cover * 1.6 - j as f32 * 0.05).clamp(0.0, 1.0);
            if c <= 0.0 {
                continue;
            }
            let w = sw * c;
            let x = j as f32 * sw + (sw - w) / 2.0;
            draw_rectangle(x, 0.0, w + 1.0, VIEW, col([9, 10, 14]));
            if c < 1.0 {
                draw_line(x, 0.0, x, VIEW, 1.0, col(accent));
                draw_line(x + w, 0.0, x + w, VIEW, 1.0, col(accent));
            }
        }
    }

    pub(super) fn hud(
        &mut self,
        g: &mut Gfx,
        view: &View,
        me: Option<usize>,
        arena: &Arena,
        roster: &Roster,
        now: f64,
    ) {
        // Hintergrund mit Verlauf
        let top = VIEW;
        let hh = VH - VIEW;
        for i in 0..8 {
            let k = i as f32 / 8.0;
            draw_rectangle(0.0, top + hh * k, VW, hh / 8.0 + 1.0, col(mix([22, 24, 32], [10, 11, 15], k)));
        }
        draw_line(0.0, top, VW, top, 1.0, col([56, 62, 80]));
        let n = view.tanks.len().max(1);
        let bp = bullet_params(arena.mutator);
        let lead = leader(view);
        let gap = if n <= 4 { 8.0 } else { 5.0 };
        let cw = (VW - 20.0 - gap * (n as f32 - 1.0)) / n as f32;
        let (ctop, ch) = (VIEW + 7.0, VH - VIEW - 28.0);
        for (k, tank) in view.tanks.iter().enumerate() {
            let rect = Rect::new((10.0 + k as f32 * (cw + gap)).round(), ctop, cw.round(), ch);
            self.card(g, rect, tank, view, me, lead, roster, bp.most, bp.life as f32, now);
            if let Some(w) = tank.weapon {
                self.weapon_help.insert(tank.slot, (w, now));
            }
        }
        // Die Hilfe bleibt auch nach dem letzten Schuss sichtbar, solange die Waffe wirkt.
        if let Some(tk) = me.or(if self.quiet { Some(0) } else { None }).and_then(|slot| view.tank(slot)) {
            if tk.alive {
                let active = tk
                    .weapon
                    .or(if tk.eff.has(Eff::Ram) { Some(panzer_core::items::Item::Ram) } else { None })
                    .or_else(|| view.shots.iter().filter(|p| p.owner == tk.slot).find_map(|p| p.kind.icon()));
                let weapon =
                    active.or_else(|| self.weapon_help.get(&tk.slot).filter(|(_, at)| now - at < 4.0).map(|(w, _)| *w));
                if let Some(w) = weapon.filter(|w| !w.hint().is_empty()) {
                    let recent = if active.is_none() { "Zuletzt: " } else { "" };
                    let text = format!("{recent}{}  ·  {}", w.name(), w.hint());
                    let size = (14.0 * (VW - 60.0) / g.measure(&text, 14.0, false).width.max(1.0)).min(14.0);
                    let width = g.measure(&text, size, false).width + 28.0;
                    let x = (VW - width) / 2.0;
                    draw_rectangle(x, VIEW - 29.0, width, 25.0, cola([10, 14, 20], 0.94));
                    draw_rectangle(x, VIEW - 29.0, 3.0, 25.0, col(box_color(w)));
                    g.text(&text, VW / 2.0, VIEW - 16.0, size, col([237, 241, 247]), 0.5);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn card(
        &self,
        g: &mut Gfx,
        rect: Rect,
        tk: &TankView,
        view: &View,
        me: Option<usize>,
        lead: Option<usize>,
        roster: &Roster,
        most: usize,
        life: f32,
        now: f64,
    ) {
        let i = tk.slot.min(MAX_PLAYERS - 1);
        let c = COLORS[i];
        let compact = rect.w < 200.0;
        let mine = Some(i) == me;
        // Kartenhintergrund
        draw_rectangle(rect.x, rect.y, rect.w, rect.h, col([26, 29, 38]));
        draw_rectangle(rect.x + 6.0, rect.y + 2.0, rect.w - 12.0, rect.h / 2.0, Color::new(1.0, 1.0, 1.0, 0.025));
        draw_rectangle_lines(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            if mine { 2.0 } else { 1.0 },
            col(if mine { shade(c, 0.9) } else { [50, 55, 70] }),
        );
        draw_rectangle(rect.x + 8.0, rect.y, rect.w - 16.0, 3.0, col(c));
        let ix = rect.x + if compact { 25.0 } else { 31.0 };
        let icon = if tk.alive { &g.spr.tanks[i] } else { &g.spr.wrecks[i] };
        g.sprite(icon, ix, rect.y + rect.h / 2.0 + 1.0, 0.0, 1.15, WHITE);
        if Some(i) == lead {
            g.sprite(&g.spr.crown, ix, rect.y + 9.0, 0.0, 0.85, WHITE);
        }
        let score = view.scores[i];
        let mut z = 1.0;
        if let Some(pop) = self.score_pop[i] {
            let e = (now - pop) as f32;
            if e < 0.7 {
                z = 1.0 + 0.8 * (1.0 - ease_out(e / 0.7));
                g.light(ACCENT_C, rect.x + rect.w - 30.0, rect.y + rect.h / 2.0, 40.0, 5.0 * (1.0 - e / 0.7));
                g.flush_light();
            }
        }
        let num = score.to_string();
        let size = if compact { 34.0 } else { 42.0 };
        let nw = g.measure(&num, size, true).width;
        g.fancy(
            &num,
            rect.x + rect.w - 10.0 - nw / 2.0,
            rect.y + rect.h / 2.0 + 2.0,
            size,
            if tk.alive { [255, 255, 255] } else { DIM_C },
            3.0,
            true,
            1.0,
            z,
        );
        let mut tags: Vec<&str> = Vec::new();
        if mine {
            tags.push("DU");
        }
        if view.is_bot(i) {
            tags.push("BOT");
        }
        if roster.admin[i] {
            tags.push("HOST");
        }
        let (row_x, row_y);
        if compact {
            row_x = rect.x + 52.0;
            row_y = rect.y + 44.0;
            for (j, tag) in tags.iter().enumerate() {
                self.tag(g, tag, rect.x + 8.0 + 34.0 * j as f32, rect.y + 6.0, c);
            }
            let nm: String = roster.name(i).chars().take(9).collect();
            g.text(&nm, rect.x + 52.0, rect.y + 22.0, 13.0, col(mix(c, [255, 255, 255], 0.35)), 0.0);
        } else {
            let nm: String = roster.name(i).chars().take(14).collect();
            let w = g.text(&nm, rect.x + 60.0, rect.y + 14.0, 15.0, col(mix(c, [255, 255, 255], 0.35)), 0.0);
            let mut tx = rect.x + 66.0 + w;
            for tag in tags {
                tx += self.tag(g, tag, tx, rect.y + 7.0, c) + 4.0;
            }
            row_x = rect.x + 60.0;
            row_y = rect.y + 31.0;
        }
        if !tk.alive {
            if !compact {
                g.text("zerstört", row_x, row_y, 13.0, col(DIM_C), 0.0);
            }
        } else if let Some(w) = tk.weapon {
            // Waffe mit Restmunition
            g.sprite(&g.spr.badges[&w], row_x + 9.0, row_y + 3.0, 0.0, 0.9, WHITE);
            let bw = if compact { 40.0 } else { 70.0 };
            if w == panzer_core::items::Item::Rail || w == panzer_core::items::Item::Flame {
                let frac = if w == panzer_core::items::Item::Rail {
                    (tk.charge as f32 / RAIL_CHARGE as f32).min(1.0)
                } else {
                    tk.ammo as f32 / w.ammo() as f32
                };
                draw_rectangle(row_x + 24.0, row_y, bw, 6.0, col([46, 50, 62]));
                let fc = if w == panzer_core::items::Item::Rail { [120, 220, 255] } else { [255, 140, 40] };
                draw_rectangle(row_x + 24.0, row_y, bw * frac, 6.0, col(fc));
            } else {
                let mut label = if compact { String::new() } else { w.name().to_string() };
                if w.is_weapon() {
                    label.push_str(&format!(" {}×", tk.ammo));
                }
                let label = label.trim();
                if !label.is_empty() {
                    g.text(label, row_x + 23.0, row_y + 3.0, 13.0, col([200, 205, 215]), 0.0);
                }
            }
        } else {
            // freie Kugeln als leuchtende Punkte
            for b in 0..most {
                let (bx, by) = (row_x + 4.0 + b as f32 * 11.0, row_y + 3.0);
                if (b as i32) < tk.ammo {
                    draw_circle(bx, by, 4.0, col(mix(c, [255, 255, 255], 0.5)));
                    draw_circle(bx, by, 2.0, WHITE);
                } else {
                    draw_circle_lines(bx, by, 3.5, 1.0, col([70, 75, 90]));
                }
            }
        }
        if tk.alive {
            // Effekte mit Restzeit
            let mut ex = row_x;
            let ey = row_y + 21.0;
            let mut items: Vec<(panzer_core::items::Item, f32, f32)> = Vec::new();
            if tk.shield > 0.0 {
                items.push((panzer_core::items::Item::Shield, tk.shield as f32, SHIELD_TIME as f32));
            }
            for (e, v) in active_effects(&tk.eff) {
                if !e.no_icon() {
                    items.push((e.icon(), v as f32, e.full_time() as f32));
                }
            }
            for (item, v, full) in items.into_iter().take(if compact { 2 } else { 4 }) {
                draw_circle(ex + 8.0, ey, 8.0, col([228, 232, 240]));
                g.sprite(&g.spr.icons[&item], ex + 8.0, ey, 0.0, 0.5, WHITE);
                arc(ex + 8.0, ey, 10.0, -FRAC_PI_2, -FRAC_PI_2 - TAU * (v / full).min(1.0), 2.0, col(ACCENT_C));
                ex += 24.0;
            }
        }
        if tk.reload > 0.0 && tk.alive && tk.weapon.is_none() {
            let frac = 1.0 - (tk.reload as f32 / life).min(1.0);
            draw_rectangle(rect.x + 8.0, rect.y + rect.h - 6.0, rect.w - 16.0, 3.0, col([40, 44, 56]));
            draw_rectangle(rect.x + 8.0, rect.y + rect.h - 6.0, (rect.w - 16.0) * frac, 3.0, col(c));
        }
        if !tk.alive {
            draw_rectangle(
                rect.x,
                rect.y,
                rect.w,
                rect.h,
                Color::new(8.0 / 255.0, 9.0 / 255.0, 12.0 / 255.0, 110.0 / 255.0),
            );
        }
        let _ = box_color;
    }

    fn tag(&self, g: &Gfx, text: &str, x: f32, y: f32, c: [u8; 3]) -> f32 {
        let w = g.measure(text, 12.0, false).width + 8.0;
        draw_rectangle(x, y, w, 15.0, col(mix(c, [255, 255, 255], 0.35)));
        g.text(text, x + 4.0, y + 7.5, 12.0, col([14, 15, 19]), 0.0);
        w
    }
}
