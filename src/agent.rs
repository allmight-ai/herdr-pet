//! Lê o status do agente do Herdr para o pet reagir.
//!
//! O Herdr expõe o estado via socket API (`herdr agent list`):
//! `working | done | blocked | idle | unknown`. O pane `watch` polla isso
//! e anima o mood. Detecção via Screen Manifest (Claude Code sem config extra).

use serde::Deserialize;

use crate::state::PaneSeq;

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
    #[serde(default)]
    agent: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    agent_session: Option<AgentSession>,
}

#[derive(Deserialize)]
struct AgentSession {
    #[serde(default)]
    value: Option<String>,
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
    /// Rótulo do Herdr (`claude`, `grok`, `glm`, …). Subagentes Claude herdam o do pai.
    pub agent: Option<String>,
    pub cwd: Option<String>,
    /// ID da sessão Claude (`agent_session.value`) — chave da pasta `subagents/`.
    pub session_id: Option<String>,
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
        agent: a.agent.clone(),
        cwd: a.cwd.clone(),
        session_id: a.agent_session.as_ref().and_then(|s| s.value.clone()),
    }
}

/// Info de **todos** os agentes detectados — pra agregar trabalho (XP) em todos os projetos.
/// Claude/GLM ganham os subagentes ainda rodando (o Herdr só lista o processo pai).
pub fn all_agents_info() -> Vec<AgentInfo> {
    let agents: Vec<AgentInfo> = list_entries().iter().map(entry_to_info).collect();
    crate::subagents::expand(&agents)
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

/// Display agregado pro loop `watch`: **(status, tarefas)** que o pet mostra.
///
/// Se **qualquer** agente está `working`, o pet acorda (`Working`) e a lista
/// devolve as tarefas de todos os working — o focado-working primeiro (a rotação
/// do `watch` começa por ele), depois os demais. Com vários working, o loop
/// **rotaciona** entre essas tarefas. Quando ninguém trabalha, devolve a tarefa
/// do agente focado (única) e espelha o status dele (idle/blocked/done/unknown).
/// Títulos vazios são descartados.
///
/// Só `working` prevalece no agregado: é o sinal de trabalho que importa pro
/// pet. O "focado" segue a prioridade de `focused_agent_info`: marcado `focused`
/// → mesmo `HERDR_WORKSPACE_ID` → primeiro da lista. `agents` costuma vir de
/// `all_agents_info` (um único `herdr agent list`).
pub fn aggregate_display(agents: &[AgentInfo]) -> (AgentStatus, Vec<String>) {
    let ws = std::env::var("HERDR_WORKSPACE_ID").ok();
    let focused = agents
        .iter()
        .find(|a| a.focused)
        .or_else(|| agents.iter().find(|a| a.workspace_id.as_deref() == ws.as_deref()))
        .or_else(|| agents.first());

    if agents
        .iter()
        .any(|a| matches!(a.status, AgentStatus::Working))
    {
        // Tarefas dos working, focado-working primeiro (sort estável preserva a
        // ordem dos demais). O loop `watch` rotaciona entre elas.
        let mut working: Vec<&AgentInfo> = agents
            .iter()
            .filter(|a| matches!(a.status, AgentStatus::Working))
            .collect();
        if let Some(f) = focused.filter(|f| matches!(f.status, AgentStatus::Working)) {
            working.sort_by_key(|a| !std::ptr::eq(*a, f));
        }
        let titles = working.iter().filter_map(|a| a.title.clone()).collect();
        (AgentStatus::Working, titles)
    } else {
        // Sem working → espelha o focado (tarefa única, se houver).
        let titles = focused.and_then(|f| f.title.clone()).into_iter().collect();
        focused
            .map(|f| (f.status, titles))
            .unwrap_or((AgentStatus::Idle, Vec::new()))
    }
}

/// Snapshot de um `herdr agent list`: display agregado, nº working (ritmo do XP),
/// pares `(pane, seq)` (catch-up / baseline) e panes que estão `working` agora
/// (resumo da sessão). Um único poll resolve tudo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentsSnapshot {
    pub status: AgentStatus,
    pub titles: Vec<String>,
    pub n_working: usize,
    pub pane_seqs: Vec<PaneSeq>,
    pub working_panes: Vec<String>,
    /// Rótulos curtos de quem está `working` (pra o rodapé não ser um número cego).
    pub working_labels: Vec<String>,
}

/// Constrói o snapshot a partir da lista já lida (sem novo `herdr agent list`).
/// `n_working` e o humor saem do **mesmo** filtro: se ninguém está `working`,
/// o pet dorme e o rodapé é `⚙ 0`. Sem working fantasma com mood dormindo.
pub fn snapshot(agents: &[AgentInfo]) -> AgentsSnapshot {
    let working: Vec<&AgentInfo> = agents
        .iter()
        .filter(|a| matches!(a.status, AgentStatus::Working))
        .collect();
    let n_working = working.len();
    let working_panes = working.iter().map(|a| a.pane_id.clone()).collect();
    let working_labels = working.iter().map(|a| working_label(a)).collect();
    let (status, titles) = if n_working > 0 {
        aggregate_display(agents)
    } else {
        // Sem working: espelha o focado (idle/done/blocked). Tarefas de working = vazio.
        let (st, ts) = aggregate_display(agents);
        debug_assert!(!matches!(st, AgentStatus::Working));
        (st, ts)
    };
    let pane_seqs = agents
        .iter()
        .map(|a| PaneSeq {
            pane_id: a.pane_id.clone(),
            seq: a.state_change_seq,
        })
        .collect();
    AgentsSnapshot {
        status,
        titles,
        n_working,
        pane_seqs,
        working_panes,
        working_labels,
    }
}

/// `claude`, `grok`, ou o nome do subagente (descrição curta).
fn working_label(a: &AgentInfo) -> String {
    let kind = a.agent.as_deref().unwrap_or("agent");
    match a.title.as_deref() {
        Some(t) if a.session_id.is_some() && a.pane_id.contains(':') && a.pane_id.matches(':').count() >= 2 => {
            // filho sintético `w16:p5:abc` — a tarefa identifica melhor que "claude"
            let short = t.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
            if short.is_empty() {
                kind.to_string()
            } else {
                short
            }
        }
        _ => kind.to_string(),
    }
}

/// Rodapé: `⚙ 0` ou `⚙ 2 grok, claude`. Sempre o conjunto atual, nunca pico da sessão.
pub fn format_working_badge(n_working: usize, labels: &[String]) -> String {
    if n_working == 0 {
        return "⚙ 0".to_string();
    }
    if labels.is_empty() {
        return format!("⚙ {n_working}");
    }
    format!("⚙ {n_working} {}", labels.join(", "))
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
            {"agent":"claude","agent_status":"working","focused":true,"workspace_id":"w19","state_change_seq":48,"pane_id":"w19:pB","cwd":"/tmp/p","agent_session":{"agent":"claude","kind":"id","source":"herdr:claude","value":"sess-1"}}
        ],"type":"agent_list"}}"#;
        let env: Envelope = serde_json::from_str(raw).unwrap();
        let pick = env.result.agents.iter().find(|a| a.focused).unwrap();
        assert_eq!(AgentStatus::from_herdr(&pick.agent_status), AgentStatus::Working);
        assert_eq!(pick.state_change_seq, 48);
        assert_eq!(pick.pane_id, "w19:pB");
        let info = entry_to_info(pick);
        assert_eq!(info.agent.as_deref(), Some("claude"));
        assert_eq!(info.session_id.as_deref(), Some("sess-1"));
        assert_eq!(info.cwd.as_deref(), Some("/tmp/p"));
        // ausente → default (0 seq, "" pane_id)
        let grok = env.result.agents.iter().find(|a| !a.focused).unwrap();
        assert_eq!(grok.state_change_seq, 0);
        assert_eq!(grok.pane_id, "");
    }

    // --- aggregate_display: o pet "vê todos" os agentes ---

    fn ai(status: AgentStatus, title: Option<&str>, focused: bool) -> AgentInfo {
        ai_pane(status, title, focused, "", 0)
    }

    fn ai_pane(
        status: AgentStatus,
        title: Option<&str>,
        focused: bool,
        pane_id: &str,
        seq: u64,
    ) -> AgentInfo {
        AgentInfo {
            status,
            title: title.map(|s| s.to_string()),
            state_change_seq: seq,
            pane_id: pane_id.to_string(),
            workspace_id: None,
            focused,
            agent: None,
            cwd: None,
            session_id: None,
        }
    }

    #[test]
    fn aggrega_working_de_outra_sessao_acorda_pet() {
        // Pet dockado no idle (focado); outra sessão working → acorda com a tarefa dela.
        let agents = [
            ai(AgentStatus::Idle, Some("idle task"), true),
            ai(AgentStatus::Working, Some("refatorando auth"), false),
        ];
        let (status, titles) = aggregate_display(&agents);
        assert_eq!(status, AgentStatus::Working);
        assert_eq!(titles, vec!["refatorando auth"]);
    }

    #[test]
    fn aggrega_focado_working_mostra_tarefa_dele() {
        let agents = [ai(AgentStatus::Working, Some("minha task"), true)];
        let (status, titles) = aggregate_display(&agents);
        assert_eq!(status, AgentStatus::Working);
        assert_eq!(titles, vec!["minha task"]);
    }

    #[test]
    fn aggrega_todos_idle_espelha_focado() {
        let agents = [
            ai(AgentStatus::Idle, Some("outra"), false),
            ai(AgentStatus::Idle, Some("focada"), true),
        ];
        let (status, titles) = aggregate_display(&agents);
        assert_eq!(status, AgentStatus::Idle);
        assert_eq!(titles, vec!["focada"]);
    }

    #[test]
    fn aggrega_sem_agentes_devolve_idle() {
        let (status, titles) = aggregate_display(&[]);
        assert_eq!(status, AgentStatus::Idle);
        assert!(titles.is_empty());
    }

    #[test]
    fn aggrega_multiplos_working_focado_primeiro_depois_demais() {
        // 2 working; o focado também é working → lista começa por ele, depois o outro.
        let agents = [
            ai(AgentStatus::Working, Some("outra working"), false),
            ai(AgentStatus::Working, Some("focada working"), true),
        ];
        let (status, titles) = aggregate_display(&agents);
        assert_eq!(status, AgentStatus::Working);
        assert_eq!(titles, vec!["focada working", "outra working"]);
    }

    #[test]
    fn aggrega_multiplos_working_focado_idle_lista_todos() {
        // Pet dockado no idle; 2 outras working → acorda e lista as duas (rotação).
        let agents = [
            ai(AgentStatus::Idle, Some("dormindo"), true),
            ai(AgentStatus::Working, Some("task B"), false),
            ai(AgentStatus::Working, Some("task C"), false),
        ];
        let (status, titles) = aggregate_display(&agents);
        assert_eq!(status, AgentStatus::Working);
        assert_eq!(titles, vec!["task B", "task C"]);
    }

    #[test]
    fn snapshot_agrega_display_e_panes_working() {
        let agents = [
            ai_pane(AgentStatus::Idle, Some("dormindo"), true, "w1:pA", 10),
            ai_pane(AgentStatus::Working, Some("task B"), false, "w2:pB", 20),
            ai_pane(AgentStatus::Working, Some("task C"), false, "w3:pC", 30),
        ];
        let snap = snapshot(&agents);
        assert_eq!(snap.status, AgentStatus::Working);
        assert_eq!(snap.titles, vec!["task B", "task C"]);
        assert_eq!(snap.n_working, 2);
        assert_eq!(snap.working_panes, vec!["w2:pB", "w3:pC"]);
        assert_eq!(snap.pane_seqs.len(), 3);
        assert_eq!(snap.pane_seqs[1].seq, 20);
    }

    #[test]
    fn snapshot_sem_working_e_zero_e_nao_working() {
        let agents = [ai(AgentStatus::Idle, Some("dormindo"), true)];
        let snap = snapshot(&agents);
        assert_eq!(snap.n_working, 0);
        assert_eq!(snap.status, AgentStatus::Idle);
        assert_eq!(format_working_badge(snap.n_working, &snap.working_labels), "⚙ 0");
    }

    #[test]
    fn badge_lista_quem_trabalha() {
        assert_eq!(format_working_badge(0, &[]), "⚙ 0");
        assert_eq!(
            format_working_badge(2, &["grok".into(), "claude".into()]),
            "⚙ 2 grok, claude"
        );
    }
}
