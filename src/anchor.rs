//! Resolve a âncora GitHub e aplica o **lock-in**: a 1ª conta vista trava, e trocar
//! de conta depois NÃO rerolla o pet (a raridade fica presa à identidade original).

use crate::state::{self, State};

/// Lê o ID numérico do usuário via `gh api user --jq .id`.
pub fn resolve_github_id() -> Result<u64, String> {
    let out = std::process::Command::new("gh")
        .args(["api", "user", "--jq", ".id"])
        .output()
        .map_err(|e| format!("não consegui rodar `gh`: {}", e))?;
    if !out.status.success() {
        return Err(format!(
            "`gh api user` falhou (você está logado no gh?): {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    s.parse::<u64>()
        .map_err(|_| format!("id do GitHub não é numérico: {:?}", s))
}

/// Garante um state com a âncora travada.
///
/// - Se já existe state → **reusa** a âncora gravada (lock-in; trocar de conta
///   GitHub não rerolla).
/// - Se não existe → resolve do `gh`, cria o state e persiste.
pub fn ensure_locked_state() -> Result<State, String> {
    if let Some(existing) = state::load() {
        return Ok(existing);
    }
    let id = resolve_github_id()?;
    let s = State::new(id);
    state::save(&s).map_err(|e| format!("erro ao salvar state: {}", e))?;
    Ok(s)
}
