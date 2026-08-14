//! Subagentes que o Herdr não lista — um processo pai, N filhos internos.
//!
//! Cada backend descobre os filhos do **seu** pai (Claude Code via
//! `projects/<cwd>/<session>/subagents/`; Grok via `active_sessions.json` +
//! `sessions/<cwd>/<id>/subagents/`, amarrado por session_id ou
//! `HERDR_PANE_ID` do pid — não pelo cwd sozinho). O pet não filtra por
//! marca: se o pai tem sessão no disco, os filhos ativos entram no `⚙ N`
//! e no XP.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;

use crate::agent::{AgentInfo, AgentStatus};

/// Expande a lista do Herdr com todo subagente ainda rodando, de qualquer pai.
/// Filho só conta se o pai está `working` — senão o jsonl morto vira fantasma no `⚙ N`.
/// Cada filho entra no máximo uma vez (dedupe por `session_id` / id do subagente).
pub fn expand(agents: &[AgentInfo]) -> Vec<AgentInfo> {
    let mut extra = Vec::new();
    extra.extend(claude_children(agents));
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

/// Claude Code **e** GLM (Herdr rotula os dois como `claude`; `glm` se um dia vier).
/// Só o **disparo** do fallback sem `session_id` usa isto (quem tenta adotar).
fn claude_kind(a: &AgentInfo) -> bool {
    matches!(a.agent.as_deref(), Some("claude") | Some("glm"))
}

/// Pode ser dono de sessão no layout `projects/<enc>/<sid>/`. Na dúvida, sim:
/// `None`/`unknown`/outra marca entram. Só o Grok tem backend próprio e fica de fora.
fn may_own_claude_layout(a: &AgentInfo) -> bool {
    !matches!(a.agent.as_deref(), Some("grok"))
}

/// Janela do fallback sem `session_id`: o jsonl tem que ter avançado agora.
/// Sessão parada (Claude fora do Herdr, leftover) não é adotada.
const CLAUDE_FALLBACK_FRESH_SECS: u64 = 120;

/// Filhos do layout `projects/<enc>/<sid>/subagents/`.
///
/// 1. **Com `session_id`** (qualquer `agent`, inclusive `None`/`unknown`):
///    pasta daquela sessão. Contrato do módulo: o pet não filtra por marca.
/// 2. **Sem `session_id`** (GLM hoje — o Herdr não manda `agent_session`):
///    fallback **só** se o pai é o único possível dono de sessão
///    claude-layout naquele cwd (`may_own_claude_layout`: qualquer pane
///    que não seja grok, qualquer status); **e** existe exatamente uma
///    sessão cujo jsonl avançou nos últimos `CLAUDE_FALLBACK_FRESH_SECS`;
///    **e** ela vive num único root. 2+ candidatos no cwd (incl. `agent:
///    None`), jsonl velho, 2 jsonl recentes, ou recentes em 2 roots → zero.
///
/// O caminho pleno do GLM depende do Herdr passar `agent_session` (follow-up
/// upstream). Enquanto isso o fallback é **inerte** com vários claude-kind
/// no mesmo cwd (3 panes neste hunt) — é a disciplina, não um furo pra
/// afrouxar. Residual: um Claude/GLM **fora** do Herdr no mesmo cwd, se for
/// o único writer recente, ainda pode ser adotado — unicidade só vê o que
/// o Herdr lista; recência não prova dono. Sem essa garantia, recusamos.
fn claude_children(agents: &[AgentInfo]) -> Vec<AgentInfo> {
    let mut out = Vec::new();
    for parent in agents
        .iter()
        .filter(|a| matches!(a.status, AgentStatus::Working))
    {
        if parent.session_id.as_deref().is_some_and(|s| !s.is_empty()) {
            out.extend(claude_running_under(parent));
            continue;
        }
        if claude_kind(parent) {
            out.extend(claude_fallback_newest_if_unique(parent, agents));
        }
    }
    out
}

fn claude_running_under(parent: &AgentInfo) -> Vec<AgentInfo> {
    let Some(session) = parent.session_id.as_deref().filter(|s| !s.is_empty()) else {
        return Vec::new();
    };
    let Some(cwd) = parent.cwd.as_deref() else {
        return Vec::new();
    };
    collect_claude_session(cwd, session, parent)
}

/// Sem session_id: único possível dono de sessão claude-layout no cwd +
/// exatamente um jsonl recente num único root. Senão recusa.
fn claude_fallback_newest_if_unique(parent: &AgentInfo, agents: &[AgentInfo]) -> Vec<AgentInfo> {
    let Some(cwd) = parent.cwd.as_deref() else {
        return Vec::new();
    };
    let want = norm_cwd(cwd);
    let same = agents
        .iter()
        .filter(|a| {
            may_own_claude_layout(a)
                && a.cwd.as_deref().map(norm_cwd).as_deref() == Some(want.as_str())
        })
        .count();
    if same != 1 {
        return Vec::new();
    }
    let Some((root, sid)) = unique_recent_claude_session(cwd) else {
        return Vec::new();
    };
    collect_claude_session_in(&root, cwd, &sid, parent)
}

fn collect_claude_session(cwd: &str, session: &str, parent: &AgentInfo) -> Vec<AgentInfo> {
    for root in claude_roots() {
        let dir = claude_subagents_dir(&root, cwd, session);
        if dir.is_dir() {
            return collect_claude(&dir, parent);
        }
    }
    Vec::new()
}

fn claude_subagents_dir(root: &Path, cwd: &str, session: &str) -> PathBuf {
    root.join("projects")
        .join(encode_claude_project(cwd))
        .join(session)
        .join("subagents")
}

fn collect_claude_session_in(
    root: &Path,
    cwd: &str,
    session: &str,
    parent: &AgentInfo,
) -> Vec<AgentInfo> {
    let dir = claude_subagents_dir(root, cwd, session);
    if dir.is_dir() {
        collect_claude(&dir, parent)
    } else {
        Vec::new()
    }
}

fn jsonl_is_fresh(mtime: std::time::SystemTime, now: std::time::SystemTime) -> bool {
    match now.duration_since(mtime) {
        Ok(age) => age.as_secs() <= CLAUDE_FALLBACK_FRESH_SECS,
        Err(_) => true, // relógio no futuro: trata como fresco
    }
}

/// Exatamente **uma** sessão recente em **um** root. 0 recentes, 2+ no
/// mesmo root, ou recentes em roots distintos (`~/.claude` + `~/.claude-glm`)
/// → `None`. Não mistura roots na escolha.
fn unique_recent_claude_session(cwd: &str) -> Option<(PathBuf, String)> {
    let enc = encode_claude_project(cwd);
    let now = std::time::SystemTime::now();
    let mut roots_hits: Vec<(PathBuf, Vec<String>)> = Vec::new();
    for root in claude_roots() {
        let proj = root.join("projects").join(&enc);
        let Ok(entries) = fs::read_dir(&proj) else {
            continue;
        };
        let mut sids = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Some(sid) = name.strip_suffix(".jsonl") else {
                continue;
            };
            if sid.is_empty() || sid == "memory" || sid.starts_with("agent-") {
                continue;
            }
            let Ok(mtime) = fs::metadata(&path).and_then(|m| m.modified()) else {
                continue;
            };
            if !jsonl_is_fresh(mtime, now) {
                continue;
            }
            sids.push(sid.to_string());
        }
        if !sids.is_empty() {
            roots_hits.push((root, sids));
        }
    }
    if roots_hits.len() != 1 {
        return None;
    }
    let (root, sids) = roots_hits.pop()?;
    if sids.len() != 1 {
        return None;
    }
    Some((root, sids.into_iter().next()?))
}

fn claude_roots() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(explicit) = std::env::var("CLAUDE_CONFIG_DIR") {
        push_unique_root(&mut dirs, PathBuf::from(explicit));
    }
    // Extra roots só em `cargo test` (dois configs sem mutar HOME). Release ignora.
    #[cfg(test)]
    if let Ok(extra) = std::env::var("HERDR_PET_CLAUDE_ROOTS") {
        for p in extra.split(':') {
            if !p.is_empty() {
                push_unique_root(&mut dirs, PathBuf::from(p));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        push_unique_root(&mut dirs, home.join(".claude"));
        push_unique_root(&mut dirs, home.join(".claude-glm"));
    }
    dirs
}

fn push_unique_root(dirs: &mut Vec<PathBuf>, p: PathBuf) {
    let key = fs::canonicalize(&p).unwrap_or_else(|_| p.clone());
    if dirs
        .iter()
        .any(|d| fs::canonicalize(d).unwrap_or_else(|_| d.clone()) == key)
    {
        return;
    }
    dirs.push(p);
}

/// Convenção do Claude Code: cada char que não é ASCII `[A-Za-z0-9]` vira `-`.
///
/// Evidência em disco (2026-08-14), `~/.claude{,-glm}/projects/<dir>` ↔ `cwd` do jsonl:
/// - `/` → `-`  (`/home/foo/bar` → `-home-foo-bar`)
/// - `.` → `-`  (`/home/frederico/.buzz` → `-home-frederico--buzz`)
/// - `_` → `-`  (`…/MercadoLivre/automacao_ml` → `…-MercadoLivre-automacao-ml`)
/// - `-` permanece `-` (`Correios-OneForAll`). Sem lowercase.
pub fn encode_claude_project(cwd: &str) -> String {
    cwd.trim_end_matches('/')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
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
    if session.pid != 0 && !pid_is_alive(session.pid) {
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

/// Linux: `/proc/<pid>` existe. macOS / sem proc: `kill -0` (sem sinal).
fn pid_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    if Path::new("/proc/self").exists() {
        return Path::new(&format!("/proc/{pid}")).exists();
    }
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
    static CLAUDE_CFG_LOCK: Mutex<()> = Mutex::new(());

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

    fn with_claude_cfg<T>(root: &Path, f: impl FnOnce() -> T) -> T {
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

    fn with_claude_two_roots<T>(a: &Path, b: &Path, f: impl FnOnce() -> T) -> T {
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

    fn parent_glm(cwd: &str, pane: &str) -> AgentInfo {
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
    fn write_claude_session(
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

    #[test]
    fn encode_claude_troca_barra() {
        assert_eq!(
            encode_claude_project("/home/frederico/projects/gerador-json-correios"),
            "-home-frederico-projects-gerador-json-correios"
        );
        assert_eq!(encode_claude_project("/tmp/foo/"), "-tmp-foo");
    }

    #[test]
    fn encode_claude_sanitiza_ponto_underscore_hifen() {
        // Disco: ~/.claude/projects/-home-frederico--buzz ↔ cwd /home/frederico/.buzz
        assert_eq!(
            encode_claude_project("/home/frederico/.buzz"),
            "-home-frederico--buzz"
        );
        // Disco: ~/.claude-glm/…/MercadoLivre-automacao-ml ↔ …/automacao_ml
        assert_eq!(
            encode_claude_project("/home/frederico/projects/MercadoLivre/automacao_ml"),
            "-home-frederico-projects-MercadoLivre-automacao-ml"
        );
        // `-` no path já é `-` (Correios-OneForAll).
        assert_eq!(
            encode_claude_project("/home/frederico/projects/Correios-OneForAll"),
            "-home-frederico-projects-Correios-OneForAll"
        );
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

    fn write_one_kid(home: &Path, cwd: &str, sid: &str, kid: &str, pid: u32) {
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

    #[test]
    fn expand_dois_pais_grok_mesmo_cwd_um_filho_conta_um() {
        // Sem bind pane↔sessão (pid fake), 2 working no mesmo cwd é ambíguo:
        // fallback recusa. Filho conta 0×, nunca 2×.
        let home = tmp_dir("g-fanout");
        let cwd = "/tmp/proj-fanout";
        write_one_kid(&home, cwd, "sess-a", "kid", 4_294_967_294);
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
        // Fallback por cwd: pid vivo sem HERDR_PANE_ID (init) + único grok no cwd.
        let home = tmp_dir("g-one");
        let cwd = "/tmp/proj-one";
        write_one_kid(&home, cwd, "sess-a", "kid", 1);
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
        write_one_kid(&home, cwd, "sess-a", "kid", 4_294_967_294);
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
    fn expand_pid_morto_descarta_sessao_stale() {
        // Pid fake morto ≠ "sem chave": mesmo com 1 working no cwd, não adota
        // o filho cujo meta.json ficou running pra sempre.
        let home = tmp_dir("g-stale");
        let cwd = "/tmp/proj-stale";
        write_one_kid(&home, cwd, "sess-dead", "ghost", 4_294_967_294);
        let a = AgentInfo {
            pane_id: "w1:p1".into(),
            cwd: Some(cwd.into()),
            session_id: None,
            ..parent_grok()
        };
        let out = with_grok_home(&home, || expand(&[a.clone()]));
        assert!(
            out.iter().all(|x| x.session_id.as_deref() != Some("ghost")),
            "pid morto não cai no fallback por cwd"
        );
        assert_eq!(out, vec![a]);
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

    #[test]
    fn expand_glm_sem_session_unico_no_cwd_pega_filho_do_jsonl_mais_novo() {
        let root = tmp_dir("c-glm-one");
        let cwd = "/tmp/glm-proj";
        write_claude_session(&root, cwd, "old-sid", Some(("ghost", true)), 3600);
        write_claude_session(&root, cwd, "new-sid", Some(("kid", true)), 0);
        let p = parent_glm(cwd, "w19:pS");
        let out = with_claude_cfg(&root, || expand(&[p.clone()]));
        let kids: Vec<_> = out
            .iter()
            .filter(|a| a.pane_id.starts_with("w19:pS:"))
            .collect();
        assert_eq!(kids.len(), 1, "só o filho da sessão mais nova");
        assert_eq!(kids[0].session_id.as_deref(), Some("kid"));
        assert!(
            out.iter().all(|a| a.session_id.as_deref() != Some("ghost")),
            "sessão velha não vaza"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expand_glm_dois_no_mesmo_cwd_nao_chuta() {
        let root = tmp_dir("c-glm-two");
        let cwd = "/tmp/glm-two";
        write_claude_session(&root, cwd, "sid", Some(("kid", true)), 0);
        let a = parent_glm(cwd, "w19:pS");
        let b = parent_glm(cwd, "w19:pV");
        let out = with_claude_cfg(&root, || expand(&[a.clone(), b.clone()]));
        assert!(
            out.iter().all(|x| x.session_id.as_deref() != Some("kid")),
            "ambíguo: 2 claude-kind no cwd"
        );
        assert_eq!(out.len(), 2);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expand_glm_com_pane_sem_marca_no_cwd_nao_chuta() {
        // Guard conta qualquer possível dono (agent: None), não só claude/glm.
        let root = tmp_dir("c-glm-none");
        let cwd = "/tmp/glm-none";
        write_claude_session(&root, cwd, "sid", Some(("kid", true)), 0);
        let glm = parent_glm(cwd, "w19:pS");
        let unmarked = AgentInfo {
            agent: None,
            session_id: None,
            status: AgentStatus::Idle,
            pane_id: "w19:pX".into(),
            cwd: Some(cwd.into()),
            ..parent_claude()
        };
        let out = with_claude_cfg(&root, || expand(&[glm.clone(), unmarked.clone()]));
        assert!(
            out.iter().all(|a| a.session_id.as_deref() != Some("kid")),
            "pane sem marca no cwd bloqueia o fallback"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expand_glm_working_mais_idle_mesmo_cwd_sem_session_nao_chuta() {
        let root = tmp_dir("c-glm-mix");
        let cwd = "/tmp/glm-mix";
        write_claude_session(&root, cwd, "sid", Some(("kid", true)), 0);
        let idle = AgentInfo {
            status: AgentStatus::Idle,
            ..parent_glm(cwd, "w19:pIdle")
        };
        let working = parent_glm(cwd, "w19:pWork");
        let out = with_claude_cfg(&root, || expand(&[idle.clone(), working.clone()]));
        assert!(
            out.iter().all(|a| a.session_id.as_deref() != Some("kid")),
            "idle no cwd bloqueia o fallback (qualquer status conta)"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expand_claude_com_session_id_nao_adota_jsonl_mais_novo() {
        let root = tmp_dir("c-sid-wins");
        let cwd = "/tmp/claude-sid";
        write_claude_session(&root, cwd, "sess-bound", Some(("mine", true)), 3600);
        write_claude_session(&root, cwd, "sess-new", Some(("other", true)), 0);
        let p = AgentInfo {
            cwd: Some(cwd.into()),
            session_id: Some("sess-bound".into()),
            ..parent_claude()
        };
        let out = with_claude_cfg(&root, || expand(&[p.clone()]));
        let kids: Vec<_> = out
            .iter()
            .filter(|a| a.pane_id.contains(':') && a.pane_id != p.pane_id)
            .collect();
        assert_eq!(kids.len(), 1);
        assert_eq!(kids[0].session_id.as_deref(), Some("mine"));
        assert!(out.iter().all(|a| a.session_id.as_deref() != Some("other")));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expand_glm_rotulo_glm_tambem_descobre() {
        let root = tmp_dir("c-glm-label");
        let cwd = "/tmp/glm-label";
        write_claude_session(&root, cwd, "sid", Some(("kid", true)), 0);
        let p = AgentInfo {
            agent: Some("glm".into()),
            ..parent_glm(cwd, "w19:pG")
        };
        let out = with_claude_cfg(&root, || expand(&[p.clone()]));
        assert_eq!(
            out.iter()
                .filter(|a| a.session_id.as_deref() == Some("kid"))
                .count(),
            1
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expand_glm_mais_novo_sem_filho_nao_pega_o_velho() {
        // Subcontar: jsonl novo sem subagents, velho com filho running → 0.
        let root = tmp_dir("c-glm-undercount");
        let cwd = "/tmp/glm-under";
        write_claude_session(&root, cwd, "old-sid", Some(("ghost", true)), 3600);
        write_claude_session(&root, cwd, "new-sid", None, 0);
        let p = parent_glm(cwd, "w19:pS");
        let out = with_claude_cfg(&root, || expand(&[p.clone()]));
        assert_eq!(out, vec![p]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expand_pai_sem_marca_com_session_ainda_pega_filho() {
        // Regressão: session_id no disco vale pra qualquer agent (None / unknown).
        let root = tmp_dir("c-no-brand");
        let cwd = "/tmp/nobrand";
        write_claude_session(&root, cwd, "sess-x", Some(("kid", true)), 3600);
        let p = AgentInfo {
            agent: None,
            session_id: Some("sess-x".into()),
            cwd: Some(cwd.into()),
            pane_id: "w1:pZ".into(),
            ..parent_claude()
        };
        let out = with_claude_cfg(&root, || expand(&[p.clone()]));
        assert_eq!(
            out.iter()
                .filter(|a| a.session_id.as_deref() == Some("kid"))
                .count(),
            1,
            "marca ausente + session_id ainda descobre"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expand_pai_unknown_com_session_ainda_pega_filho() {
        let root = tmp_dir("c-unknown");
        let cwd = "/tmp/unknown-brand";
        write_claude_session(&root, cwd, "sess-u", Some(("kid", true)), 0);
        let p = AgentInfo {
            agent: Some("unknown".into()),
            session_id: Some("sess-u".into()),
            cwd: Some(cwd.into()),
            pane_id: "w1:pU".into(),
            ..parent_claude()
        };
        let out = with_claude_cfg(&root, || expand(&[p.clone()]));
        assert_eq!(
            out.iter()
                .filter(|a| a.session_id.as_deref() == Some("kid"))
                .count(),
            1
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expand_glm_jsonl_velho_nao_e_adotado() {
        // 2a: único no cwd, mas o jsonl parou há 1 h → leftover / fora do Herdr.
        let root = tmp_dir("c-glm-stale");
        let cwd = "/tmp/glm-stale";
        write_claude_session(&root, cwd, "old-sid", Some(("ghost", true)), 3600);
        let p = parent_glm(cwd, "w19:pS");
        let out = with_claude_cfg(&root, || expand(&[p.clone()]));
        assert!(
            out.iter().all(|a| a.session_id.as_deref() != Some("ghost")),
            "jsonl fora da janela de {CLAUDE_FALLBACK_FRESH_SECS}s não é adotado"
        );
        assert_eq!(out, vec![p]);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expand_glm_recente_em_dois_roots_recusa() {
        // 2b: ~/.claude e ~/.claude-glm (aqui: dois roots fake) com jsonl fresco
        // no mesmo cwd → não mistura, recusa.
        let root_a = tmp_dir("c-glm-root-a");
        let root_b = tmp_dir("c-glm-root-b");
        let cwd = "/tmp/glm-cross";
        write_claude_session(&root_a, cwd, "sid-a", Some(("kid-a", true)), 0);
        write_claude_session(&root_b, cwd, "sid-b", Some(("kid-b", true)), 0);
        let p = parent_glm(cwd, "w19:pS");
        let out = with_claude_two_roots(&root_a, &root_b, || expand(&[p.clone()]));
        assert!(
            out.iter().all(|a| {
                a.session_id.as_deref() != Some("kid-a") && a.session_id.as_deref() != Some("kid-b")
            }),
            "recentes em 2 roots: ambíguo"
        );
        assert_eq!(out, vec![p]);
        let _ = fs::remove_dir_all(root_a);
        let _ = fs::remove_dir_all(root_b);
    }
}
