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

/// Display agregado pro loop `watch`: **(status, tarefa)** que o pet mostra.
///
/// Se **qualquer** agente está `working`, o pet acorda (`Working`) e mostra a
/// tarefa de quem trabalha — preferindo o agente focado se ele também trabalha
/// (pet dockado numa sessão working), senão o primeiro `working`. Quando ninguém
/// trabalha, espelha o agente focado (idle/blocked/done/unknown). Só `working`
/// prevalece no agregado: é o sinal de trabalho que importa pro pet.
///
/// O "focado" segue a mesma prioridade de `focused_agent_info`: marcado `focused`
/// → mesmo `HERDR_WORKSPACE_ID` → primeiro da lista. `agents` costuma vir de
/// `all_agents_info` (um único `herdr agent list`).
pub fn aggregate_display(agents: &[AgentInfo]) -> (AgentStatus, Option<String>) {
    let ws = std::env::var("HERDR_WORKSPACE_ID").ok();
    let focused = agents
        .iter()
        .find(|a| a.focused)
        .or_else(|| agents.iter().find(|a| a.workspace_id.as_deref() == ws.as_deref()))
        .or_else(|| agents.first());

    // Alguém working → pet acordado. Tarefa: a do focado se ele trabalha; senão a
    // do primeiro working.
    let chosen_worker = focused
        .filter(|f| matches!(f.status, AgentStatus::Working))
        .or_else(|| agents.iter().find(|a| matches!(a.status, AgentStatus::Working)));

    match chosen_worker {
        Some(w) => (AgentStatus::Working, w.title.clone()),
        None => focused
            .map(|f| (f.status, f.title.clone()))
            .unwrap_or((AgentStatus::Idle, None)),
    }
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

    // --- aggregate_display: o pet "vê todos" os agentes ---

    fn ai(status: AgentStatus, title: Option<&str>, focused: bool) -> AgentInfo {
        AgentInfo {
            status,
            title: title.map(|s| s.to_string()),
            state_change_seq: 0,
            pane_id: String::new(),
            workspace_id: None,
            focused,
        }
    }

    #[test]
    fn aggrega_working_de_outra_sessao_acorda_pet() {
        // Pet dockado no idle (focado); outra sessão working → acorda com a tarefa dela.
        let agents = [
            ai(AgentStatus::Idle, Some("idle task"), true),
            ai(AgentStatus::Working, Some("refatorando auth"), false),
        ];
        let (status, title) = aggregate_display(&agents);
        assert_eq!(status, AgentStatus::Working);
        assert_eq!(title.as_deref(), Some("refatorando auth"));
    }

    #[test]
    fn aggrega_focado_working_mostra_tarefa_dele() {
        let agents = [ai(AgentStatus::Working, Some("minha task"), true)];
        let (status, title) = aggregate_display(&agents);
        assert_eq!(status, AgentStatus::Working);
        assert_eq!(title.as_deref(), Some("minha task"));
    }

    #[test]
    fn aggrega_todos_idle_espelha_focado() {
        let agents = [
            ai(AgentStatus::Idle, Some("outra"), false),
            ai(AgentStatus::Idle, Some("focada"), true),
        ];
        let (status, title) = aggregate_display(&agents);
        assert_eq!(status, AgentStatus::Idle);
        assert_eq!(title.as_deref(), Some("focada"));
    }

    #[test]
    fn aggrega_sem_agentes_devolve_idle() {
        let (status, title) = aggregate_display(&[]);
        assert_eq!(status, AgentStatus::Idle);
        assert_eq!(title, None);
    }

    #[test]
    fn aggrega_multiplos_working_prefere_focado_working() {
        // 2 working; o focado também é working → tarefa do focado.
        let agents = [
            ai(AgentStatus::Working, Some("outra working"), false),
            ai(AgentStatus::Working, Some("focada working"), true),
        ];
        let (status, title) = aggregate_display(&agents);
        assert_eq!(status, AgentStatus::Working);
        assert_eq!(title.as_deref(), Some("focada working"));
    }
}
