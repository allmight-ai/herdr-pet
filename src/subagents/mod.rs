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
use std::path::Path;

use crate::agent::{AgentInfo, AgentStatus};

mod claude;
mod grok;
#[cfg(test)]
mod testkit;

use claude::claude_children;
use grok::grok_children;

pub use claude::{claude_still_running, encode_claude_project};
pub use grok::{encode_grok_cwd, grok_still_running};

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

pub(super) fn child(parent: &AgentInfo, id: &str, title: Option<String>) -> AgentInfo {
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

// --- peças que os dois backends usam ---

/// Desempate quando a cauda do jsonl **não** tem tool pendente (ou o meta
/// Grok não tem `status`). Bem acima do teto do Bash (600 s): tools
/// paralelas podem ficar 10 min sem escrever se a lenta ainda roda — o
/// casamento `tool_use`/`tool_result` cobre isso; 1800 s é cinto extra.
pub(super) const CHILD_STALE_SECS: u64 = 1800;

/// Mtime além de `secs` atrás. Stat falho ⇒ `false`: I/O transitório não mata filho.
pub(super) fn file_older_than(path: &Path, secs: u64) -> bool {
    let Ok(mtime) = fs::metadata(path).and_then(|m| m.modified()) else {
        return false; // stat falhou: não mata filho por I/O transitório
    };
    match std::time::SystemTime::now().duration_since(mtime) {
        Ok(age) => age.as_secs() > secs,
        Err(_) => false,
    }
}

/// Cwd sem a barra final — o Herdr e os backends divergem nisso.
pub(super) fn norm_cwd(cwd: &str) -> String {
    cwd.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subagents::claude::CLAUDE_FALLBACK_FRESH_SECS;
    use crate::subagents::testkit::*;

    #[test]
    fn expand_ignora_filho_se_pai_nao_esta_working() {
        let idle = AgentInfo {
            status: AgentStatus::Done,
            ..parent_claude()
        };
        assert_eq!(expand(std::slice::from_ref(&idle)), vec![idle]);
    }

    #[test]
    fn expand_nao_inventa_filho_sem_disco() {
        let home = tmp_dir("g-empty");
        write(&home.join("active_sessions.json"), "[]");
        let grok = parent_grok();
        let out = with_grok_home(&home, || expand(std::slice::from_ref(&grok)));
        assert_eq!(out, vec![grok]);
        let _ = fs::remove_dir_all(home);
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
        let out = with_grok_home(&home, || expand(std::slice::from_ref(&a)));
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
        let out = with_grok_home(&home, || expand(std::slice::from_ref(&a)));
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
        let out = with_claude_cfg(&root, || expand(std::slice::from_ref(&p)));
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
        let out = with_claude_cfg(&root, || expand(std::slice::from_ref(&p)));
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
        let out = with_claude_cfg(&root, || expand(std::slice::from_ref(&p)));
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
        let out = with_claude_cfg(&root, || expand(std::slice::from_ref(&p)));
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
        let out = with_claude_cfg(&root, || expand(std::slice::from_ref(&p)));
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
        let out = with_claude_cfg(&root, || expand(std::slice::from_ref(&p)));
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
        let out = with_claude_cfg(&root, || expand(std::slice::from_ref(&p)));
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
        let out = with_claude_two_roots(&root_a, &root_b, || expand(std::slice::from_ref(&p)));
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
