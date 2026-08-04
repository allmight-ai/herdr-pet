//! Renderer da casinha LCD: **stdout + ANSI** (sem protocolo gráfico). Cores por
//! raridade, sprite 1-bit, barras. Alinhamento por **largura de display**
//! (`unicode-width`) — símbolos/ANSI contam direito.

use crate::agent::AgentStatus;
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
/// Emojis (✨, …) renderizam em 2 colunas no terminal embora unicode-width conte 1.
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
            w += char_width(c);
        }
    }
    w
}

/// Largura de um char. Emojis contam 2 (como o terminal desenha); demais seguem unicode-width.
fn char_width(c: char) -> usize {
    if matches!(c, '✨' | '⭐' | '🌟' | '✅' | '⚡' | '🔥') {
        return 2;
    }
    UnicodeWidthChar::width(c).unwrap_or(0)
}

/// Trunca `s` pra caber em `max` colunas de display, com "…" no fim se passar.
fn truncate_display(s: &str, max: usize) -> String {
    if display_width(s) <= max {
        return s.to_string();
    }
    let mut out = String::new();
    let mut w = 0usize;
    for c in s.chars() {
        let cw = char_width(c);
        if w + cw + 1 > max {
            break;
        }
        w += cw;
        out.push(c);
    }
    out.push('…');
    out
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

/// Mood do pet (label + cor ANSI) derivado do status do agente que ele espelha.
/// Reação puramente cosmética (v1) — sem progressão.
fn mood_of(status: AgentStatus) -> (&'static str, &'static str) {
    match status {
        AgentStatus::Working => ("« treinando »", "\x1b[38;5;46m"),     // verde, energizado
        AgentStatus::Done => ("★ comemorando! ★", "\x1b[1;38;5;220m"), // dourado
        AgentStatus::Blocked => ("?  curioso  ?", "\x1b[38;5;214m"),   // laranja, alerta
        AgentStatus::Idle => ("z z z  dormindo", "\x1b[2;38;5;245m"),  // cinza, sono
        AgentStatus::Unknown => (". . .", "\x1b[2;38;5;245m"),         // cinza, neutro
    }
}

/// Flair animado acima do sprite, por status. LCD-consistente (ascii/block).
fn flair(status: AgentStatus, frame: u32) -> Option<&'static str> {
    match status {
        AgentStatus::Idle => Some("z  z"),                                    // sono
        AgentStatus::Done => Some(if frame % 2 == 0 { "*  *" } else { "  * " }), // sparkles piscando
        AgentStatus::Unknown => Some("?"),                                    // confuso
        AgentStatus::Working | AgentStatus::Blocked => None, // working = bounce; blocked = alerta parado
    }
}

/// Período (em frames) em que o render MUDA — cadência mínima de redraw.
/// `1` = estático (idle/blocked: só redesenha quando o status muda); `>1` = anima.
/// Usado p/ pular redraws quando nada visível mudou (otimização do loop watch).
pub fn animation_period(status: AgentStatus, pet: &Pet) -> u32 {
    if pet.shiny && pet.rarity == Rarity::Primordial {
        8 // iridescente: 8 matizes (ciclo lento)
    } else {
        match status {
            AgentStatus::Working | AgentStatus::Done => 2, // bounce
            AgentStatus::Unknown => 4,                     // bounce lento
            AgentStatus::Idle | AgentStatus::Blocked => 1, // estático
        }
    }
}

/// Desenha a casinha LCD completa. `status` = estado do agente espelhado (drive o mood +
/// o bounce do sprite); `task` = tarefa atual do agente (linha dim, se houver); `frame` = animação.
pub fn render_casinha(pet: &Pet, frame: u32, status: AgentStatus, task: Option<&str>) -> String {
    const W: usize = 28;
    let color = ansi_color(pet.rarity, pet.shiny, frame);
    let sprite = sprite_for(pet.species.id);
    // Bounce por mood: working/done = animado; blocked/idle = parado; unknown = idle padrão.
    let bounce = match status {
        AgentStatus::Working | AgentStatus::Done => (frame % 2) as usize,
        AgentStatus::Blocked | AgentStatus::Idle => 0,
        AgentStatus::Unknown => usize::from(frame % 4 >= 2),
    };
    let (mood, mood_color) = mood_of(status);

    let mut o = String::new();
    o.push_str(&format!("┌─{}─┐\n", "─".repeat(W)));

    // nome colorido (cor do tier/shiny)
    row_left(&mut o, &format!("{}{}{}", color, pet.name, RESET), W);
    // espécie · tier (+ ✨ se shiny). Emoji ✨ renderiza no terminal (gallery/lineage
    // já o usam); display_width conta como 2 p/ manter o alinhamento.
    let shiny_tag = if pet.shiny { " ✨" } else { "" };
    row_left(
        &mut o,
        &format!("{} · {}{}", pet.species.name, pet.rarity.as_title(), shiny_tag),
        W,
    );
    // tarefa atual do agente (se houver) — dim, truncada pra caber
    if let Some(t) = task {
        row_left(
            &mut o,
            &format!("{DIM}{}{RESET}", truncate_display(&format!("» {t}"), W)),
            W,
        );
    }
    sep(&mut o, W);

    // Área do sprite: flair animado (acima, na última linha em branco) + sprite (bounce).
    let above = 2 + bounce;
    for i in 0..above {
        if i + 1 == above {
            if let Some(f) = flair(status, frame) {
                row_sprite(&mut o, f, mood_color, W);
                continue;
            }
        }
        blank(&mut o, W);
    }
    for line in sprite {
        row_sprite(&mut o, line, &color, W);
    }
    for _ in 0..(2usize.saturating_sub(bounce)) {
        blank(&mut o, W);
    }
    // mood do pet — label centralizado + cor, reage ao status do agente
    row_sprite(&mut o, mood, mood_color, W);

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
