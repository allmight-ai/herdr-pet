//! Descoberta dos filhos do Grok: `~/.grok/active_sessions.json` +
//! `sessions/<enc-cwd>/<sid>/subagents/<id>/meta.json`.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;

use super::{child, file_older_than, norm_cwd, CHILD_STALE_SECS};
use crate::agent::{AgentInfo, AgentStatus};

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
/// 2. Pid informado e **morto** → sessão stale, descarta. Não é "sem chave":
///    `active_sessions.json` sobrevive a SIGKILL e o `meta.json` fica
///    `running` pra sempre; fallback por cwd adotaria o fantasma.
/// 3. `HERDR_PANE_ID` do pid vivo (`/proc/{pid}/environ` no Linux; `ps eww -p`
///    no macOS). Cache por `session_id` (pid recicla). Se o pane **resolveu**
///    e não casa com nenhum grok da lista → **descarta** a sessão.
/// 4. Sem chave (pid 0 / vivo sem `HERDR_PANE_ID`): fallback por cwd **só**
///    se existe exatamente 1 grok working naquele cwd e nenhum idle.
///    Ambíguo → sem filho.
///
/// Se (1) ou (2) amarram a um pai **idle**, os filhos não vazam.
pub(super) fn grok_children(agents: &[AgentInfo]) -> Vec<AgentInfo> {
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
    if session.pid != 0 && !crate::proc::pid_alive(session.pid).unwrap_or(false) {
        return None;
    }
    if let Some(p) = grok
        .iter()
        .find(|a| a.session_id.as_deref() == Some(session.session_id.as_str()))
    {
        return Some(*p);
    }
    // Pane resolveu → casa ou descarta. Não cai no cwd (pid reciclado).
    if let Some(pane) = herdr_pane_id_for_session(&session.session_id, session.pid) {
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
    grok_debug_skip(
        &session.session_id,
        &format!(
            "cwd ambíguo ({} grok, {} working) — filhos não atribuídos",
            same_cwd,
            working_here.len()
        ),
    );
    None
}

/// Falso-negativo do fallback Grok (vários panes no cwd). Só fala com
/// `HERDR_PET_DEBUG_SUBAGENTS=1` — o poll é a cada ~2 s.
fn grok_debug_skip(session_id: &str, reason: &str) {
    if std::env::var_os("HERDR_PET_DEBUG_SUBAGENTS").is_some() {
        eprintln!("herdr-pet: skip grok session {session_id}: {reason}");
    }
}

/// `session_id` → `HERDR_PANE_ID` (ou ausência). Chave é a sessão, não o pid:
/// pid recicla (grok A morre, grok B nasce com o mesmo número) e o cache
/// por pid devolveria o pane velho. `session_id` do `GrokActive` é estável.
/// Acerto e `None` cacheiam pra sempre — env daquela sessão não muda;
/// recachear `None` reforkaria `ps` a cada poll no macOS.
static PANE_BY_SESSION: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();

fn pane_cache() -> std::sync::MutexGuard<'static, HashMap<String, Option<String>>> {
    PANE_BY_SESSION
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// `HERDR_PANE_ID` do processo Grok (Herdr injeta no PTY).
/// Linux: `/proc/{pid}/environ`. macOS (e fallback): `ps eww -p <pid>`.
fn herdr_pane_id_for_session(session_id: &str, pid: u32) -> Option<String> {
    if pid == 0 || session_id.is_empty() {
        return None;
    }
    if let Some(hit) = pane_cache().get(session_id) {
        return hit.clone();
    }
    let found = herdr_pane_id_from_proc(pid).or_else(|| herdr_pane_id_from_ps(pid));
    pane_cache().insert(session_id.to_string(), found.clone());
    found
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
        if !grok_meta_running(&meta, &meta_path) {
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

/// Terminou se tem `completed_at` ou status terminal. Sem status e sem
/// `completed_at` o recorte puro devolve `true` — o staleness do `meta.json`
/// vive em `grok_meta_running` (esta fn não vê o path).
pub fn grok_still_running(status: &str, completed_at: Option<&str>) -> bool {
    if completed_at.is_some() {
        return false;
    }
    !matches!(
        status,
        "completed" | "failed" | "cancelled" | "canceled" | "error" | "stopped"
    )
}

/// `status: running` (etc.) sem `completed_at` = vivo — o Grok não reescreve o
/// meta a cada turno, então mtime velho **não** mata um `running` explícito.
/// Status ausente: só vivo se o `meta.json` ainda está fresco (spawn recente
/// que ainda não gravou o campo). Sem isso, `("", None)` era fantasma eterno.
fn grok_meta_running(meta: &GrokMeta, meta_path: &Path) -> bool {
    if !grok_still_running(
        meta.status.as_deref().unwrap_or(""),
        meta.completed_at.as_deref(),
    ) {
        return false;
    }
    if meta.status.as_deref().unwrap_or("").is_empty() {
        return !file_older_than(meta_path, CHILD_STALE_SECS);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subagents::testkit::*;

    #[test]
    fn encode_grok_percent() {
        assert_eq!(
            encode_grok_cwd("/home/frederico/projects/herdr-pet"),
            "%2Fhome%2Ffrederico%2Fprojects%2Fherdr-pet"
        );
    }

    #[test]
    fn grok_status_completed_nao_roda() {
        assert!(!grok_still_running(
            "completed",
            Some("2026-08-13T00:00:00Z")
        ));
        assert!(!grok_still_running("completed", None));
        assert!(grok_still_running("running", None));
        // Sem path: recorte puro. Staleness do meta vazio está em grok_meta_running.
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
    fn grok_meta_sem_status_fresco_conta_como_vivo() {
        let dir = tmp_dir("g-empty-fresh");
        write(
            &dir.join("sid/meta.json"),
            r#"{"subagent_id":"sid","description":"nascendo"}"#,
        );
        let got = collect_grok(&dir, &parent_grok());
        assert_eq!(got.len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn grok_meta_sem_status_stale_esta_morto() {
        let dir = tmp_dir("g-empty-stale");
        let meta = dir.join("sid/meta.json");
        write(&meta, r#"{"subagent_id":"sid","description":"abandonado"}"#);
        set_age(&meta, CHILD_STALE_SECS + 60);
        let got = collect_grok(&dir, &parent_grok());
        assert!(got.is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn grok_meta_running_explicito_stale_ainda_vive() {
        // Grok não reescreve meta a cada turno — mtime velho + status running = vivo.
        let dir = tmp_dir("g-run-stale");
        let meta = dir.join("sid/meta.json");
        write(
            &meta,
            r#"{"subagent_id":"sid","description":"longo","status":"running"}"#,
        );
        set_age(&meta, CHILD_STALE_SECS + 60);
        let got = collect_grok(&dir, &parent_grok());
        assert_eq!(got.len(), 1);
        let _ = fs::remove_dir_all(dir);
    }
}
