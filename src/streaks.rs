//! Agregação do diário: sessões viram dias, dias viram sequência.
//!
//! Puro — nada de I/O e nada de relógio: `streak` recebe o "hoje" de fora, pra
//! o teste poder mentir sobre a data.

use crate::journal::Entry;

/// Um dia de trabalho, somando todas as sessões daquele dia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Day {
    pub day: String,
    pub xp: u64,
    pub secs_working: u64,
    pub sessions: usize,
}

/// Sequência de dias consecutivos com trabalho: a atual e o recorde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Streak {
    pub current: u32,
    pub best: u32,
    pub last_day: Option<String>,
}

/// Um item por dia, em ordem cronológica.
pub fn by_day(_entries: &[Entry]) -> Vec<Day> {
    todo!("fatia C")
}

/// Sequência atual (só conta se o último dia é hoje ou ontem — quebrou, zerou)
/// e o recorde histórico.
pub fn streak(_days: &[Day], _today: &str) -> Streak {
    todo!("fatia C")
}

/// Dia juliano do calendário civil (algoritmo de Hinnant) — a diferença entre
/// dois deles diz se dois dias são consecutivos, sem depender de fuso.
pub fn days_from_civil(_y: i64, _m: u32, _d: u32) -> i64 {
    todo!("fatia C")
}

/// `"2026-08-19"` → `(2026, 8, 19)`. `None` se não for uma data plausível.
pub fn parse_day(_s: &str) -> Option<(i64, u32, u32)> {
    todo!("fatia C")
}
