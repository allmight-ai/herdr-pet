//! Lê o status do agente do Herdr para o pet reagir.
//!
//! O Herdr expõe o estado via socket API (`herdr agent list`):
//! `working | done | blocked | idle | unknown`. O pane `watch` polla isso
//! e anima o mood. Detecção via Screen Manifest (Claude Code sem config extra).

use serde::Deserialize;

/// Status do agente espelhado pelo pet (enum do Herdr).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Working,
    Done,
    Blocked,
    Idle,
    Unknown,
}

impl AgentStatus {
    pub fn from_herdr(s: &str) -> Self {
        match s {
            "working" => AgentStatus::Working,
            "done" => AgentStatus::Done,
            "blocked" => AgentStatus::Blocked,
            "idle" => AgentStatus::Idle,
            _ => AgentStatus::Unknown,
        }
    }
}

/// Roda o CLI `herdr` procurando-o em ordem: `HERDR_BIN_PATH`, `herdr` no PATH,
/// `~/.local/bin/herdr`. Devolve o stdout em bytes se algum candidado rodar com
/// sucesso; `None` caso contrário (pet fica neutro/`Unknown`).
fn run_herdr(args: &[&str]) -> Option<Vec<u8>> {
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(b) = std::env::var("HERDR_BIN_PATH") {
        candidates.push(b);
    }
    candidates.push("herdr".to_string());
    if let Ok(home) = std::env::var("HOME") {
        candidates.push(format!("{}/.local/bin/herdr", home));
    }
    for c in &candidates {
        if let Ok(out) = std::process::Command::new(c).args(args).output() {
            if out.status.success() {
                return Some(out.stdout);
            }
        }
    }
    None
}

// --- parsing do envelope `herdr agent list` ---
// {"id":"cli:agent:list","result":{"agents":[{"agent_status":"working","focused":true,"workspace_id":"w19",...}],"type":"agent_list"}}

#[derive(Deserialize)]
struct Envelope {
    result: Result_,
}

#[derive(Deserialize)]
struct Result_ {
    agents: Vec<AgentEntry>,
}

#[derive(Deserialize)]
struct AgentEntry {
    agent_status: String,
    #[serde(default)]
    focused: bool,
    #[serde(default)]
    workspace_id: Option<String>,
    #[serde(default)]
    terminal_title_stripped: Option<String>,
    #[serde(default)]
    state_change_seq: u64,
    #[serde(default)]
    pane_id: String,
}

/// Info do agente que o pet acompanha: status + tarefa + seq (pro catch-up) +
/// pane_id (chave do seq por agente) + focused (display espelha o focado).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInfo {
    pub status: AgentStatus,
    pub title: Option<String>,
    pub state_change_seq: u64,
    pub pane_id: String,
    pub workspace_id: Option<String>,
    pub focused: bool,
}

/// Lê **todos** os agentes detectados do Herdr (vazio se a leitura falhar).
fn list_entries() -> Vec<AgentEntry> {
    let Some(stdout) = run_herdr(&["agent", "list"]) else {
        return Vec::new();
    };
    let Ok(env) = serde_json::from_slice::<Envelope>(&stdout) else {
        return Vec::new();
    };
    env.result.agents
}

fn entry_to_info(a: &AgentEntry) -> AgentInfo {
    let title = a
        .terminal_title_stripped
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    AgentInfo {
        status: AgentStatus::from_herdr(&a.agent_status),
        title,
        state_change_seq: a.state_change_seq,
        pane_id: a.pane_id.clone(),
        workspace_id: a.workspace_id.clone(),
        focused: a.focused,
    }
}

/// Info de **todos** os agentes detectados — pra agregar trabalho (XP) em todos os projetos.
pub fn all_agents_info() -> Vec<AgentInfo> {
    list_entries().iter().map(entry_to_info).collect()
}

/// Agente que o pet deve espelhar (display): o **focado**; senão o do mesmo workspace;
/// senão o primeiro. `None` se não há agente ou a leitura falhar.
pub fn focused_agent_info() -> Option<AgentInfo> {
    let agents = list_entries();
    if agents.is_empty() {
        return None;
    }
    let ws = std::env::var("HERDR_WORKSPACE_ID").ok();
    let pick = agents
        .iter()
        .find(|a| a.focused)
        .or_else(|| agents.iter().find(|a| a.workspace_id.as_deref() == ws.as_deref()))
        .or_else(|| agents.first())?;
    Some(entry_to_info(pick))
}

/// Só o status (compat). Veja `focused_agent_info` pra status + tarefa.
pub fn focused_agent_status() -> Option<AgentStatus> {
    focused_agent_info().map(|i| i.status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_herdr_status_strings() {
        assert_eq!(AgentStatus::from_herdr("working"), AgentStatus::Working);
        assert_eq!(AgentStatus::from_herdr("done"), AgentStatus::Done);
        assert_eq!(AgentStatus::from_herdr("blocked"), AgentStatus::Blocked);
        assert_eq!(AgentStatus::from_herdr("idle"), AgentStatus::Idle);
        assert_eq!(AgentStatus::from_herdr("nonsense"), AgentStatus::Unknown);
    }

    #[test]
    fn picks_focused_agent_from_envelope() {
        let raw = r#"{"id":"cli:agent:list","result":{"agents":[
            {"agent":"grok","agent_status":"idle","focused":false,"workspace_id":"w18"},
            {"agent":"claude","agent_status":"working","focused":true,"workspace_id":"w19","state_change_seq":48,"pane_id":"w19:pB"}
        ],"type":"agent_list"}}"#;
        let env: Envelope = serde_json::from_str(raw).unwrap();
        let pick = env.result.agents.iter().find(|a| a.focused).unwrap();
        assert_eq!(AgentStatus::from_herdr(&pick.agent_status), AgentStatus::Working);
        assert_eq!(pick.state_change_seq, 48);
        assert_eq!(pick.pane_id, "w19:pB");
        // ausente → default (0 seq, "" pane_id)
        let grok = env.result.agents.iter().find(|a| !a.focused).unwrap();
        assert_eq!(grok.state_change_seq, 0);
        assert_eq!(grok.pane_id, "");
    }
}
