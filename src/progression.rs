//! Curva de progressão: como o XP total vira nível (1..=99) e como o trabalho
//! do agente vira XP.
//!
//! Custo pra subir do nível L pro L+1 = `100 × L` (curva acelerante: cada
//! nível pede mais XP que o anterior). Total pra alcançar o nível N é
//! `50·N·(N−1)`. Ganho: ~1000 XP/hora de trabalho acompanhado (~1 ano até o 99).
//! Ver `docs/adr/0001-xp-from-real-agent-work.md`.

use std::time::Duration;

// --- curva ---

/// XP total necessário para ALCANÇAR `level` (nível 1 → 0 XP).
/// `total_ate(N) = 100·(1+2+…+(N−1)) = 50·N·(N−1)`.
pub fn xp_to_reach(level: u8) -> u64 {
    debug_assert!((1..=99).contains(&level), "nível fora de 1..=99: {level}");
    let n = level as u64;
    50 * n * (n - 1)
}

/// Nível (1..=99) correspondente a um total de XP. Satura no 99.
///
/// Maior nível L cujo `xp_to_reach(L)` seja `≤ xp`.
pub fn level_for_xp(xp: u64) -> u8 {
    (1..=99u8)
        .take_while(|&l| xp_to_reach(l) <= xp)
        .last()
        .unwrap_or(1)
}

/// Visão de progresso dentro do nível atual: nível, XP já ganho nele, e total
/// pra subir pro próximo. No 99, `xp_into`/`xp_span` viram 0 (teto).
pub struct LevelView {
    pub level: u8,
    pub xp_into: u64,
    pub xp_span: u64,
}

pub fn level_view(xp: u64) -> LevelView {
    let level = level_for_xp(xp);
    if level >= 99 {
        return LevelView { level: 99, xp_into: 0, xp_span: 0 };
    }
    let base = xp_to_reach(level);
    let next = xp_to_reach(level + 1);
    LevelView { level, xp_into: xp - base, xp_span: next - base }
}

// --- earning ---

/// XP por hora de trabalho ACOMPANHADO (pane aberto + agente `working`).
/// ~1000/h → ~485 h até o nível 99 ≈ 1 ano de uso regular. Knob de calibração.
pub const XP_PER_HOUR_ACCOMPANIED: u64 = 1000;

/// Milissegundos numa hora. Converte o acumulador (ms·XP) em XP inteiro.
const MS_PER_HOUR: u64 = 3_600_000;

/// Acumulador de XP por tempo de trabalho acompanhado. Guarda o resto sub-XP
/// entre ticks sem truncar por tick — matemática inteira, sem float no caminho quente.
pub struct Accrual {
    // Unidades: ms·XP. 1 XP (a XP_PER_HOUR_ACCOMPANIED/h) equivale a MS_PER_HOUR ms·XP.
    acc: u64,
}

impl Accrual {
    pub const fn new() -> Self {
        Self { acc: 0 }
    }

    /// Contabiliza `dt` de trabalho; devolve o XP inteiro ganho e guarda o resto.
    pub fn add_working(&mut self, dt: Duration) -> u64 {
        let ms = dt.as_millis() as u64;
        self.acc += ms * XP_PER_HOUR_ACCOMPANIED;
        let whole = self.acc / MS_PER_HOUR;
        self.acc %= MS_PER_HOUR;
        whole
    }
}

/// XP por unidade do `state_change_seq` no CATCH-UP (pane estava fechado).
/// Empírico: uma sessão de trabalho avança o seq em ~50–60; a 3/seq, um catch-up
/// típico dá ~150–180 XP — bem menos que uma hora acompanhando (~1000). Tunável.
pub const XP_PER_SEQ_CATCHUP: u64 = 3;
/// Teto de delta contabilizado por catch-up (anti-inflação enquanto não calibramos).
pub const MAX_CATCHUP_DELTA: u64 = 2_000;

/// XP de catch-up a partir do delta do `state_change_seq` (trabalho não acompanhado).
pub fn xp_for_catchup(seq_delta: u64) -> u64 {
    seq_delta.min(MAX_CATCHUP_DELTA) * XP_PER_SEQ_CATCHUP
}
