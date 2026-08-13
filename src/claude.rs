//! Subagentes do Claude Code (e GLM) que o Herdr não lista.
//!
//! O Herdr vê um processo `claude` por pane. Time/Task do Claude Code vive
//! em `~/.claude/projects/<cwd>/<session>/subagents/` — o pet lê isso pra
//! contar quem ainda está rodando e mostrar a tarefa no display.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::agent::{AgentInfo, AgentStatus};

#[derive(Debug, Deserialize)]
struct SubagentMeta {
    #[serde(default)]
    #[serde(rename = "agentType")]
    agent_type: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

/// Expande a lista do Herdr com subagentes Claude/GLM que ainda estão rodando.
pub fn expand_subagents(agents: &[AgentInfo]) -> Vec<AgentInfo> {
    let mut extra = Vec::new();
    for a in agents {
        if !is_claude_family(a) {
            continue;
        }
        extra.extend(running_under(a));
    }
    if extra.is_empty() {
        return agents.to_vec();
    }
    let mut out = agents.to_vec();
    out.extend(extra);
    out
}

fn is_claude_family(a: &AgentInfo) -> bool {
    matches!(a.agent.as_deref(), Some("claude") | Some("glm"))
}

fn running_under(parent: &AgentInfo) -> Vec<AgentInfo> {
    let Some(session) = parent.session_id.as_deref() else {
        return Vec::new();
    };
    let Some(cwd) = parent.cwd.as_deref() else {
        return Vec::new();
    };
    for root in claude_roots(parent) {
        let dir = root
            .join("projects")
            .join(encode_project(cwd))
            .join(session)
            .join("subagents");
        if dir.is_dir() {
            return collect_running(&dir, parent);
        }
    }
    Vec::new()
}

fn claude_roots(parent: &AgentInfo) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(explicit) = std::env::var("CLAUDE_CONFIG_DIR") {
        dirs.push(PathBuf::from(explicit));
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        match parent.agent.as_deref() {
            Some("glm") => {
                dirs.push(home.join(".claude-glm"));
                dirs.push(home.join(".claude"));
            }
            _ => {
                dirs.push(home.join(".claude"));
                dirs.push(home.join(".claude-glm"));
            }
        }
    }
    dirs
}

/// `/home/foo/bar` → `-home-foo-bar` (convenção do Claude Code).
pub fn encode_project(cwd: &str) -> String {
    cwd.trim_end_matches('/').replace('/', "-")
}

fn collect_running(dir: &Path, parent: &AgentInfo) -> Vec<AgentInfo> {
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
        if !subagent_still_running(&jsonl) {
            continue;
        }
        let title = read_title(&path);
        out.push(AgentInfo {
            status: AgentStatus::Working,
            title,
            state_change_seq: 0,
            pane_id: format!("{}:{id}", parent.pane_id),
            workspace_id: parent.workspace_id.clone(),
            focused: false,
            agent: parent.agent.clone(),
            cwd: parent.cwd.clone(),
            session_id: Some(id.to_string()),
        });
    }
    out.sort_by(|a, b| a.pane_id.cmp(&b.pane_id));
    out
}

fn read_title(meta_path: &Path) -> Option<String> {
    let data = fs::read(meta_path).ok()?;
    let meta: SubagentMeta = serde_json::from_slice(&data).ok()?;
    meta.description
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| meta.agent_type)
}

/// Sem `end_turn` no último turno do assistente → ainda trabalhando
/// (jsonl ausente = recém-spawnado).
pub fn subagent_still_running(jsonl: &Path) -> bool {
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
    let finished = v.get("type").and_then(|t| t.as_str()) == Some("assistant")
        && matches!(
            v.pointer("/message/stop_reason").and_then(|s| s.as_str()),
            Some("end_turn") | Some("stop_sequence")
        );
    !finished
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn parent() -> AgentInfo {
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

    fn tmp_dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("herdr-pet-subagent-{name}"));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn write(path: &Path, s: &str) {
        let mut f = fs::File::create(path).unwrap();
        f.write_all(s.as_bytes()).unwrap();
    }

    #[test]
    fn encode_project_troca_barra() {
        assert_eq!(
            encode_project("/home/frederico/projects/gerador-json-correios"),
            "-home-frederico-projects-gerador-json-correios"
        );
        assert_eq!(encode_project("/tmp/foo/"), "-tmp-foo");
    }

    #[test]
    fn jsonl_end_turn_nao_esta_rodando() {
        let dir = tmp_dir("done");
        let jsonl = dir.join("a.jsonl");
        write(
            &jsonl,
            r#"{"type":"assistant","message":{"stop_reason":"end_turn","content":[{"type":"text"}]}}
"#,
        );
        assert!(!subagent_still_running(&jsonl));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn jsonl_tool_use_esta_rodando() {
        let dir = tmp_dir("run");
        let jsonl = dir.join("a.jsonl");
        write(
            &jsonl,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read"}]}}
"#,
        );
        assert!(subagent_still_running(&jsonl));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn jsonl_ausente_conta_como_recem_spawn() {
        let dir = tmp_dir("miss");
        assert!(subagent_still_running(&dir.join("nope.jsonl")));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn collect_so_os_que_ainda_trabalham() {
        let dir = tmp_dir("mix");
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

        let got = collect_running(&dir, &parent());
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].title.as_deref(), Some("Reescrever copy do carrossel"));
        assert_eq!(got[0].status, AgentStatus::Working);
        assert_eq!(got[0].pane_id, "w16:p5:aaa");
        assert!(!got[0].focused);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn expand_ignora_grok() {
        let grok = AgentInfo {
            agent: Some("grok".into()),
            session_id: Some("x".into()),
            cwd: Some("/tmp".into()),
            ..parent()
        };
        let out = expand_subagents(&[grok.clone()]);
        assert_eq!(out, vec![grok]);
    }
}
