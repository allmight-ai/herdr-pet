//! Diário de sessões: cada pane do pet que fecha vira uma linha no
//! `sessions.jsonl`, ao lado do `state.json`. O `Summary` da sessão hoje morre
//! na tela; aqui ele vira histórico — matéria-prima do `log` e das sequências.
//!
//! Append-only e tolerante: linha ilegível é pulada, nunca derruba a leitura —
//! diário é acessório, não pode quebrar o pet.

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Uma sessão encerrada. `day` é a data **local** do fecho (`YYYY-MM-DD`),
/// gravada na hora: quem conta sequência conta os dias do usuário, não os do UTC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub day: String,
    pub started_at: u64,
    pub ended_at: u64,
    pub xp_gained: u64,
    pub xp_total: u64,
    pub level: u8,
    pub agents: usize,
    pub secs_working: u64,
}

/// `state_dir()/sessions.jsonl`.
pub fn path() -> PathBuf {
    crate::state::state_dir().join("sessions.jsonl")
}

/// Anexa uma sessão ao diário padrão.
pub fn append(e: &Entry) -> std::io::Result<()> {
    append_to(&path(), e)
}

/// Anexa num caminho explícito (testável).
///
/// Uma linha curta com `O_APPEND` é atômica na prática em filesystems POSIX —
/// não reescrevemos o arquivo inteiro: o diário só cresce e o pet salva com o
/// pane fechando. Termina sempre com `\n` pra o próximo append não colar.
pub fn append_to(p: &Path, e: &Entry) -> std::io::Result<()> {
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let mut line = serde_json::to_vec(e)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    line.push(b'\n');
    let mut f = OpenOptions::new().create(true).append(true).open(p)?;
    f.write_all(&line)?;
    Ok(())
}

/// Lê o diário padrão. Vazio se não existir.
pub fn load() -> Vec<Entry> {
    load_from(&path())
}

/// Lê de um caminho explícito, pulando linhas ilegíveis.
///
/// Arquivo ausente → vazio (sem erro). Linha que não parseia é ignorada: o
/// diário não pode derrubar o pet por uma linha truncada ou lixo no meio.
pub fn load_from(p: &Path) -> Vec<Entry> {
    let f = match fs::File::open(p) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in BufReader::new(f).lines() {
        let Ok(line) = line else {
            continue;
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(e) = serde_json::from_str::<Entry>(line) {
            out.push(e);
        }
    }
    out
}

/// Data local de hoje (`YYYY-MM-DD`), com fallback pra UTC.
///
/// Usa `date +%F` (um subprocesso, uma vez por fecho de sessão) pra respeitar
/// o fuso do usuário sem puxar `chrono`. Se o comando falhar ou devolver lixo,
/// cai na data UTC do epoch — subcontar um dia de fuso é melhor que inventar.
pub fn today_local() -> String {
    if let Ok(out) = Command::new("date").arg("+%F").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            let s = s.trim();
            if is_ymd(s) {
                return s.to_string();
            }
        }
    }
    utc_today_from_epoch()
}

fn is_ymd(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b[..4].iter().all(u8::is_ascii_digit)
        && b[5..7].iter().all(u8::is_ascii_digit)
        && b[8..].iter().all(u8::is_ascii_digit)
}

/// Data UTC de hoje a partir do epoch — fallback sem dependência nova.
fn utc_today_from_epoch() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Inverso de `days_from_civil` (Hinnant): dias desde 1970-01-01 → (y, m, d).
/// Local aqui porque a fatia C ainda não expõe o inverso — candidato a
/// fatorar em `streaks` depois.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(day: &str, xp: u64) -> Entry {
        Entry {
            day: day.to_string(),
            started_at: 1_700_000_000,
            ended_at: 1_700_003_600,
            xp_gained: xp,
            xp_total: 100 + xp,
            level: 2,
            agents: 1,
            secs_working: 3600,
        }
    }

    fn tpath(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "herdr-pet-journal-{}-{}-{tag}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn round_trip_append_e_load_devolve_igual() {
        let p = tpath("round");
        let _ = fs::remove_file(&p);
        let e = sample("2026-08-19", 42);
        append_to(&p, &e).unwrap();
        let got = load_from(&p);
        assert_eq!(got, vec![e]);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn dois_appends_preservam_ordem_sem_reescrever() {
        let p = tpath("ordem");
        let _ = fs::remove_file(&p);
        let a = sample("2026-08-18", 10);
        let b = sample("2026-08-19", 20);
        append_to(&p, &a).unwrap();
        let after_one = fs::read(&p).unwrap();
        append_to(&p, &b).unwrap();
        let after_two = fs::read(&p).unwrap();
        // O segundo append só cresce o arquivo — o prefixo da primeira linha fica.
        assert!(
            after_two.starts_with(&after_one),
            "segundo append reescreveu o arquivo em vez de anexar"
        );
        assert_eq!(load_from(&p), vec![a, b]);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn linha_corrompida_no_meio_e_pulada() {
        let p = tpath("corrupt");
        let _ = fs::remove_file(&p);
        let good1 = sample("2026-08-17", 1);
        let good2 = sample("2026-08-18", 2);
        append_to(&p, &good1).unwrap();
        {
            let mut f = OpenOptions::new().append(true).open(&p).unwrap();
            writeln!(f, "{{isto nao e json").unwrap();
            writeln!(f).unwrap();
            writeln!(f, "{{\"day\":\"x\"}}").unwrap(); // JSON válido mas não é Entry
        }
        append_to(&p, &good2).unwrap();
        assert_eq!(load_from(&p), vec![good1, good2]);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn arquivo_ausente_devolve_vazio() {
        let p = tpath("missing");
        let _ = fs::remove_file(&p);
        assert!(!p.exists());
        assert!(load_from(&p).is_empty());
    }

    #[test]
    fn today_local_casa_com_ymd() {
        let s = today_local();
        assert!(is_ymd(&s), "today_local() = {s:?} não casa com YYYY-MM-DD");
    }

    #[test]
    fn civil_from_days_ancora_no_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(1), (1970, 1, 2));
        // 2024-02-29 (bissexto) — 19782 dias desde 1970-01-01.
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }
}
