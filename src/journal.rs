//! Diário de sessões: cada pane do pet que fecha vira uma linha no
//! `sessions.jsonl`, ao lado do `state.json`. O `Summary` da sessão hoje morre
//! na tela; aqui ele vira histórico — matéria-prima do `log` e das sequências.
//!
//! Append-only e tolerante: linha ilegível é pulada, nunca derruba a leitura —
//! diário é acessório, não pode quebrar o pet.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Uma sessão encerrada. `day` é a data **local** do fecho (`YYYY-MM-DD`),
/// gravada na hora: quem conta sequência conta os dias do usuário, não os do UTC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub day: String,
    pub started_at: u64,
    pub ended_at: u64,
    pub xp_gained: u64,
    pub xp_total: u64,
    pub level: u8,
    pub agents: usize,
    pub secs_working: u64,
}

/// `state_dir()/sessions.jsonl`.
pub fn path() -> PathBuf {
    todo!("fatia B")
}

/// Anexa uma sessão ao diário padrão.
pub fn append(_e: &Entry) -> std::io::Result<()> {
    todo!("fatia B")
}

/// Anexa num caminho explícito (testável).
pub fn append_to(_p: &Path, _e: &Entry) -> std::io::Result<()> {
    todo!("fatia B")
}

/// Lê o diário padrão. Vazio se não existir.
pub fn load() -> Vec<Entry> {
    todo!("fatia B")
}

/// Lê de um caminho explícito, pulando linhas ilegíveis.
pub fn load_from(_p: &Path) -> Vec<Entry> {
    todo!("fatia B")
}

/// Data local de hoje (`YYYY-MM-DD`), com fallback pra UTC.
pub fn today_local() -> String {
    todo!("fatia B")
}
