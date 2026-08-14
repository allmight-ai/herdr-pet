//! Subagentes que o Herdr não lista — um processo pai, N filhos internos.
//!
//! Cada backend descobre os filhos do **seu** pai (Claude Code via
//! `projects/<cwd>/<session>/subagents/`; Grok via `active_sessions.json` +
//! `sessions/<cwd>/<id>/subagents/`). O pet não filtra por marca: se o pai
//! tem sessão no disco, os filhos ativos entram no `⚙ N` e no XP.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::agent::{AgentInfo, AgentStatus};

/// Expande a lista do Herdr com todo subagente ainda rodando, de qualquer pai.
/// Filho só conta se o pai está `working` — senão o jsonl morto vira fantasma no `⚙ N`.
pub fn expand(agents: &[AgentInfo]) -> Vec<AgentInfo> {
    let mut extra = Vec::new();
    for a in agents {
        if !matches!(a.status, AgentStatus::Working) {
            continue;
        }
        extra.extend(claude_running_under(a));
        extra.extend(grok_running_under(a));
    }
    if extra.is_empty() {
        return agents.to_vec();
    }
    let mut out = agents.to_vec();
    out.extend(extra);
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

fn grok_running_under(parent: &AgentInfo) -> Vec<AgentInfo> {
    if parent.agent.as_deref() != Some("grok") {
        return Vec::new();
    }
    let Some(cwd) = parent.cwd.as_deref() else {
        return Vec::new();
    };
    let home = grok_home();
    let sessions = match load_grok_active(&home) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let want = norm_cwd(cwd);
    let mut out = Vec::new();
    for s in sessions {
        if s.cwd.as_deref().map(norm_cwd).as_deref() != Some(want.as_str()) {
            continue;
        }
        let dir = home
            .join("sessions")
            .join(encode_grok_cwd(cwd))
            .join(&s.session_id)
            .join("subagents");
        out.extend(collect_grok(&dir, parent));
    }
    out
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
        state_change_seq: 0,
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

    fn parent_claude() -> AgentInfo {
        AgentInfo {
            status: AgentStatus::Working,
            title: Some("main".into()),
            state_change_seq: 7,
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
        assert_eq!(got[0].title.as_deref(), Some("Reescrever copy do carrossel"));
        assert_eq!(got[0].pane_id, "w16:p5:aaa");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn grok_status_completed_nao_roda() {
        assert!(!grok_still_running("completed", Some("2026-08-13T00:00:00Z")));
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
        let grok = parent_grok();
        let out = expand(&[grok.clone()]);
        assert_eq!(out, vec![grok]);
    }
}
