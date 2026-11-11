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
use crate::raster::mix;
use crate::sprites::box_color;
use crate::theme;

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
        _glow: bool,
    ) {
        let width = g.measure(text, size * scale.max(0.05), true).width;
        draw_rectangle(
            cx - width / 2.0 - 16.0,
            cy - size * scale / 2.0 - 8.0,
            width + 32.0,
            size * scale + 16.0,
            cola([28, 31, 30], alpha * 0.88),
        );
        g.fancy(text, cx, cy, size, c, 0.0, true, alpha, scale.max(0.05));
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
            let k_in = ease_out(t / 0.35);
            let fade = ((2.0 - t) / 0.5).clamp(0.0, 1.0);
            self.banner(
                g,
                &format!("Runde {}", view.maze_id),
                48.0,
                [255, 255, 255],
                cx,
                cy - 40.0,
                0.6 + 0.4 * k_in,
                fade,
                true,
            );
            g.fancy(th.name, cx, cy + 2.0, 22.0, th.accent, 1.0, false, fade, 1.0);
            if let Some(m) = arena.mutator {
                self.banner(
                    g,
                    m.name(),
                    32.0,
                    theme::YELLOW,
                    cx,
                    cy + 48.0,
                    0.6 + 0.4 * ease_out((t - 0.25) / 0.35),
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
        draw_rectangle(0.0, VIEW, VW, VH - VIEW, col(theme::DECK));
        draw_rectangle(0.0, VH - 20.0, VW, 20.0, col(theme::PAPER));
        draw_line(0.0, VIEW, VW, VIEW, 1.0, col(theme::DECK_RAISED));
        let bp = bullet_params(arena.mutator);
        let lead = leader(view);
        let mut tanks: Vec<&TankView> = view.tanks.iter().collect();
        if let Some(slot) = me {
            tanks.sort_by_key(|t| (Some(t.slot) != Some(slot), t.slot));
        }
        let n = tanks.len().max(1);
        let gap = 8.0;
        let available = VW - 24.0 - gap * (n as f32 - 1.0);
        let own_width = if me.is_some() && n > 1 { 272.0 } else { 0.0 };
        let weight: f32 = tanks.iter().map(|t| if me.is_none() && !view.is_bot(t.slot) { 2.0 } else { 1.0 }).sum();
        let mut x: f32 = 12.0;
        for tank in tanks {
            let featured = Some(tank.slot) == me || (me.is_none() && !view.is_bot(tank.slot));
            let width = if own_width > 0.0 {
                if Some(tank.slot) == me {
                    own_width
                } else {
                    (available - own_width) / (n - 1) as f32
                }
            } else {
                available * if featured { 2.0 } else { 1.0 } / weight.max(1.0)
            };
            let rect = Rect::new(x.round(), VIEW + 8.0, width.floor(), VH - VIEW - 32.0);
            self.card(g, rect, tank, view, me, featured, lead, roster, bp.most, bp.life as f32, now);
            if let Some(w) = tank.weapon {
                self.weapon_help.insert(tank.slot, (w, now));
            }
            x += width + gap;
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
        r: Rect,
        tk: &TankView,
        view: &View,
        me: Option<usize>,
        featured: bool,
        lead: Option<usize>,
        roster: &Roster,
        most: usize,
        life: f32,
        now: f64,
    ) {
        let slot = tk.slot.min(MAX_PLAYERS - 1);
        let team = COLORS[slot];
        let featured = featured && r.w >= 140.0;
        if featured {
            theme::plate(r, theme::DECK_RAISED, theme::DECK_RAISED);
            draw_rectangle(r.x + 1.0, r.y + 8.0, 3.0, r.h - 16.0, col(team));
        } else {
            draw_line(r.x - 4.0, r.y + 8.0, r.x - 4.0, r.bottom() - 8.0, 1.0, col(theme::DECK_RAISED));
        }
        let text = if tk.alive { theme::ON_DECK } else { theme::DECK_DIM };
        let score = view.scores[slot].to_string();
        let score_size = if featured { 24.0 } else { 22.0 };
        if self.score_pop[slot].is_some_and(|at| now - at < 0.7) {
            draw_rectangle(r.right() - 32.0, r.y + 35.0, 20.0, 2.0, col(theme::YELLOW));
        }
        g.text(&score, r.right() - 10.0, r.y + if featured { 13.0 } else { 29.0 }, score_size, col(text), 1.0);
        let icon = if tk.alive { &g.spr.tanks[slot] } else { &g.spr.wrecks[slot] };
        g.sprite(icon, r.x + 24.0, r.y + 22.0, -0.2, if featured { 1.0 } else { 0.7 }, WHITE);
        if Some(slot) == lead {
            g.sprite(&g.spr.crown, r.x + 24.0, r.y + 5.0, 0.0, 0.45, WHITE);
        }
        let name = if Some(slot) == me { format!("Du · {}", roster.name(slot)) } else { roster.name(slot) };
        let name =
            theme::fit(g, &name, if featured { 13.0 } else { 11.0 }, if featured { r.w - 104.0 } else { r.w - 12.0 });
        g.text(
            &name,
            r.x + if featured { 48.0 } else { 4.0 },
            r.y + if featured { 12.0 } else { 6.0 },
            if featured { 13.0 } else { 11.0 },
            col(theme::DECK_DIM),
            0.0,
        );
        if !tk.alive {
            if featured {
                g.text("Zerstört", r.x + 48.0, r.y + 35.0, 16.0, col(theme::DECK_DIM), 0.0);
            }
            return;
        }
        let row_x = r.x + if featured { 48.0 } else { 6.0 };
        let row_y = r.y + 48.0;
        if let Some(w) = tk.weapon {
            if featured {
                let label = theme::fit(g, w.name(), 16.0, r.w - 60.0);
                g.text(&label, row_x, r.y + 33.0, 16.0, col(text), 0.0);
                g.sprite(&g.spr.badges[&w], r.x + 24.0, row_y, 0.0, 0.85, WHITE);
            } else {
                g.sprite(&g.spr.badges[&w], row_x + 6.0, row_y, 0.0, 0.65, WHITE);
            }
            let ammo_x = row_x + if featured { 0.0 } else { 18.0 };
            if w == panzer_core::items::Item::Rail {
                g.text(
                    if tk.charge > 0.0 {
                        "Lädt"
                    } else if featured {
                        "Feuer halten"
                    } else {
                        "Rail"
                    },
                    ammo_x,
                    row_y,
                    11.0,
                    col(theme::DECK_DIM),
                    0.0,
                );
                let fraction = (tk.charge as f32 / RAIL_CHARGE as f32).clamp(0.0, 1.0);
                let bw = if featured { 80.0 } else { (r.w - 20.0).max(8.0) };
                draw_rectangle(r.x + 8.0, r.bottom() - 5.0, bw, 3.0, col(theme::DECK_RAISED));
                draw_rectangle(r.x + 8.0, r.bottom() - 5.0, bw * fraction, 3.0, col([120, 220, 255]));
            } else {
                let label = if featured { format!("{} / {}", tk.ammo, w.ammo()) } else { format!("{}×", tk.ammo) };
                g.text(&label, ammo_x, row_y, 12.0, col(theme::DECK_DIM), 0.0);
            }
        } else {
            if featured {
                g.text("Bordkanone", row_x, r.y + 33.0, 16.0, col(text), 0.0);
            }
            let step = if featured { 10.0 } else { 6.0 };
            for b in 0..most {
                let x = row_x + b as f32 * step + 2.0;
                let bright = (b as i32) < tk.ammo;
                if bright {
                    draw_circle(x, row_y, if featured { 3.0 } else { 2.0 }, col(team));
                } else {
                    draw_circle_lines(x, row_y, if featured { 3.0 } else { 2.0 }, 1.0, col(theme::DECK_DIM));
                }
            }
        }
        let mut effects = Vec::new();
        if tk.shield > 0.0 {
            effects.push((panzer_core::items::Item::Shield, tk.shield as f32, SHIELD_TIME as f32));
        }
        for (effect, value) in active_effects(&tk.eff) {
            if !effect.no_icon() {
                effects.push((effect.icon(), value as f32, effect.full_time() as f32));
            }
        }
        for (j, (item, value, full)) in
            effects.into_iter().take(if featured && r.w >= 220.0 { 4 } else { 2 }).enumerate()
        {
            let x = r.right() - 12.0 - j as f32 * 18.0;
            g.sprite(&g.spr.icons[&item], x, row_y, 0.0, 0.5, WHITE);
            arc(x, row_y, 7.0, -FRAC_PI_2, -FRAC_PI_2 - TAU * (value / full).min(1.0), 1.3, col(theme::DECK_DIM));
        }
        if tk.reload > 0.0 && tk.weapon.is_none() {
            let fraction = 1.0 - (tk.reload as f32 / life).clamp(0.0, 1.0);
            draw_rectangle(r.x + 8.0, r.bottom() - 5.0, r.w - 16.0, 2.0, col(theme::DECK_RAISED));
            draw_rectangle(r.x + 8.0, r.bottom() - 5.0, (r.w - 16.0) * fraction, 2.0, col(team));
        }
    }
}
