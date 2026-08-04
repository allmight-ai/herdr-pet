//! Leitura do status do agente do Herdr — o "programador" que o pet acompanha.
//!
//! O Herdr expõe o estado de cada agente via socket API (`herdr agent list`),
//! com o enum: `working | done | blocked | idle | unknown`. O pane `watch`
//! polla isso e reage (animação/mood). Detecção via Screen Manifest TOML
//! (Claude Code: zero config, latência de poucos segundos).
//!
//! v1 (cosmético/stakeless): só animação. Sem progressão — isso é v2.

use serde::Deserialize;

/// Estado do agente espelhado pelo pet (enum real do Herdr 0.8.0,
/// confirmado via `herdr agent wait --until`).
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
}

/// Info do agente que o pet acompanha: status + tarefa atual (terminal_title_stripped).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInfo {
    pub status: AgentStatus,
    pub title: Option<String>,
}

/// Agente que o pet deve espelhar: o **focado** (o que o programador dirige); senão o
/// do mesmo workspace do pane; senão o primeiro. `None` se não há agente ou a leitura falhar.
pub fn focused_agent_info() -> Option<AgentInfo> {
    let stdout = run_herdr(&["agent", "list"])?;
    let env: Envelope = serde_json::from_slice(&stdout).ok()?;
    let agents = env.result.agents;
    if agents.is_empty() {
        return None;
    }
    let ws = std::env::var("HERDR_WORKSPACE_ID").ok();
    let pick = agents
        .iter()
        .find(|a| a.focused)
        .or_else(|| agents.iter().find(|a| a.workspace_id.as_deref() == ws.as_deref()))
        .or_else(|| agents.first())?;
    let title = pick
        .terminal_title_stripped
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    Some(AgentInfo {
        status: AgentStatus::from_herdr(&pick.agent_status),
        title,
    })
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
            {"agent":"claude","agent_status":"working","focused":true,"workspace_id":"w19"}
        ],"type":"agent_list"}}"#;
        let env: Envelope = serde_json::from_str(raw).unwrap();
        let pick = env.result.agents.iter().find(|a| a.focused).unwrap();
        assert_eq!(AgentStatus::from_herdr(&pick.agent_status), AgentStatus::Working);
    }
}
