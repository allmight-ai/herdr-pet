//! Descoberta dos filhos no layout `projects/<enc-cwd>/<sid>/subagents/`
//! — Claude Code e GLM (mesmo formato de disco).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::{child, file_older_than, norm_cwd, CHILD_STALE_SECS};
use crate::agent::{AgentInfo, AgentStatus};

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
pub(super) const CLAUDE_FALLBACK_FRESH_SECS: u64 = 120;

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
pub(super) fn claude_children(agents: &[AgentInfo]) -> Vec<AgentInfo> {
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
        .or(meta.agent_type)
}

/// Só o **último turno**: de trás pra frente até o último `assistant`.
/// `tool_use` sem `tool_result` casado nesse turno = tool paralela ainda
/// rodando. Turno antigo desbalanceado não imortaliza; o parse para na
/// cauda (não no arquivo inteiro).
fn claude_has_unmatched_tool(data: &str) -> bool {
    let mut results = HashSet::new();
    let mut anon_results: u32 = 0;
    for line in data.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(content) = v.pointer("/message/content").and_then(|c| c.as_array()) else {
            continue;
        };
        if v.get("type").and_then(|t| t.as_str()) != Some("assistant") {
            for item in content {
                if item.get("type").and_then(|t| t.as_str()) != Some("tool_result") {
                    continue;
                }
                if let Some(id) = item.get("tool_use_id").and_then(|i| i.as_str()) {
                    results.insert(id.to_string());
                } else {
                    anon_results = anon_results.saturating_add(1);
                }
            }
            continue;
        }
        let mut pending = 0u32;
        for item in content {
            if item.get("type").and_then(|t| t.as_str()) != Some("tool_use") {
                continue;
            }
            if let Some(id) = item
                .get("id")
                .and_then(|i| i.as_str())
                .filter(|s| !s.is_empty())
            {
                if !results.contains(id) {
                    pending = pending.saturating_add(1);
                }
            } else if anon_results == 0 {
                pending = pending.saturating_add(1);
            } else {
                anon_results = anon_results.saturating_sub(1);
            }
        }
        return pending > 0;
    }
    false
}

/// Ainda trabalhando? jsonl ausente = recém-spawnado. Terminou se o último
/// assistente tem `end_turn` **ou** só texto (relatório final sem stop_reason —
/// o Claude às vezes fecha assim e virava fantasma no `⚙ N`).
///
/// Tools paralelas no **último** turno: cada `tool_result` vira uma linha
/// `user`. A rápida chega antes; a lenta (Bash ≤ 600 s) ainda roda. Se
/// esse turno tem `tool_use` sem `tool_result` casado (por id) ⇒ vivo,
/// sem olhar mtime. mtime só desempatá cauda **sem** tool pendente no
/// turno atual (órfão de turno antigo não conta).
pub fn claude_still_running(jsonl: &Path) -> bool {
    if !jsonl.exists() {
        return true;
    }
    let Ok(data) = fs::read_to_string(jsonl) else {
        return false;
    };
    if claude_has_unmatched_tool(&data) {
        return true;
    }
    let Some(last) = data.lines().rev().find(|l| !l.trim().is_empty()) else {
        return true;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(last) else {
        return !file_older_than(jsonl, CHILD_STALE_SECS);
    };
    if v.get("type").and_then(|t| t.as_str()) != Some("assistant") {
        return !file_older_than(jsonl, CHILD_STALE_SECS);
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
    if types.contains(&"tool_use") {
        return true;
    }
    // Só texto (sem tool_use) = resposta final, mesmo sem stop_reason.
    if types.contains(&"text") {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subagents::testkit::*;

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
    fn claude_jsonl_tool_result_fresco_ainda_roda() {
        let dir = tmp_dir("tr-fresh");
        let jsonl = dir.join("a.jsonl");
        write(
            &jsonl,
            r#"{"type":"user","message":{"content":[{"type":"tool_result"}]}}
"#,
        );
        assert!(claude_still_running(&jsonl));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn claude_jsonl_tool_result_stale_esta_morto() {
        // Sem tool_use pendente + jsonl parado = morto (kill / leftover).
        let dir = tmp_dir("tr-stale");
        let jsonl = dir.join("a.jsonl");
        write(
            &jsonl,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_x"}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_x"}]}}
"#,
        );
        set_age(&jsonl, CHILD_STALE_SECS + 60);
        assert!(!claude_still_running(&jsonl));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn claude_jsonl_tool_pendente_mtime_velho_vive() {
        // Paralelo: Read já devolveu; Bash ainda roda. Last line = tool_result
        // da rápida. tool_use lento sem casamento ⇒ vivo apesar do mtime.
        let dir = tmp_dir("tr-pending");
        let jsonl = dir.join("a.jsonl");
        write(
            &jsonl,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_fast"},{"type":"tool_use","id":"toolu_slow"}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_fast"}]}}
"#,
        );
        set_age(&jsonl, CHILD_STALE_SECS + 60);
        assert!(claude_still_running(&jsonl));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn claude_jsonl_orfao_turno_antigo_cauda_casada_stale_morto() {
        // tool_use órfão no turno 1; turno 2 casado. A cauda atual não tem
        // pendente — mtime velho ⇒ morto (não imortaliza o órfão antigo).
        let dir = tmp_dir("tr-old-orphan");
        let jsonl = dir.join("a.jsonl");
        write(
            &jsonl,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_old"}]}}
{"type":"user","message":{"content":[{"type":"text","text":"ok"}]}}
{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_new"}]}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_new"}]}}
"#,
        );
        set_age(&jsonl, CHILD_STALE_SECS + 60);
        assert!(!claude_still_running(&jsonl));
        let _ = fs::remove_dir_all(dir);
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
}
