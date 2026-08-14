//! State persistente do companion.
//!
//! Em `HERDR_PLUGIN_STATE_DIR` (pane do Herdr) ou no dir XDG do plugin
//! (`~/.local/state/herdr/plugins/allmight-ai.herdr-pet`) — padrão de leitura E
//! escrita fora do pane. `.herdr-pet-state/` no CWD só por compat com dev
//! antigo (se já existir); nunca é criado implicitamente. Guarda a âncora
//! (lock-in no primeiro GitHub ID), o índice ativo e os índices já chocados.
//! A raridade não vive só no disco — é rederivável da âncora.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
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
        // Dedupe por pane (maior seq neste tick). Snapshot já descarta pane/seq
        // omitidos; se o mesmo pane vier 2×, o maior valor é o observado.
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
            self.remember_seq(pane_id, *observed);
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
    /// contagem). Dedupe pelo maior seq do slice. Se o observado for menor que o last
    /// (reset genuíno do Herdr), a baseline desce; o 0 espúrio da API omitida não
    /// chega — `agent::snapshot` descarta PaneSeq incompleto.
    pub fn record_seen_seq(&mut self, agents: &[PaneSeq]) {
        let mut latest: HashMap<&str, u64> = HashMap::new();
        for ps in agents {
            latest
                .entry(&ps.pane_id)
                .and_modify(|e| *e = (*e).max(ps.seq))
                .or_insert(ps.seq);
        }
        for (pane_id, observed) in latest {
            self.remember_seq(pane_id, observed);
        }
    }

    /// Grava o seq observado. `observed < last` é reset genuíno (Herdr reiniciou
    /// ou o pane_id foi reusado): rebobina sem creditar neste tick — o ganho já
    /// saiu 0 via `saturating_sub`. Deltas seguintes partem do novo zero.
    ///
    /// Replay 200→0→200 de campo omitido continua morto: o 0 espúrio não passa
    /// do snapshot (`Option`, descarta ausente). Um 0 que chega aqui é real.
    fn remember_seq(&mut self, pane_id: &str, observed: u64) {
        self.last_seq_by_pane.insert(pane_id.to_string(), observed);
    }
}

/// Diretório do state, nesta ordem (núcleo puro em `resolve_state_dir`):
/// 1. `HERDR_PLUGIN_STATE_DIR` (pane do plugin — verdade do Herdr)
/// 2. dir XDG do plugin **com** `state.json` já presente
/// 3. `.herdr-pet-state/state.json` no CWD (compat com dev antigo — nunca criado)
/// 4. dir XDG do plugin — padrão pra LER E ESCREVER fora do pane (o save cria)
///
/// A regra nova do passo 4 fecha o C10: `herdr-pet init` num shell qualquer grava
/// no dir do plugin (o mesmo do pane), não num `.herdr-pet-state/` órfão do CWD —
/// um state só, âncora única. O passo 3 mantém o dev que JÁ tinha state local
/// funcionando; o passo 2 preserva a precedência antiga (install ganha de dev).
pub fn state_dir() -> PathBuf {
    resolve_state_dir(
        std::env::var("HERDR_PLUGIN_STATE_DIR").ok().as_deref(),
        herdr_plugin_state_dir(),
        cwd_dev_state().as_deref(),
    )
}

/// `.herdr-pet-state/state.json` no CWD, se existir (compat dev antigo).
fn cwd_dev_state() -> Option<PathBuf> {
    let p = PathBuf::from(".herdr-pet-state/state.json");
    p.is_file().then_some(p)
}

/// Núcleo de `state_dir` (puro, testável sem mexer em env global). `cwd_dev_state`
/// sendo `Some` significa que o state de dev existe no CWD.
fn resolve_state_dir(
    plugin_env: Option<&str>,
    xdg_dir: Option<PathBuf>,
    cwd_dev_state: Option<&Path>,
) -> PathBuf {
    if let Some(d) = plugin_env {
        return PathBuf::from(d);
    }
    if let Some(x) = &xdg_dir {
        if x.join("state.json").is_file() {
            return x.clone();
        }
    }
    if cwd_dev_state.is_some() {
        return PathBuf::from(".herdr-pet-state");
    }
    xdg_dir.unwrap_or_else(|| PathBuf::from(".herdr-pet-state"))
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
///
/// Ausente → `None` (o auto-init pode criar). Presente mas **ilegível**
/// (truncado/corrompido) → preserva ANTES de desistir: copia o conteúdo pra
/// `<path>.corrupt` (numerando se já houver — nunca sobrescreve a cópia
/// anterior) e avisa no stderr; só então devolve `None`. O auto-init recria o
/// state, mas os dados antigos nunca são destruídos silenciosamente (C9).
pub fn load_from(path: &Path) -> Option<State> {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(_) => return None, // ausente (ou inacessível): sem state
    };
    match serde_json::from_slice(&data) {
        Ok(s) => Some(s),
        Err(e) => {
            let dest = preserve_corrupt(path, &data);
            eprintln!("{}", corrupt_warning(path, &dest, &e));
            None
        }
    }
}

/// Copia o conteúdo ilegível pra `<path>.corrupt` (`.corrupt.1`, `.2`, … se já
/// houver — preservas empilham sem sobrescrever). Devolve o destino usado.
fn preserve_corrupt(path: &Path, data: &[u8]) -> PathBuf {
    let mut dest = corrupt_sibling(path, "");
    let mut n = 0;
    while dest.exists() {
        n += 1;
        dest = corrupt_sibling(path, &format!(".{n}"));
    }
    let _ = fs::write(&dest, data);
    dest
}

fn corrupt_sibling(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}.corrupt{suffix}", path.display()))
}

/// Mensagem de aviso (fatorada pra ser testável — o `eprintln` em si não é).
fn corrupt_warning(path: &Path, dest: &Path, err: &serde_json::Error) -> String {
    format!(
        "herdr-pet: state {} ilegível ({}); conteúdo preservado em {} — o state será recriado",
        path.display(),
        err,
        dest.display()
    )
}

/// Salva num caminho explícito (testável). Atômico (C9): tmp + fsync + rename —
/// crash no meio da gravação nunca deixa o `state.json` truncado; ou o arquivo
/// antigo permanece, ou o novo aparece inteiro.
pub fn save_to(path: &Path, state: &State) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(state).expect("state serializa");
    write_atomic(path, &data)
}

/// Gravação atômica: escreve num tmp no mesmo diretório, dá fsync e renomeia.
/// Morava em `setup.rs`; movida pra cá pra o state usar o mesmo mecanismo.
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp-herdr-pet");
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Carrega do state padrão.
pub fn load() -> Option<State> {
    load_from(&state_path())
}

/// Salva no state padrão.
pub fn save(state: &State) -> std::io::Result<()> {
    save_to(&state_path(), state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tdir(tag: &str, with_state: bool) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "herdr-pet-resolve-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        if with_state {
            fs::write(d.join("state.json"), "{}").unwrap();
        }
        d
    }

    #[test]
    fn resolve_prefere_env_do_pane() {
        let d = resolve_state_dir(
            Some("/pane/plugin"),
            Some(tdir("a", true)),
            Some(Path::new(".herdr-pet-state/state.json")),
        );
        assert_eq!(d, PathBuf::from("/pane/plugin"));
    }

    #[test]
    fn resolve_xdg_com_state_vence_dev_do_cwd() {
        // Precedência antiga preservada: install (XDG com state) ganha do dev local.
        let xdg = tdir("b", true);
        let d = resolve_state_dir(
            None,
            Some(xdg.clone()),
            Some(Path::new(".herdr-pet-state/state.json")),
        );
        assert_eq!(d, xdg);
    }

    #[test]
    fn resolve_dev_antigo_do_cwd_quando_xdg_nao_tem_state() {
        let d = resolve_state_dir(
            None,
            Some(tdir("c", false)),
            Some(Path::new(".herdr-pet-state/state.json")),
        );
        assert_eq!(d, PathBuf::from(".herdr-pet-state"));
    }

    #[test]
    fn resolve_sem_nada_padrao_e_xdg_para_escrever() {
        // Regra nova (C10): sem env e sem state em lugar nenhum, o XDG é o padrão
        // de escrita — nunca um `.herdr-pet-state/` implícito no CWD arbitrário.
        let xdg = tdir("d", false);
        let d = resolve_state_dir(None, Some(xdg.clone()), None);
        assert_eq!(d, xdg);
    }
}
