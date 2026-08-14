//! State persistente do companion.
//!
//! Em `HERDR_PLUGIN_STATE_DIR` (Herdr) ou `.herdr-pet-state/` (dev).
//! Guarda a âncora (lock-in no primeiro GitHub ID), o índice ativo e os índices
//! já chocados. A raridade não vive só no disco — é rederivável da âncora.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::progression::{harmonic_milli, level_for_xp, xp_for_catchup, MILLI};

/// Sinal de trabalho de um agente: o `state_change_seq` observado num dado pane.
/// Conceito do `CONTEXT.md` ("Sinal de trabalho"); é a chave do mapa `last_seq_by_pane`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSeq {
    pub pane_id: String,
    pub seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub anchor: String,
    pub github_id: u64,
    pub active_index: u32,
    pub hatched: Vec<u32>,
    /// XP total do pet ativo. Ausente em states antigos → default 0.
    #[serde(default)]
    pub xp: u64,
    /// Último `state_change_seq` visto por pane (catch-up de trabalho não acompanhado,
    /// em qualquer projeto). Mapa pane_id → seq; ausente em states antigos → vazio.
    #[serde(default)]
    pub last_seq_by_pane: HashMap<String, u64>,
}

impl State {
    /// State inicial para um GitHub ID (pet #0 já nasceu).
    pub fn new(github_id: u64) -> Self {
        State {
            anchor: crate::forge::anchor_for(github_id),
            github_id,
            active_index: 0,
            hatched: vec![0],
            xp: 0,
            last_seq_by_pane: HashMap::new(),
        }
    }

    /// Marca um índice como chocada e ativa (idempotente).
    pub fn record_hatch(&mut self, index: u32) {
        if !self.hatched.contains(&index) {
            self.hatched.push(index);
        }
        self.active_index = index;
    }

    /// Nível atual do pet ativo (1..=99), derivado do XP total.
    pub fn level(&self) -> u8 {
        level_for_xp(self.xp)
    }

    /// Contabiliza trabalho de **todos** os agentes observados enquanto o pane esteve
    /// fechado. `agents`: `(pane_id, seq observado)` de cada agente. Primeira vista de
    /// um pane = baseline (sem creditar); nas seguintes, XP pelo delta. Vários agentes
    /// avançando sofrem o decaimento harmônico (anti-proliferação). Devolve o XP ganho.
    pub fn apply_catchup(&mut self, agents: &[PaneSeq]) -> u64 {
        // Dedupe por pane (maior seq) — pane duplicado (ex.: pane_id ausente) não dobra a conta.
        let mut latest: HashMap<&str, u64> = HashMap::new();
        for ps in agents {
            latest
                .entry(&ps.pane_id)
                .and_modify(|e| *e = (*e).max(ps.seq))
                .or_insert(ps.seq);
        }
        // Ganhos contra o mapa como estava na entrada: um pane não enxerga o insert de outro.
        let mut linear = 0u64;
        let mut contributors = 0u64;
        for (pane_id, observed) in &latest {
            let gained = match self.last_seq_by_pane.get(*pane_id) {
                Some(&last) => {
                    let g = xp_for_catchup(observed.saturating_sub(last));
                    if g > 0 {
                        contributors += 1;
                    }
                    g
                }
                None => 0, // primeira vista do pane: baseline, sem creditar histórico
            };
            linear += gained;
        }
        for (pane_id, observed) in &latest {
            self.last_seq_by_pane.insert((*pane_id).to_string(), *observed);
        }
        // Fator de largura harmonic_milli(N)/N: 1 agente = MILLI (cheio); mais = menos cada.
        let factor = if contributors > 0 {
            harmonic_milli(contributors as usize) / contributors
        } else {
            0
        };
        let granted = linear * factor / MILLI;
        self.xp += granted;
        granted
    }

    /// Avança a baseline de cada agente **sem creditar XP** — usado no poll enquanto o
    /// pane está aberto, pra o próximo catch-up contar só o período fechado (sem dupla
    /// contagem). O nome carrega o invariante: só registra o que já vimos.
    pub fn record_seen_seq(&mut self, agents: &[PaneSeq]) {
        for ps in agents {
            self.last_seq_by_pane.insert(ps.pane_id.clone(), ps.seq);
        }
    }
}

/// Diretório do state, nesta ordem:
/// 1. `HERDR_PLUGIN_STATE_DIR` (pane do plugin)
/// 2. state do Herdr em disco (`~/.local/state/herdr/plugins/allmight-ai.herdr-pet`)
///    — pra `herdr-pet status` no shell ver o mesmo XP do watch
/// 3. `.herdr-pet-state/` (dev, sem install)
pub fn state_dir() -> PathBuf {
    if let Ok(d) = std::env::var("HERDR_PLUGIN_STATE_DIR") {
        return PathBuf::from(d);
    }
    if let Some(p) = herdr_plugin_state_dir() {
        if p.join("state.json").is_file() {
            return p;
        }
    }
    PathBuf::from(".herdr-pet-state")
}

/// `XDG_STATE_HOME/herdr/plugins/allmight-ai.herdr-pet` (padrão: `~/.local/state/...`).
pub fn herdr_plugin_state_dir() -> Option<PathBuf> {
    let base = match std::env::var("XDG_STATE_HOME") {
        Ok(d) => PathBuf::from(d),
        Err(_) => PathBuf::from(std::env::var("HOME").ok()?).join(".local/state"),
    };
    Some(base.join("herdr/plugins/allmight-ai.herdr-pet"))
}

pub fn state_path() -> PathBuf {
    state_dir().join("state.json")
}

/// Carrega de um caminho explícito (testável, sem depender de env var global).
pub fn load_from(path: &Path) -> Option<State> {
    let data = fs::read(path).ok()?;
    serde_json::from_slice(&data).ok()
}

/// Salva num caminho explícito (testável).
pub fn save_to(path: &Path, state: &State) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(state).expect("state serializa");
    fs::write(path, data)
}

/// Carrega do state padrão.
pub fn load() -> Option<State> {
    load_from(&state_path())
}

/// Salva no state padrão.
pub fn save(state: &State) -> std::io::Result<()> {
    save_to(&state_path(), state)
}
