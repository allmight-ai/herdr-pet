//! State persistente do companion.
//!
//! Em `HERDR_PLUGIN_STATE_DIR` (Herdr) ou `.herdr-pet-state/` (dev).
//! Guarda a âncora (lock-in no primeiro GitHub ID), o índice ativo e os índices
//! já chocados. A raridade não vive só no disco — é rederivável da âncora.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::progression::{level_for_xp, xp_for_catchup};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub anchor: String,
    pub github_id: u64,
    pub active_index: u32,
    pub hatched: Vec<u32>,
    /// XP total do pet ativo. Ausente em states antigos → default 0.
    #[serde(default)]
    pub xp: u64,
    /// Último `state_change_seq` visto do agente (pro catch-up de trabalho não acompanhado).
    #[serde(default)]
    pub last_state_change_seq: u64,
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
            last_state_change_seq: 0,
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

    /// Contabiliza trabalho do agente observado enquanto o pane estava fechado.
    /// Primeira observação (seq 0) trava a baseline sem creditar histórico;
    /// nas seguintes, concede XP pelo delta (ritmo de catch-up) e avança o seq.
    /// Devolve o XP ganho.
    pub fn apply_catchup(&mut self, observed_seq: u64) -> u64 {
        let gained = if self.last_state_change_seq == 0 {
            0
        } else {
            let delta = observed_seq.saturating_sub(self.last_state_change_seq);
            xp_for_catchup(delta)
        };
        self.last_state_change_seq = observed_seq;
        self.xp += gained;
        gained
    }
}

/// Diretório do state: `HERDR_PLUGIN_STATE_DIR` (runtime) ou fallback de dev.
pub fn state_dir() -> PathBuf {
    match std::env::var("HERDR_PLUGIN_STATE_DIR") {
        Ok(d) => PathBuf::from(d),
        Err(_) => PathBuf::from(".herdr-pet-state"),
    }
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
