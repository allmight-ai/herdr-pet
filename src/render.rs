//! Renderer da casinha LCD: **stdout + ANSI** (sem protocolo gráfico — confirmado
//! na doc do Herdr). Cores por raridade, sprite 1-bit, barras de HP/SP/IV.

use crate::pet::{Pet, Rarity};
use crate::sprites::sprite_for;

pub const RESET: &str = "\x1b[0m";
pub const DIM: &str = "\x1b[2m";
pub const BOLD: &str = "\x1b[1m";

/// Cor ANSI (256) por raridade; shiny = dourado.
pub fn ansi_color(rarity: Rarity, shiny: bool) -> &'static str {
    if shiny {
        return "\x1b[38;5;220m";
    }
    match rarity {
        Rarity::Common => "\x1b[38;5;46m", // verde LCD
        Rarity::Uncommon => "\x1b[38;5;51m", // cyan
        Rarity::Rare => "\x1b[38;5;201m", // magenta
        Rarity::Epic => "\x1b[38;5;214m", // âmbar
        Rarity::Legendary => "\x1b[38;5;196m", // vermelho
    }
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

/// Comprimento "visual" de uma string com escapes ANSI (ignora os escapes).
fn visual_len(s: &str) -> usize {
    let mut len = 0usize;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c2 in chars.by_ref() {
                if c2 == 'm' {
                    break;
                }
            }
        } else {
            len += 1;
        }
    }
    len
}

/// Linha `│ {content alinhado à esquerda} │` com largura interna W.
fn row_left(o: &mut String, content: &str, w: usize) {
    let spaces = w.saturating_sub(visual_len(content));
    o.push_str("│ ");
    o.push_str(content);
    o.push_str(&" ".repeat(spaces));
    o.push_str(" │\n");
}

/// Linha em branco dentro da casinha.
fn blank(o: &mut String, w: usize) {
    o.push_str(&format!("│ {} │\n", " ".repeat(w)));
}

/// Linha `├───┤`.
fn sep(o: &mut String, w: usize) {
    o.push_str(&format!("├─{}─┤\n", "─".repeat(w)));
}

/// Sprite centralizado e colorido.
fn row_sprite(o: &mut String, visual: &str, color: &str, w: usize) {
    let vlen = visual.chars().count();
    let inner = w + 2; // largura interna considerando os 2 espaços do row
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

/// Desenha a casinha LCD completa do pet (frame = passo da animação idle).
pub fn render_casinha(pet: &Pet, frame: u32) -> String {
    const W: usize = 26; // largura interna útil
    let color = ansi_color(pet.rarity, pet.shiny);
    let sprite = sprite_for(pet.species.id);
    let bounce = if frame % 4 >= 2 { 1 } else { 0 }; // idle: sprite sobe/desce

    let mut o = String::new();
    o.push_str(&format!("┌─{}─┐\n", "─".repeat(W)));

    // header
    let star = if pet.shiny { " ✨" } else { "" };
    row_left(
        &mut o,
        &format!("{}{}{}", pet.name, star, RESET),
        W,
    );
    row_left(
        &mut o,
        &format!("{}{}{} · {}{}{}", pet.species.name, RESET, DIM, color, pet.rarity.as_str(), RESET),
        W,
    );
    sep(&mut o, W);

    // tela LCD: espaço + sprite (com bounce) + espaço
    for _ in 0..(2 + bounce) {
        blank(&mut o, W);
    }
    for line in sprite {
        row_sprite(&mut o, line, color, W);
    }
    for _ in 0..(2usize.saturating_sub(bounce)) {
        blank(&mut o, W);
    }
    sep(&mut o, W);

    // stats
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
    fn visual_len_ignores_ansi() {
        assert_eq!(visual_len("\x1b[38;5;46mabc\x1b[0m"), 3);
        assert_eq!(visual_len("plain"), 5);
    }
}
