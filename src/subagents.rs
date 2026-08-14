//! Subagentes que o Herdr não lista — um processo pai, N filhos internos.
//!
//! Cada backend descobre os filhos do **seu** pai (Claude Code via
//! `projects/<cwd>/<session>/subagents/`; Grok via `active_sessions.json` +
//! `sessions/<cwd>/<id>/subagents/`, amarrado por session_id ou
//! `HERDR_PANE_ID` do pid — não pelo cwd sozinho). O pet não filtra por
//! marca: se o pai tem sessão no disco, os filhos ativos entram no `⚙ N`
//! e no XP.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::agent::{AgentInfo, AgentStatus};

/// Expande a lista do Herdr com todo subagente ainda rodando, de qualquer pai.
/// Filho só conta se o pai está `working` — senão o jsonl morto vira fantasma no `⚙ N`.
/// Cada filho entra no máximo uma vez (dedupe por `session_id` / id do subagente).
pub fn expand(agents: &[AgentInfo]) -> Vec<AgentInfo> {
    let mut extra = Vec::new();
    for a in agents {
        if !matches!(a.status, AgentStatus::Working) {
            continue;
        }
        extra.extend(claude_running_under(a));
    }
    extra.extend(grok_children(agents));
    extra = dedupe_children(extra);
    if extra.is_empty() {
        return agents.to_vec();
    }
    let mut out = agents.to_vec();
    out.extend(extra);
    out
}

/// Um filho = uma entrada. Chave: `session_id` do child (id do subagente);
/// se faltar, o `pane_id` sintético. Atribui ao primeiro pai que o descobriu.
fn dedupe_children(children: Vec<AgentInfo>) -> Vec<AgentInfo> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(children.len());
    for c in children {
        let key = c
            .session_id
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| c.pane_id.clone());
        if seen.insert(key) {
            out.push(c);
        }
    }
    out
}

// --- Claude Code / GLM / qualquer sessão no layout `projects/<enc>/<sid>/subagents` ---

#[derive(Debug, Deserialize)]
struct ClaudeMeta {
    #[serde(default)]
    #[serde(rename = "agentType")]
    agent_type: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

fn claude_running_under(parent: &AgentInfo) -> Vec<AgentInfo> {
    let Some(session) = parent.session_id.as_deref() else {
        return Vec::new();
    };
    let Some(cwd) = parent.cwd.as_deref() else {
        return Vec::new();
    };
    for root in claude_roots() {
        let dir = root
            .join("projects")
            .join(encode_claude_project(cwd))
            .join(session)
            .join("subagents");
        if dir.is_dir() {
            return collect_claude(&dir, parent);
        }
    }
    Vec::new()
}

fn claude_roots() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(explicit) = std::env::var("CLAUDE_CONFIG_DIR") {
        dirs.push(PathBuf::from(explicit));
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join(".claude"));
        dirs.push(home.join(".claude-glm"));
    }
    dirs
}

/// `/home/foo/bar` → `-home-foo-bar` (convenção do Claude Code).
pub fn encode_claude_project(cwd: &str) -> String {
    cwd.trim_end_matches('/').replace('/', "-")
}

fn collect_claude(dir: &Path, parent: &AgentInfo) -> Vec<AgentInfo> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) if n.starts_with("agent-") && n.ends_with(".meta.json") => n,
            _ => continue,
        };
        let id = name
            .trim_start_matches("agent-")
            .trim_end_matches(".meta.json");
        let jsonl = path.with_file_name(format!("agent-{id}.jsonl"));
        if !claude_still_running(&jsonl) {
            continue;
        }
        let title = read_claude_title(&path);
        out.push(child(parent, id, title));
    }
    out.sort_by(|a, b| a.pane_id.cmp(&b.pane_id));
    out
}

fn read_claude_title(meta_path: &Path) -> Option<String> {
    let data = fs::read(meta_path).ok()?;
    let meta: ClaudeMeta = serde_json::from_slice(&data).ok()?;
    meta.description
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| meta.agent_type)
}

/// Ainda trabalhando? jsonl ausente = recém-spawnado. Terminou se o último
/// assistente tem `end_turn` **ou** só texto (relatório final sem stop_reason —
/// o Claude às vezes fecha assim e virava fantasma no `⚙ N`).
pub fn claude_still_running(jsonl: &Path) -> bool {
    if !jsonl.exists() {
        return true;
    }
    let Ok(data) = fs::read_to_string(jsonl) else {
        return false;
    };
    let Some(last) = data.lines().rev().find(|l| !l.trim().is_empty()) else {
        return true;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(last) else {
        return true;
    };
    if v.get("type").and_then(|t| t.as_str()) != Some("assistant") {
        return true;
    }
    if matches!(
        v.pointer("/message/stop_reason").and_then(|s| s.as_str()),
        Some("end_turn") | Some("stop_sequence")
    ) {
        return false;
    }
    let types: Vec<&str> = v
        .pointer("/message/content")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.get("type").and_then(|t| t.as_str()))
                .collect()
        })
        .unwrap_or_default();
    if types.iter().any(|t| *t == "tool_use") {
        return true;
    }
    // Só texto (sem tool_use) = resposta final, mesmo sem stop_reason.
    if types.iter().any(|t| *t == "text") {
        return false;
    }
    true
}

// --- Grok: `~/.grok/active_sessions.json` + `sessions/<enc-cwd>/<sid>/subagents/<id>/meta.json` ---

#[derive(Debug, Deserialize)]
struct GrokActive {
    session_id: String,
    #[serde(default)]
    pid: u32,
    #[serde(default)]
    cwd: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GrokMeta {
    #[serde(default)]
    subagent_id: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    subagent_type: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    completed_at: Option<String>,
}

/// Filhos Grok, cada um atribuído a **no máximo um** pai `working`.
///
/// Estratégia (em ordem) — `active_sessions.json` traz `session_id` + `pid`;
/// o Herdr hoje não manda `agent_session` nos panes grok:
///
/// 1. `parent.session_id` == entrada ativa — se o Herdr passar a sessão.
/// 2. `HERDR_PANE_ID` do pid (`/proc/{pid}/environ` no Linux; `ps eww -p`
///    no macOS, o manifesto declara as duas plataformas). Se o pane
///    **resolveu** e não casa com nenhum grok da lista → **descarta** a
///    sessão (pid reciclado não escorrega pro vizinho via cwd).
/// 3. Sem chave (pid morto / sem env): fallback por cwd **só** se existe
///    exatamente 1 grok working naquele cwd e nenhum grok idle no mesmo
///    cwd. Ambíguo (2 working, ou 1 working + idle) → sem filho, não
///    inventa dono.
///
/// Se (1) ou (2) amarram a um pai **idle**, os filhos não vazam.
fn grok_children(agents: &[AgentInfo]) -> Vec<AgentInfo> {
    let grok: Vec<&AgentInfo> = agents
        .iter()
        .filter(|a| a.agent.as_deref() == Some("grok"))
        .collect();
    let working: Vec<&AgentInfo> = grok
        .iter()
        .copied()
        .filter(|a| matches!(a.status, AgentStatus::Working))
        .collect();
    if working.is_empty() {
        return Vec::new();
    }
    let home = grok_home();
    let sessions = match load_grok_active(&home) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let mut out = Vec::new();
    for s in &sessions {
        let Some(parent) = owner_for_grok_session(s, &grok, &working) else {
            continue;
        };
        if !matches!(parent.status, AgentStatus::Working) {
            continue;
        }
        let Some(cwd) = s.cwd.as_deref().or(parent.cwd.as_deref()) else {
            continue;
        };
        let dir = home
            .join("sessions")
            .join(encode_grok_cwd(cwd))
            .join(&s.session_id)
            .join("subagents");
        out.extend(collect_grok(&dir, parent));
    }
    out
}

fn owner_for_grok_session<'a>(
    session: &GrokActive,
    grok: &[&'a AgentInfo],
    working: &[&'a AgentInfo],
) -> Option<&'a AgentInfo> {
    if let Some(p) = grok
        .iter()
        .find(|a| a.session_id.as_deref() == Some(session.session_id.as_str()))
    {
        return Some(*p);
    }
    // Pane resolveu → casa ou descarta. Não cai no cwd (pid reciclado).
    if let Some(pane) = herdr_pane_id_for_pid(session.pid) {
        return grok.iter().find(|a| a.pane_id == pane).copied();
    }
    cwd_unambiguous_working(session, grok, working)
}

/// Fallback sem chave pane↔sessão: só 1 grok working naquele cwd e zero idle.
fn cwd_unambiguous_working<'a>(
    session: &GrokActive,
    grok: &[&'a AgentInfo],
    working: &[&'a AgentInfo],
) -> Option<&'a AgentInfo> {
    let want = session.cwd.as_deref().map(norm_cwd)?;
    let same_cwd = grok
        .iter()
        .copied()
        .filter(|a| a.cwd.as_deref().map(norm_cwd).as_deref() == Some(want.as_str()))
        .count();
    let working_here: Vec<&AgentInfo> = working
        .iter()
        .copied()
        .filter(|a| a.cwd.as_deref().map(norm_cwd).as_deref() == Some(want.as_str()))
        .collect();
    if working_here.len() == 1 && same_cwd == 1 {
        return Some(working_here[0]);
    }
    None
}

/// `HERDR_PANE_ID` do processo Grok (Herdr injeta no PTY).
/// Linux: `/proc/{pid}/environ`. macOS (e fallback): `ps eww -p <pid>`.
/// `None` = pid 0, processo sumiu, ou o env não tem a variável — aí o
/// caller pode tentar o fallback restrito por cwd.
fn herdr_pane_id_for_pid(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    herdr_pane_id_from_proc(pid).or_else(|| herdr_pane_id_from_ps(pid))
}

fn herdr_pane_id_from_proc(pid: u32) -> Option<String> {
    let data = fs::read(format!("/proc/{pid}/environ")).ok()?;
    for pair in data.split(|b| *b == 0) {
        let Some(rest) = pair.strip_prefix(b"HERDR_PANE_ID=") else {
            continue;
        };
        let Ok(s) = std::str::from_utf8(rest) else {
            continue;
        };
        let s = s.trim();
        if !s.is_empty() {
            return Some(s.to_string());
        }
    }
    None
}

fn herdr_pane_id_from_ps(pid: u32) -> Option<String> {
    let out = std::process::Command::new("ps")
        .args(["eww", "-p", &pid.to_string()])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for tok in text.split_whitespace() {
        let Some(v) = tok.strip_prefix("HERDR_PANE_ID=") else {
            continue;
        };
        let v = v.trim();
        if !v.is_empty() {
            return Some(v.to_string());
        }
    }
    None
}

fn grok_home() -> PathBuf {
    if let Ok(h) = std::env::var("GROK_HOME") {
        return PathBuf::from(h);
    }
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".grok"))
        .unwrap_or_else(|_| PathBuf::from(".grok"))
}

fn load_grok_active(home: &Path) -> Option<Vec<GrokActive>> {
    let data = fs::read(home.join("active_sessions.json")).ok()?;
    serde_json::from_slice(&data).ok()
}

fn norm_cwd(cwd: &str) -> String {
    cwd.trim_end_matches('/').to_string()
}

/// `/home/foo/bar` → `%2Fhome%2Ffoo%2Fbar` (URL-encode do Grok).
pub fn encode_grok_cwd(cwd: &str) -> String {
    let mut out = String::new();
    for b in cwd.trim_end_matches('/').bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn collect_grok(dir: &Path, parent: &AgentInfo) -> Vec<AgentInfo> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let meta_path = entry.path().join("meta.json");
        if !meta_path.is_file() {
            continue;
        }
        let Ok(data) = fs::read(&meta_path) else {
            continue;
        };
        let Ok(meta) = serde_json::from_slice::<GrokMeta>(&data) else {
            continue;
        };
        if !grok_meta_running(&meta) {
            continue;
        }
        let id = meta
            .subagent_id
            .clone()
            .or_else(|| {
                entry
                    .path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(String::from)
            })
            .unwrap_or_else(|| "sub".into());
        let title = meta
            .description
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .or(meta.subagent_type.clone());
        out.push(child(parent, &id, title));
    }
    out.sort_by(|a, b| a.pane_id.cmp(&b.pane_id));
    out
}

/// Terminou se tem `completed_at` ou status terminal. O resto (incl. ausente) = rodando.
pub fn grok_still_running(status: &str, completed_at: Option<&str>) -> bool {
    if completed_at.is_some() {
        return false;
    }
    !matches!(
        status,
        "completed" | "failed" | "cancelled" | "canceled" | "error" | "stopped"
    )
}

fn grok_meta_running(meta: &GrokMeta) -> bool {
    grok_still_running(
        meta.status.as_deref().unwrap_or(""),
        meta.completed_at.as_deref(),
    )
}

fn child(parent: &AgentInfo, id: &str, title: Option<String>) -> AgentInfo {
    AgentInfo {
        status: AgentStatus::Working,
        title,
        state_change_seq: None, // filho sintético: sem seq do Herdr
        pane_id: format!("{}:{id}", parent.pane_id),
        workspace_id: parent.workspace_id.clone(),
        focused: false,
        agent: parent.agent.clone(),
        cwd: parent.cwd.clone(),
        session_id: Some(id.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::Mutex;

    static GROK_HOME_LOCK: Mutex<()> = Mutex::new(());

    fn with_grok_home<T>(home: &Path, f: impl FnOnce() -> T) -> T {
        let _guard = GROK_HOME_LOCK.lock().expect("GROK_HOME lock");
        let prev = std::env::var("GROK_HOME").ok();
        std::env::set_var("GROK_HOME", home);
        let out = f();
        match prev {
            Some(p) => std::env::set_var("GROK_HOME", p),
            None => std::env::remove_var("GROK_HOME"),
        }
        out
    }

    fn parent_claude() -> AgentInfo {
        AgentInfo {
            status: AgentStatus::Working,
            title: Some("main".into()),
            state_change_seq: Some(7),
            pane_id: "w16:p5".into(),
            workspace_id: Some("w16".into()),
            focused: true,
            agent: Some("claude".into()),
            cwd: Some("/tmp/proj".into()),
            session_id: Some("sess".into()),
        }
    }

    fn parent_grok() -> AgentInfo {
        AgentInfo {
            agent: Some("grok".into()),
            pane_id: "w19:pB".into(),
            session_id: None,
            cwd: Some("/home/frederico/projects/herdr-pet".into()),
            ..parent_claude()
        }
    }

    fn tmp_dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("herdr-pet-sub-{name}"));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write(path: &Path, s: &str) {
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let mut f = fs::File::create(path).unwrap();
        f.write_all(s.as_bytes()).unwrap();
    }

    #[test]
    fn encode_claude_troca_barra() {
        assert_eq!(
            encode_claude_project("/home/frederico/projects/gerador-json-correios"),
            "-home-frederico-projects-gerador-json-correios"
        );
        assert_eq!(encode_claude_project("/tmp/foo/"), "-tmp-foo");
    }

    #[test]
    fn encode_grok_percent() {
        assert_eq!(
            encode_grok_cwd("/home/frederico/projects/herdr-pet"),
            "%2Fhome%2Ffrederico%2Fprojects%2Fherdr-pet"
        );
    }

    #[test]
    fn claude_jsonl_end_turn_nao_esta_rodando() {
        let dir = tmp_dir("done");
        let jsonl = dir.join("a.jsonl");
        write(
            &jsonl,
            r#"{"type":"assistant","message":{"stop_reason":"end_turn"}}
"#,
        );
        assert!(!claude_still_running(&jsonl));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn claude_jsonl_texto_final_sem_stop_reason_nao_esta_rodando() {
        // Fantasma real: relatório pronto, stop_reason ausente.
        let dir = tmp_dir("text-done");
        let jsonl = dir.join("a.jsonl");
        write(
            &jsonl,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"pronto"}]}}
"#,
        );
        assert!(!claude_still_running(&jsonl));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn expand_ignora_filho_se_pai_nao_esta_working() {
        let idle = AgentInfo {
            status: AgentStatus::Done,
            ..parent_claude()
        };
        assert_eq!(expand(&[idle.clone()]), vec![idle]);
    }

    #[test]
    fn claude_jsonl_tool_use_esta_rodando() {
        let dir = tmp_dir("run");
        let jsonl = dir.join("a.jsonl");
        write(
            &jsonl,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use"}]}}
"#,
        );
        assert!(claude_still_running(&jsonl));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn collect_claude_so_os_que_ainda_trabalham() {
        let dir = tmp_dir("cmix");
        write(
            &dir.join("agent-aaa.meta.json"),
            r#"{"agentType":"general-purpose","description":"Reescrever copy do carrossel"}"#,
        );
        write(
            &dir.join("agent-aaa.jsonl"),
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use"}]}}
"#,
        );
        write(
            &dir.join("agent-bbb.meta.json"),
            r#"{"agentType":"explore","description":"já acabou"}"#,
        );
        write(
            &dir.join("agent-bbb.jsonl"),
            r#"{"type":"assistant","message":{"stop_reason":"end_turn"}}
"#,
        );
        let got = collect_claude(&dir, &parent_claude());
        assert_eq!(got.len(), 1);
        assert_eq!(
            got[0].title.as_deref(),
            Some("Reescrever copy do carrossel")
        );
        assert_eq!(got[0].pane_id, "w16:p5:aaa");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn grok_status_completed_nao_roda() {
        assert!(!grok_still_running(
            "completed",
            Some("2026-08-13T00:00:00Z")
        ));
        assert!(!grok_still_running("completed", None));
        assert!(grok_still_running("running", None));
        assert!(grok_still_running("", None));
        assert!(!grok_still_running("failed", None));
    }

    #[test]
    fn collect_grok_so_os_ativos() {
        let dir = tmp_dir("gmix");
        write(
            &dir.join("sid-run/meta.json"),
            r#"{"subagent_id":"sid-run","description":"Revisar o status","subagent_type":"explore","status":"running"}"#,
        );
        write(
            &dir.join("sid-done/meta.json"),
            r#"{"subagent_id":"sid-done","description":"já foi","status":"completed","completed_at":"2026-08-13T00:00:00Z"}"#,
        );
        let got = collect_grok(&dir, &parent_grok());
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].title.as_deref(), Some("Revisar o status"));
        assert_eq!(got[0].pane_id, "w19:pB:sid-run");
        assert_eq!(got[0].status, AgentStatus::Working);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn expand_nao_inventa_filho_sem_disco() {
        let home = tmp_dir("g-empty");
        write(&home.join("active_sessions.json"), "[]");
        let grok = parent_grok();
        let out = with_grok_home(&home, || expand(&[grok.clone()]));
        assert_eq!(out, vec![grok]);
        let _ = fs::remove_dir_all(home);
    }

    fn write_one_kid(home: &Path, cwd: &str, sid: &str, kid: &str) {
        write(
            &home.join("active_sessions.json"),
            &format!(r#"[{{"session_id":"{sid}","pid":4294967294,"cwd":"{cwd}"}}]"#),
        );
        write(
            &home
                .join("sessions")
                .join(encode_grok_cwd(cwd))
                .join(sid)
                .join("subagents")
                .join(kid)
                .join("meta.json"),
            &format!(r#"{{"subagent_id":"{kid}","description":"um filho","status":"running"}}"#),
        );
    }

    #[test]
    fn expand_dois_pais_grok_mesmo_cwd_um_filho_conta_um() {
        // Sem bind pane↔sessão (pid fake), 2 working no mesmo cwd é ambíguo:
        // fallback recusa. Filho conta 0×, nunca 2×.
        let home = tmp_dir("g-fanout");
        let cwd = "/tmp/proj-fanout";
        write_one_kid(&home, cwd, "sess-a", "kid");
        let a = AgentInfo {
            pane_id: "w1:p1".into(),
            cwd: Some(cwd.into()),
            session_id: None,
            ..parent_grok()
        };
        let b = AgentInfo {
            pane_id: "w1:p2".into(),
            cwd: Some(cwd.into()),
            session_id: None,
            ..parent_grok()
        };
        let out = with_grok_home(&home, || expand(&[a.clone(), b.clone()]));
        let kids: Vec<_> = out
            .iter()
            .filter(|x| x.session_id.as_deref() == Some("kid"))
            .collect();
        assert_eq!(kids.len(), 0, "ambíguo: não inventa dono (e não clona)");
        assert_eq!(out.len(), 2);
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn expand_um_grok_working_cwd_unico_pega_filho() {
        // Fallback por cwd só quando é o único grok daquele cwd.
        let home = tmp_dir("g-one");
        let cwd = "/tmp/proj-one";
        write_one_kid(&home, cwd, "sess-a", "kid");
        let a = AgentInfo {
            pane_id: "w1:p1".into(),
            cwd: Some(cwd.into()),
            session_id: None,
            ..parent_grok()
        };
        let out = with_grok_home(&home, || expand(&[a.clone()]));
        let kids: Vec<_> = out
            .iter()
            .filter(|x| x.session_id.as_deref() == Some("kid"))
            .collect();
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].pane_id, "w1:p1:kid");
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn expand_working_mais_idle_mesmo_cwd_sem_bind_nao_chuta() {
        let home = tmp_dir("g-mix");
        let cwd = "/tmp/proj-mix";
        write_one_kid(&home, cwd, "sess-a", "kid");
        let idle = AgentInfo {
            status: AgentStatus::Idle,
            pane_id: "w1:pIdle".into(),
            cwd: Some(cwd.into()),
            session_id: None,
            ..parent_grok()
        };
        let working = AgentInfo {
            pane_id: "w1:pWork".into(),
            cwd: Some(cwd.into()),
            session_id: None,
            ..parent_grok()
        };
        let out = with_grok_home(&home, || expand(&[idle.clone(), working.clone()]));
        assert!(
            out.iter().all(|a| a.session_id.as_deref() != Some("kid")),
            "idle no cwd bloqueia o fallback"
        );
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn grok_filho_de_pai_idle_nao_vaza_pro_vizinho_quando_amarrado() {
        // (1) session_id do pai idle bate: filho não entra no working vizinho.
        let home = tmp_dir("g-idle-bind");
        let cwd = "/tmp/proj-idle-bind";
        write(
            &home.join("active_sessions.json"),
            r#"[{"session_id":"sess-idle","pid":4294967293,"cwd":"/tmp/proj-idle-bind"}]"#,
        );
        write(
            &home
                .join("sessions")
                .join(encode_grok_cwd(cwd))
                .join("sess-idle")
                .join("subagents")
                .join("ghost")
                .join("meta.json"),
            r#"{"subagent_id":"ghost","description":"do idle","status":"running"}"#,
        );
        let idle = AgentInfo {
            status: AgentStatus::Idle,
            pane_id: "w1:pIdle".into(),
            cwd: Some(cwd.into()),
            session_id: Some("sess-idle".into()),
            ..parent_grok()
        };
        let working = AgentInfo {
            pane_id: "w1:pWork".into(),
            cwd: Some(cwd.into()),
            session_id: None,
            ..parent_grok()
        };
        let out = with_grok_home(&home, || expand(&[idle.clone(), working.clone()]));
        assert!(
            out.iter().all(|a| a.session_id.as_deref() != Some("ghost")),
            "filho do idle não vaza"
        );
        assert_eq!(out, vec![idle, working]);
        let _ = fs::remove_dir_all(home);
    }
}
