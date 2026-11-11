//! Shared interface palette. Player and weapon colors belong to the arena.
use macroquad::prelude::*;

use crate::gfx::{col, Gfx};

pub const PAPER: [u8; 3] = [247, 244, 235];
pub const SURFACE: [u8; 3] = [255, 253, 247];
pub const RAISED: [u8; 3] = [235, 232, 222];
pub const LINE: [u8; 3] = [211, 208, 197];
pub const INK: [u8; 3] = [39, 43, 42];
pub const SECONDARY: [u8; 3] = [91, 96, 90];
pub const YELLOW: [u8; 3] = [245, 194, 62];
pub const DECK: [u8; 3] = [29, 35, 36];
pub const DECK_RAISED: [u8; 3] = [43, 50, 51];
pub const ON_DECK: [u8; 3] = [242, 239, 226];
pub const DECK_DIM: [u8; 3] = [168, 181, 178];

/// Arena equipment has a clipped corner, so it reads apart from an editable text field.
pub fn plate(r: Rect, fill: [u8; 3], edge: [u8; 3]) {
    let shape = |r: Rect| {
        let cut = 8.0f32.min(r.h * 0.25).min(r.w * 0.25);
        [
            (r.x, r.y),
            (r.right() - cut, r.y),
            (r.right(), r.y + cut),
            (r.right(), r.bottom()),
            (r.x + cut, r.bottom()),
            (r.x, r.bottom() - cut),
        ]
    };
    crate::gfx::poly(&shape(r), col(edge));
    crate::gfx::poly(&shape(Rect::new(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0)), col(fill));
}

/// The selected action repeats the tank tread pattern, without an additional icon family.
pub fn treads(x: f32, y: f32, c: [u8; 3]) {
    for j in 0..3 {
        draw_line(x + j as f32 * 5.0, y, x + j as f32 * 5.0 + 3.0, y + 9.0, 2.0, col(c));
    }
}

pub fn keyhint(g: &Gfx, key: &str, label: &str, x: f32, y: f32, dark: bool) -> f32 {
    let (fill, text, edge) = if dark { (DECK_RAISED, ON_DECK, DECK_DIM) } else { (SURFACE, INK, LINE) };
    let w = g.measure(key, 11.0, false).width + 14.0;
    outlined(Rect::new(x, y - 11.0, w, 22.0), 4.0, fill, edge);
    g.text(key, x + w / 2.0, y, 11.0, col(text), 0.5);
    w + 8.0 + g.text(label, x + w + 8.0, y, 12.0, col(if dark { DECK_DIM } else { SECONDARY }), 0.0)
}

/// Rounded surfaces without a texture or an additional render pass.
pub fn panel(r: Rect, radius: f32, color: [u8; 3]) {
    let k = radius.min(r.w / 2.0).min(r.h / 2.0);
    let c = col(color);
    draw_rectangle(r.x + k, r.y, r.w - k * 2.0, r.h, c);
    draw_rectangle(r.x, r.y + k, r.w, r.h - k * 2.0, c);
    for x in [r.x + k, r.x + r.w - k] {
        for y in [r.y + k, r.y + r.h - k] {
            draw_circle(x, y, k, c);
        }
    }
}

pub fn outlined(r: Rect, radius: f32, fill: [u8; 3], edge: [u8; 3]) {
    panel(r, radius, edge);
    panel(Rect::new(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0), (radius - 1.0).max(0.0), fill);
}

/// Fit complete Unicode characters, including the ellipsis, to the available width.
/// Leiht sich den Text, wenn er passt; nur abgeschnittene Texte werden neu gebaut.
pub fn fit<'a>(g: &Gfx, text: &'a str, size: f32, width: f32) -> std::borrow::Cow<'a, str> {
    use std::borrow::Cow;
    if g.measure(text, size, false).width <= width {
        return Cow::Borrowed(text);
    }
    let mut s = String::with_capacity(text.len() + 1);
    s.push_str(text);
    while !s.is_empty() {
        s.pop();
        s.push('…');
        if g.measure(&s, size, false).width <= width {
            return Cow::Owned(s);
        }
        s.pop();
    }
    Cow::Borrowed("")
}

/// Inputs keep the caret and the end being edited in view.
pub fn input_text(g: &Gfx, text: &str, size: f32, width: f32) -> String {
    if g.measure(text, size, false).width <= width {
        return text.to_string();
    }
    for (at, _) in text.char_indices().skip(1) {
        let tail = format!("…{}", &text[at..]);
        if g.measure(&tail, size, false).width <= width {
            return tail;
        }
    }
    String::new()
}
