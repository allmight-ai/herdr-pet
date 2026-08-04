//! Renderer da casinha LCD: **stdout + ANSI** (sem protocolo gráfico). Cores por
//! raridade, sprite 1-bit, barras. Alinhamento por **largura de display**
//! (`unicode-width`) — símbolos/ANSI contam direito.

use crate::pet::{Pet, Rarity};
use crate::sprites::sprite_for;
use unicode_width::UnicodeWidthChar;

pub const RESET: &str = "\x1b[0m";
pub const DIM: &str = "\x1b[2m";
pub const BOLD: &str = "\x1b[1m";

/// Cor ANSI (256) por raridade; shiny = dourado; shiny Primordial = iridescente.
pub fn ansi_color(rarity: Rarity, shiny: bool, frame: u32) -> String {
    if shiny && rarity == Rarity::Primordial {
        return rainbow(frame);
    }
    if shiny {
        return "\x1b[1;38;5;220m".into();
    }
    match rarity {
        Rarity::Common => "\x1b[38;5;46m".into(),
        Rarity::Uncommon => "\x1b[38;5;51m".into(),
        Rarity::Rare => "\x1b[38;5;201m".into(),
        Rarity::Epic => "\x1b[38;5;214m".into(),
        Rarity::Legendary => "\x1b[38;5;196m".into(),
        Rarity::Primordial => "\x1b[1;38;5;129m".into(),
    }
}

/// Cor iridescente animada (shiny Primordial): cicla matizes por frame.
fn rainbow(frame: u32) -> String {
    const HUES: &[u8] = &[201, 213, 177, 129, 99, 92, 165, 53];
    let c = HUES[(frame as usize) % HUES.len()];
    format!("\x1b[1;38;5;{}m", c)
}

/// Barra visual: █ cheio, ░ vazio.
pub fn bar(numer: u16, denom: u16, width: usize) -> String {
    let f = if denom == 0 {
        0
    } else {
        ((numer as u64 * width as u64) / denom as u64) as usize
    };
    let f = f.min(width);
    format!("{}{}", "█".repeat(f), "░".repeat(width - f))
}

/// Largura de DISPLAY (colunas reais no terminal), ignorando escapes ANSI.
fn display_width(s: &str) -> usize {
    let mut w = 0usize;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c2 in chars.by_ref() {
                if c2 == 'm' {
                    break;
                }
            }
        } else {
            w += UnicodeWidthChar::width(c).unwrap_or(0);
        }
    }
    w
}

fn row_left(o: &mut String, content: &str, w: usize) {
    let spaces = w.saturating_sub(display_width(content));
    o.push_str("│ ");
    o.push_str(content);
    o.push_str(&" ".repeat(spaces));
    o.push_str(" │\n");
}

fn blank(o: &mut String, w: usize) {
    o.push_str(&format!("│ {} │\n", " ".repeat(w)));
}

fn sep(o: &mut String, w: usize) {
    o.push_str(&format!("├─{}─┤\n", "─".repeat(w)));
}

fn row_sprite(o: &mut String, visual: &str, color: &str, w: usize) {
    let vlen = display_width(visual);
    let inner = w + 2;
    let pad = inner.saturating_sub(vlen) / 2;
    o.push_str("│");
    o.push_str(&" ".repeat(pad));
    o.push_str(color);
    o.push_str(visual);
    o.push_str(RESET);
    let right = inner.saturating_sub(vlen + pad);
    o.push_str(&" ".repeat(right));
    o.push_str("│\n");
}

/// Desenha a casinha LCD completa (frame = animação idle + cor iridescente).
pub fn render_casinha(pet: &Pet, frame: u32) -> String {
    const W: usize = 26;
    let color = ansi_color(pet.rarity, pet.shiny, frame);
    let sprite = sprite_for(pet.species.id);
    let bounce = if frame % 4 >= 2 { 1 } else { 0 };

    let mut o = String::new();
    o.push_str(&format!("┌─{}─┐\n", "─".repeat(W)));

    // nome colorido (cor do tier/shiny)
    row_left(&mut o, &format!("{}{}{}", color, pet.name, RESET), W);
    // espécie · tier (+ "(shiny)") — texto puro, alinha sem dor de cabeça
    let shiny_tag = if pet.shiny { " (shiny)" } else { "" };
    row_left(
        &mut o,
        &format!("{} · {}{}", pet.species.name, pet.rarity.as_str(), shiny_tag),
        W,
    );
    sep(&mut o, W);

    for _ in 0..(2 + bounce) {
        blank(&mut o, W);
    }
    for line in sprite {
        row_sprite(&mut o, line, &color, W);
    }
    for _ in 0..(2usize.saturating_sub(bounce)) {
        blank(&mut o, W);
    }
    sep(&mut o, W);

    row_left(
        &mut o,
        &format!("HP {} {:>3}", bar(pet.stats.hp_max, pet.stats.hp_max, 12), pet.stats.hp_max),
        W,
    );
    row_left(
        &mut o,
        &format!("SP {} {:>3}", bar(pet.stats.sp_max, pet.stats.sp_max, 12), pet.stats.sp_max),
        W,
    );
    row_left(
        &mut o,
        &format!("IV {} {:>3}/186", bar(pet.iv.total(), 186, 12), pet.iv.total()),
        W,
    );
    row_left(
        &mut o,
        &format!("ATK {}  DEF {}  SpA {}", pet.stats.atk, pet.stats.def, pet.stats.sp_atk),
        W,
    );
    row_left(
        &mut o,
        &format!("SpD {}  SPE {}", pet.stats.sp_def, pet.stats.speed),
        W,
    );

    o.push_str(&format!("└─{}─┘", "─".repeat(W)));
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_respects_width() {
        assert_eq!(bar(5, 10, 10), "█████░░░░░");
        assert_eq!(bar(0, 10, 10), "░░░░░░░░░░");
        assert_eq!(bar(10, 10, 10), "██████████");
    }

    #[test]
    fn display_width_ignores_ansi() {
        assert_eq!(display_width("\x1b[38;5;46mabc\x1b[0m"), 3);
        assert_eq!(display_width("plain"), 5);
    }

    #[test]
    fn primordial_shiny_is_iridescent() {
        let a = ansi_color(Rarity::Primordial, true, 0);
        let b = ansi_color(Rarity::Primordial, true, 1);
        assert_ne!(a, b);
    }
}
