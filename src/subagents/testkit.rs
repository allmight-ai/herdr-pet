//! Ferramentas dos testes de subagentes: pais sintéticos, dirs temporários e
//! troca de HOME/config por processo (com lock — env é global).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::*;

pub static GROK_HOME_LOCK: Mutex<()> = Mutex::new(());

pub static CLAUDE_CFG_LOCK: Mutex<()> = Mutex::new(());

pub fn with_grok_home<T>(home: &Path, f: impl FnOnce() -> T) -> T {
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

pub fn parent_claude() -> AgentInfo {
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

pub fn parent_grok() -> AgentInfo {
    AgentInfo {
        agent: Some("grok".into()),
        pane_id: "w19:pB".into(),
        session_id: None,
        cwd: Some("/home/frederico/projects/herdr-pet".into()),
        ..parent_claude()
    }
}

pub fn tmp_dir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("herdr-pet-sub-{name}"));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

pub fn write(path: &Path, s: &str) {
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let mut f = fs::File::create(path).unwrap();
    f.write_all(s.as_bytes()).unwrap();
}

pub fn with_claude_cfg<T>(root: &Path, f: impl FnOnce() -> T) -> T {
    // Só CLAUDE_CONFIG_DIR — não toca HOME (testes de state.rs leem XDG/HOME).
    let _guard = CLAUDE_CFG_LOCK.lock().expect("CLAUDE_CONFIG_DIR lock");
    let prev = std::env::var("CLAUDE_CONFIG_DIR").ok();
    let prev_extra = std::env::var("HERDR_PET_CLAUDE_ROOTS").ok();
    std::env::set_var("CLAUDE_CONFIG_DIR", root);
    std::env::remove_var("HERDR_PET_CLAUDE_ROOTS");
    let out = f();
    match prev {
        Some(p) => std::env::set_var("CLAUDE_CONFIG_DIR", p),
        None => std::env::remove_var("CLAUDE_CONFIG_DIR"),
    }
    match prev_extra {
        Some(p) => std::env::set_var("HERDR_PET_CLAUDE_ROOTS", p),
        None => std::env::remove_var("HERDR_PET_CLAUDE_ROOTS"),
    }
    out
}

pub fn with_claude_two_roots<T>(a: &Path, b: &Path, f: impl FnOnce() -> T) -> T {
    let _guard = CLAUDE_CFG_LOCK.lock().expect("CLAUDE_CONFIG_DIR lock");
    let prev = std::env::var("CLAUDE_CONFIG_DIR").ok();
    let prev_extra = std::env::var("HERDR_PET_CLAUDE_ROOTS").ok();
    std::env::set_var("CLAUDE_CONFIG_DIR", a);
    std::env::set_var("HERDR_PET_CLAUDE_ROOTS", b);
    let out = f();
    match prev {
        Some(p) => std::env::set_var("CLAUDE_CONFIG_DIR", p),
        None => std::env::remove_var("CLAUDE_CONFIG_DIR"),
    }
    match prev_extra {
        Some(p) => std::env::set_var("HERDR_PET_CLAUDE_ROOTS", p),
        None => std::env::remove_var("HERDR_PET_CLAUDE_ROOTS"),
    }
    out
}

pub fn parent_glm(cwd: &str, pane: &str) -> AgentInfo {
    AgentInfo {
        agent: Some("claude".into()),
        pane_id: pane.into(),
        session_id: None,
        cwd: Some(cwd.into()),
        ..parent_claude()
    }
}

/// `{root}/projects/<enc>/<sid>.jsonl` (+ filho em `<sid>/subagents/`).
/// `root` vira `CLAUDE_CONFIG_DIR` — mesmo layout de `~/.claude`.
pub fn write_claude_session(
    root: &Path,
    cwd: &str,
    sid: &str,
    kid: Option<(&str, bool)>,
    age_secs: u64,
) {
    let proj = root.join("projects").join(encode_claude_project(cwd));
    let jsonl = proj.join(format!("{sid}.jsonl"));
    write(&jsonl, "{\"type\":\"system\"}\n");
    if age_secs > 0 {
        let t = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(age_secs))
            .unwrap();
        fs::File::open(&jsonl).unwrap().set_modified(t).unwrap();
    }
    if let Some((id, running)) = kid {
        let sub = proj.join(sid).join("subagents");
        write(
            &sub.join(format!("agent-{id}.meta.json")),
            &format!(r#"{{"agentType":"explore","description":"{id}"}}"#),
        );
        let body = if running {
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use"}]}}
"#
        } else {
            r#"{"type":"assistant","message":{"stop_reason":"end_turn"}}
"#
        };
        write(&sub.join(format!("agent-{id}.jsonl")), body);
    }
}

pub fn set_age(path: &Path, secs: u64) {
    let t = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(secs))
        .unwrap();
    fs::File::open(path).unwrap().set_modified(t).unwrap();
}

pub fn write_one_kid(home: &Path, cwd: &str, sid: &str, kid: &str, pid: u32) {
    write(
        &home.join("active_sessions.json"),
        &format!(r#"[{{"session_id":"{sid}","pid":{pid},"cwd":"{cwd}"}}]"#),
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
