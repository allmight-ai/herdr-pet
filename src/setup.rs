//! Pós-install / startup: deixa o pet usável sem o usuário editar config.
//!
//! O Herdr **não** carrega `[[keys.command]]` do `herdr-plugin.toml` (só do
//! `~/.config/herdr/config.toml` do user). Também não coloca o binário no PATH.
//! Este módulo faz os dois de forma idempotente no `build` e no `startup`.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const PLUGIN_ID: &str = "allmight-ai.herdr-pet";
pub const ACTION_ID: &str = "allmight-ai.herdr-pet.open";

const MARKER_BEGIN: &str = "# >>> herdr-pet (managed — do not edit)";
const MARKER_END: &str = "# <<< herdr-pet";

/// Preferências de atalho (primeira livre ganha).
const KEY_CANDIDATES: &[&str] = &["prefix+a", "prefix+shift+a", "prefix+p"];

#[derive(Debug, Default)]
pub struct SetupReport {
    pub keybind: KeybindStatus,
    pub shim: ShimStatus,
    pub reload: ReloadStatus,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeybindStatus {
    AlreadyOk,
    Installed,
    ConflictSkipped,
    ConfigMissing,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShimStatus {
    AlreadyOk,
    Installed,
    Updated,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReloadStatus {
    NotNeeded,
    Reloaded,
    Skipped,
}

impl Default for KeybindStatus {
    fn default() -> Self {
        Self::AlreadyOk
    }
}
impl Default for ShimStatus {
    fn default() -> Self {
        Self::AlreadyOk
    }
}
impl Default for ReloadStatus {
    fn default() -> Self {
        Self::NotNeeded
    }
}

/// Garante keybind no config do Herdr + shim em `~/.local/bin/herdr-pet`.
pub fn ensure_setup() -> Result<SetupReport, String> {
    let mut report = SetupReport::default();
    let binary = current_binary()?;

    let (key_status, key, key_changed) = ensure_keybind()?;
    report.keybind = key_status;
    report.key = key;
    let (shim_status, _) = ensure_path_shim(&binary)?;
    report.shim = shim_status;

    let need_reload = key_changed
        || matches!(
            report.shim,
            ShimStatus::Installed | ShimStatus::Updated
        )
        || matches!(report.keybind, KeybindStatus::Installed);

    if need_reload {
        report.reload = try_reload_config();
    }

    Ok(report)
}

pub fn print_report(r: &SetupReport) {
    match r.keybind {
        KeybindStatus::AlreadyOk => {
            if let Some(k) = &r.key {
                println!("✓ atalho ok: {k} → toggle do pet");
            } else {
                println!("✓ atalho ok");
            }
        }
        KeybindStatus::Installed => {
            let k = r.key.as_deref().unwrap_or("prefix+a");
            println!("✓ atalho instalado: {k} (Ctrl+b, soltar, tecla) → abre/fecha o pet");
        }
        KeybindStatus::ConflictSkipped => {
            eprintln!(
                "! não consegui gravar atalho automático (todas as teclas candidatas ocupadas)."
            );
            eprintln!(
                "  adicione no ~/.config/herdr/config.toml:\n  [[keys.command]]\n  key = \"prefix+a\"\n  type = \"plugin_action\"\n  command = \"{ACTION_ID}\""
            );
        }
        KeybindStatus::ConfigMissing => {
            eprintln!("! ~/.config/herdr/config.toml não encontrado — abra o Herdr uma vez e rode `herdr-pet setup` de novo.");
        }
        KeybindStatus::Error => eprintln!("! falha ao gravar atalho"),
    }

    match r.shim {
        ShimStatus::AlreadyOk => println!("✓ CLI no PATH: herdr-pet"),
        ShimStatus::Installed => println!("✓ CLI instalado em ~/.local/bin/herdr-pet"),
        ShimStatus::Updated => println!("✓ CLI no PATH atualizado (novo path do plugin)"),
        ShimStatus::Error => eprintln!("! não consegui instalar o shim em ~/.local/bin"),
    }

    match r.reload {
        ReloadStatus::Reloaded => println!("✓ config do Herdr recarregada"),
        ReloadStatus::Skipped => {
            // server pode não estar rodando no momento do install — ok
        }
        ReloadStatus::NotNeeded => {}
    }

    if matches!(
        r.keybind,
        KeybindStatus::Installed | KeybindStatus::AlreadyOk
    ) {
        if let Some(k) = &r.key {
            let short = k.strip_prefix("prefix+").unwrap_or(k);
            println!();
            println!("  Use no Herdr: Ctrl+b, soltar, depois `{short}`");
        }
    }
}

fn current_binary() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    // Resolve symlinks so the shim points at the real plugin binary.
    fs::canonicalize(&exe).or_else(|_| Ok(exe))
}

fn herdr_config_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("HERDR_CONFIG") {
        return Some(PathBuf::from(p));
    }
    let home = std::env::var("HOME").ok()?;
    let p = PathBuf::from(home).join(".config/herdr/config.toml");
    Some(p)
}

fn managed_block(key: &str) -> String {
    format!(
        r#"{MARKER_BEGIN}
[[keys.command]]
key = "{key}"
type = "plugin_action"
command = "{ACTION_ID}"
description = "Pet: toggle (abre/fecha)"
{MARKER_END}
"#
    )
}

/// Returns (status, bound_key, content_changed).
fn ensure_keybind() -> Result<(KeybindStatus, Option<String>, bool), String> {
    let Some(path) = herdr_config_path() else {
        return Ok((KeybindStatus::ConfigMissing, None, false));
    };
    if !path.exists() {
        return Ok((KeybindStatus::ConfigMissing, None, false));
    }

    let original = fs::read_to_string(&path).map_err(|e| format!("ler config: {e}"))?;

    // Já tem a action em algum keybind (managed ou manual) → não duplica.
    if original.contains(ACTION_ID) {
        let key = extract_managed_key(&original)
            .or_else(|| find_key_for_action(&original))
            .or_else(|| Some("prefix+a".into()));
        // Se ainda não tem bloco managed, migra pro formato managed (sem mudar a tecla).
        if extract_managed_key(&original).is_none() {
            if let Some(ref k) = key {
                // Remove o binding manual solto e grava o managed (mesmo key/action).
                let cleaned = remove_unmanaged_action_bindings(&original);
                let new_content = upsert_managed_block(&cleaned, k);
                if new_content != original {
                    write_atomic(&path, &new_content)?;
                    return Ok((KeybindStatus::Installed, key, true));
                }
            }
        }
        return Ok((KeybindStatus::AlreadyOk, key, false));
    }

    // Escolhe tecla livre
    let key = match pick_free_key(&original) {
        Some(k) => k.to_string(),
        None => return Ok((KeybindStatus::ConflictSkipped, None, false)),
    };

    let new_content = upsert_managed_block(&original, &key);
    if new_content == original {
        return Ok((KeybindStatus::AlreadyOk, Some(key), false));
    }

    write_atomic(&path, &new_content)?;
    Ok((KeybindStatus::Installed, Some(key), true))
}

/// Acha a `key = "..."` associada ao nosso ACTION_ID (janela de linhas ao redor).
fn find_key_for_action(content: &str) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if !line.contains(ACTION_ID) {
            continue;
        }
        let lo = i.saturating_sub(6);
        let hi = (i + 2).min(lines.len());
        for l in &lines[lo..hi] {
            let t = l.trim();
            if let Some(rest) = t.strip_prefix("key") {
                let rest = rest.trim().trim_start_matches('=').trim();
                let key = rest.trim_matches('"').trim_matches('\'').to_string();
                if !key.is_empty() {
                    return Some(key);
                }
            }
        }
    }
    None
}

/// Remove `[[keys.command]]` blocks that reference our action but aren't managed.
fn remove_unmanaged_action_bindings(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim() == "[[keys.command]]" {
            // Collect this table until next [[ or [section] or EOF
            let start = i;
            i += 1;
            while i < lines.len() {
                let t = lines[i].trim();
                if t.starts_with("[[") || (t.starts_with('[') && !t.starts_with("[[")) {
                    break;
                }
                i += 1;
            }
            let block = lines[start..i].join("\n");
            if block.contains(ACTION_ID) && !block.contains(MARKER_BEGIN) {
                // drop this block (and a surrounding blank line)
                if out.last().map(|s| s.trim().is_empty()).unwrap_or(false) {
                    out.pop();
                }
                continue;
            }
            for l in &lines[start..i] {
                out.push((*l).to_string());
            }
            continue;
        }
        out.push(line.to_string());
        i += 1;
    }
    let mut s = out.join("\n");
    if content.ends_with('\n') && !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

fn extract_managed_key(content: &str) -> Option<String> {
    let start = content.find(MARKER_BEGIN)?;
    let end = content[start..].find(MARKER_END)? + start;
    let block = &content[start..end];
    for line in block.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("key") {
            let rest = rest.trim().trim_start_matches('=').trim();
            let key = rest.trim_matches('"').trim_matches('\'').to_string();
            if !key.is_empty() {
                return Some(key);
            }
        }
    }
    None
}

fn pick_free_key(content: &str) -> Option<&'static str> {
    // Se já tem nosso action em algum keybind (managed ou manual), reutiliza.
    if content.contains(ACTION_ID) {
        // tenta achar a key na linha anterior-ish — para already-ok path
        for cand in KEY_CANDIDATES {
            if content_has_key_binding(content, cand) {
                // check if that binding is ours
                if binding_is_ours(content, cand) {
                    return Some(cand);
                }
            }
        }
    }

    for cand in KEY_CANDIDATES {
        if !content_has_key_binding(content, cand) {
            return Some(cand);
        }
        // ocupada: só reutiliza se for a nossa
        if binding_is_ours(content, cand) {
            return Some(cand);
        }
    }
    None
}

fn content_has_key_binding(content: &str, key: &str) -> bool {
    // Match `key = "prefix+a"` (with flexible spaces/quotes)
    for line in content.lines() {
        let t = line.trim();
        if !t.starts_with("key") {
            continue;
        }
        if t.contains(key) {
            return true;
        }
    }
    false
}

fn binding_is_ours(content: &str, key: &str) -> bool {
    // Managed block always ours.
    if let Some(k) = extract_managed_key(content) {
        if k == key {
            return true;
        }
    }
    // Heuristic: within ~8 lines of a key= line, look for our action id.
    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.starts_with("key") && t.contains(key) {
            let lo = i.saturating_sub(2);
            let hi = (i + 8).min(lines.len());
            let window = lines[lo..hi].join("\n");
            if window.contains(ACTION_ID) || window.contains("herdr-pet") {
                return true;
            }
        }
    }
    false
}

fn upsert_managed_block(content: &str, key: &str) -> String {
    let block = managed_block(key);
    if let (Some(start), Some(rel_end)) = (content.find(MARKER_BEGIN), content.find(MARKER_END)) {
        if rel_end >= start {
            let end = rel_end + MARKER_END.len();
            // include trailing newline after end marker if present
            let mut end = end;
            if content[end..].starts_with('\n') {
                end += 1;
            }
            let mut out = String::new();
            out.push_str(&content[..start]);
            out.push_str(&block);
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&content[end..]);
            return out;
        }
    }

    // Also remove the old mistaken comment if present (pre-setup installs).
    let mut base = content.to_string();
    let stale = "# prefix+a do pet vem do plugin allmight-ai.herdr-pet (herdr-plugin.toml)";
    if base.contains(stale) {
        base = base.replace(stale, "");
    }

    // Append managed block before [worktrees] if present, else at end.
    if let Some(idx) = base.find("\n[worktrees]") {
        let mut out = String::new();
        out.push_str(&base[..idx]);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&block);
        out.push_str(&base[idx..]);
        return out;
    }

    let mut out = base;
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push('\n');
    out.push_str(&block);
    out
}

fn ensure_path_shim(binary: &Path) -> Result<(ShimStatus, PathBuf), String> {
    let home = std::env::var("HOME").map_err(|_| "HOME não definido".to_string())?;
    let bin_dir = PathBuf::from(&home).join(".local/bin");
    fs::create_dir_all(&bin_dir).map_err(|e| format!("mkdir ~/.local/bin: {e}"))?;
    let shim = bin_dir.join("herdr-pet");

    let body = format!(
        "#!/usr/bin/env bash\n# managed by herdr-pet setup — do not edit\nset -euo pipefail\nexec \"{bin}\" \"$@\"\n",
        bin = binary.display()
    );

    if shim.exists() {
        if let Ok(existing) = fs::read_to_string(&shim) {
            if existing == body {
                // ensure executable
                set_executable(&shim)?;
                return Ok((ShimStatus::AlreadyOk, shim));
            }
        }
        // symlink or stale content → replace
        let _ = fs::remove_file(&shim);
        write_atomic(&shim, &body)?;
        set_executable(&shim)?;
        return Ok((ShimStatus::Updated, shim));
    }

    write_atomic(&shim, &body)?;
    set_executable(&shim)?;
    Ok((ShimStatus::Installed, shim))
}

fn set_executable(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .map_err(|e| format!("stat shim: {e}"))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).map_err(|e| format!("chmod shim: {e}"))?;
    }
    Ok(())
}

fn write_atomic(path: &Path, content: &str) -> Result<(), String> {
    let tmp = path.with_extension("tmp-herdr-pet");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("create tmp: {e}"))?;
        f.write_all(content.as_bytes())
            .map_err(|e| format!("write tmp: {e}"))?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path).map_err(|e| format!("rename tmp: {e}"))?;
    Ok(())
}

fn try_reload_config() -> ReloadStatus {
    let bin = herdr_bin_path();
    let out = Command::new(&bin)
        .args(["server", "reload-config"])
        .output();
    match out {
        Ok(o) if o.status.success() => ReloadStatus::Reloaded,
        _ => ReloadStatus::Skipped,
    }
}

fn herdr_bin_path() -> String {
    if let Ok(b) = std::env::var("HERDR_BIN_PATH") {
        return b;
    }
    if Command::new("herdr").arg("--version").output().is_ok() {
        return "herdr".to_string();
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = format!("{home}/.local/bin/herdr");
        if Path::new(&p).exists() {
            return p;
        }
    }
    "herdr".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_appends_when_missing() {
        let base = "onboarding = false\n\n[theme]\nname = \"cat\"\n";
        let out = upsert_managed_block(base, "prefix+a");
        assert!(out.contains(MARKER_BEGIN));
        assert!(out.contains("prefix+a"));
        assert!(out.contains(ACTION_ID));
    }

    #[test]
    fn upsert_replaces_existing_block() {
        let base = format!(
            "x = 1\n{}\n[worktrees]\ndirectory = \"~/.h\"\n",
            managed_block("prefix+a")
        );
        let out = upsert_managed_block(&base, "prefix+shift+a");
        assert_eq!(out.matches(MARKER_BEGIN).count(), 1);
        assert!(out.contains("prefix+shift+a"));
        assert!(!out.contains("key = \"prefix+a\"") || out.contains("prefix+shift+a"));
        assert!(out.contains("[worktrees]"));
    }

    #[test]
    fn pick_skips_taken_keys() {
        let content = r#"
[[keys.command]]
key = "prefix+a"
type = "shell"
command = "echo hi"
"#;
        assert_eq!(pick_free_key(content), Some("prefix+shift+a"));
    }

    #[test]
    fn pick_reuses_ours() {
        let content = managed_block("prefix+a");
        assert_eq!(pick_free_key(&content), Some("prefix+a"));
    }
}
