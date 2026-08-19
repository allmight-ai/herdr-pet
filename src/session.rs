//! Resumo da sessão: o que o pet mostra ao fechar o pane.
//!
//! Uma sessão começa na abertura do `watch` e termina no Ctrl+C / SIGHUP
//! (toggle). O resumo reforça o loop de progressão — fechar com recompensa
//! visível — e vira linha do diário (`journal::Entry`): o que a tela mostra
//! numa fração de segundo, o diário guarda pra sempre. Conceito em
//! `CONTEXT.md` ("Sessão").

use std::collections::HashSet;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Prefixo estável do resumo — o toggle espera esta substring no pane.
/// Mais específico que `"Sessão"` (C12): um título de tarefa com a palavra
/// não atalha o `wait-output`. Contrato: `format_line` começa com isto.
pub const SUMMARY_NEEDLE: &str = "🐾 Sessão:";

/// Quanto o resumo fica visível no pane antes do processo sair (Ctrl+C e toggle).
pub const FAREWELL_MS: u64 = 1400;

/// Acumulador da sessão corrente (não persistido — vive só enquanto o pane está aberto).
pub struct Session {
    start_xp: u64,
    start_level: u8,
    started_at: Instant,
    /// Início em epoch (segundos) — o `Instant` é monótono pra medir duração,
    /// mas o diário precisa de uma âncora no relógio de parede (`started_at`).
    started_epoch: u64,
    working_panes: HashSet<String>,
    peak_working: usize,
    /// Tempo total com algum agente `working` (o mesmo gate do XP live) —
    /// acumulado como `Duration` pra o sub-segundo de cada tick não truncar.
    working_time: Duration,
}

impl Session {
    /// Abre a sessão no XP/nível atuais (antes do catch-up, pra ele entrar no delta).
    pub fn start(xp: u64, level: u8) -> Self {
        Session {
            start_xp: xp,
            start_level: level,
            started_at: Instant::now(),
            started_epoch: epoch_now(),
            working_panes: HashSet::new(),
            peak_working: 0,
            working_time: Duration::ZERO,
        }
    }

    /// Registra quem trabalhou neste tick: panes `working` distintos + pico de `n_working`.
    /// `pane_id` vazio é ignorado (API às vezes omite).
    pub fn note_working<I, S>(&mut self, pane_ids: I, n_working: usize)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for id in pane_ids {
            let id = id.as_ref();
            if !id.is_empty() {
                self.working_panes.insert(id.to_string());
            }
        }
        self.peak_working = self.peak_working.max(n_working);
    }

    /// Conta `dt` como trabalho acompanhado (havia `working` no tick — o mesmo
    /// `mult > 0` que rende XP). Vira `secs_working` do diário; sessão sem
    /// acompanhamento (só catch-up) grava 0.
    pub fn note_working_span(&mut self, dt: Duration) {
        self.working_time += dt;
    }

    /// Fecha a sessão: delta de XP, agentes que trabalharam, nível, duração.
    ///
    /// Contagem: se **nunca** vimos `pane_id`, o pico anônimo vale (API omitiu).
    /// Se algum id apareceu, `unique` é a verdade — subconta mistura real
    /// anônimo+id em vez de inflar quando o mesmo agente ganha id depois (C11).
    pub fn summarize(&self, end_xp: u64, end_level: u8) -> Summary {
        let agents = if self.working_panes.is_empty() {
            self.peak_working
        } else {
            self.working_panes.len()
        };
        Summary {
            agents,
            xp_gained: end_xp.saturating_sub(self.start_xp),
            xp_total: end_xp,
            start_level: self.start_level,
            end_level,
            duration: self.started_at.elapsed(),
            started_at: self.started_epoch,
            ended_at: epoch_now(),
            secs_working: self.working_time.as_secs(),
        }
    }
}

/// Epoch de agora em segundos; 0 se o relógio estiver antes de 1970 (não
/// deveria acontecer — e o piso 0 só atrasa a linha do diário, não derruba nada).
fn epoch_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Snapshot imutável do que aconteceu na sessão — apresentação E a linha que
/// vira histórico (`to_entry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub agents: usize,
    pub xp_gained: u64,
    /// XP total no fecho (a âncora histórica: de onde o próximo dia compara).
    pub xp_total: u64,
    pub start_level: u8,
    pub end_level: u8,
    pub duration: Duration,
    /// Início/fim em epoch (segundos) — duração é pra tela, epoch é pro diário.
    pub started_at: u64,
    pub ended_at: u64,
    /// Segundos com trabalho acompanhado (sem catch-up — aquele não foi visto).
    pub secs_working: u64,
}

impl Summary {
    /// A sessão como linha do diário, no dia dado. O dia é do FECHO, decidido
    /// pelo chamador (`journal::today_local()`) — quem conta sequência conta
    /// os dias do usuário.
    pub fn to_entry(&self, day: String) -> crate::journal::Entry {
        crate::journal::Entry {
            day,
            started_at: self.started_at,
            ended_at: self.ended_at,
            xp_gained: self.xp_gained,
            xp_total: self.xp_total,
            level: self.end_level,
            agents: self.agents,
            secs_working: self.secs_working,
        }
    }
}

impl Summary {
    /// Linha completa: `🐾 Sessão: 2 agentes · +1.240 XP · Nv 6 → 7 · 47 min`.
    /// Começa com `SUMMARY_NEEDLE` — sem duplicar o prefixo.
    pub fn format_line(&self) -> String {
        format!("{SUMMARY_NEEDLE} {}", self.format_body())
    }

    /// Corpo sem o prefixo (pra `herdr notification --body`).
    pub fn format_body(&self) -> String {
        let mut parts = Vec::with_capacity(4);
        if self.agents == 0 && self.xp_gained > 0 {
            // Catch-up com ninguém working na sessão: não mente "0 agentes".
            parts.push(format!("recuperado: +{} XP", format_int(self.xp_gained)));
        } else {
            parts.push(if self.agents == 1 {
                "1 agente".to_string()
            } else {
                format!("{} agentes", self.agents)
            });
            parts.push(format!("+{} XP", format_int(self.xp_gained)));
        }
        parts.push(if self.end_level > self.start_level {
            format!("Nv {} → {}", self.start_level, self.end_level)
        } else {
            format!("Nv {}", self.end_level)
        });
        if self.duration.as_secs() > 0 {
            parts.push(format_duration(self.duration));
        }
        parts.join(" · ")
    }
}

/// Inteiro com separador de milhar pt-BR (`1240` → `1.240`).
pub fn format_int(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

/// Duração legível (`47 min`, `1 h 23 min`) — o resumo da sessão e as linhas
/// do diário (`log`) compartilham a mesma voz.
pub fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{secs} s")
    } else if secs < 3600 {
        format!("{} min", secs / 60)
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        if m == 0 {
            format!("{h} h")
        } else {
            format!("{h} h {m} min")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(agents: usize, xp: u64, from: u8, to: u8, secs: u64) -> Summary {
        Summary {
            agents,
            xp_gained: xp,
            xp_total: 0,
            start_level: from,
            end_level: to,
            duration: Duration::from_secs(secs),
            started_at: 0,
            ended_at: secs,
            secs_working: secs,
        }
    }

    #[test]
    fn entry_leva_os_campos_da_sessao_e_o_dia_do_fecho() {
        // O resumo é a tela; a Entry é a história — o mapeamento não pode
        // trocar campo (level do fecho, xp_total âncora, dias/horas cruas).
        let s = Summary {
            agents: 2,
            xp_gained: 1_240,
            xp_total: 45_678,
            start_level: 6,
            end_level: 7,
            duration: Duration::from_secs(47 * 60),
            started_at: 1_755_600_000,
            ended_at: 1_755_602_820,
            secs_working: 2_700,
        };
        let e = s.to_entry("2026-08-19".to_string());
        assert_eq!(
            e,
            crate::journal::Entry {
                day: "2026-08-19".to_string(),
                started_at: 1_755_600_000,
                ended_at: 1_755_602_820,
                xp_gained: 1_240,
                xp_total: 45_678,
                level: 7,
                agents: 2,
                secs_working: 2_700,
            }
        );
    }

    #[test]
    fn tempo_de_trabalho_acumula_subsegundo_sem_truncar_por_tick() {
        // Tick do watch é ~0,8s: truncar por tick perderia ~40% do tempo.
        let mut sess = Session::start(0, 1);
        sess.note_working_span(Duration::from_millis(700));
        sess.note_working_span(Duration::from_millis(700));
        assert_eq!(sess.summarize(0, 1).secs_working, 1); // 1,4s → 1s
    }

    #[test]
    fn sessao_sem_acompanhamento_grava_zero_segundos() {
        // Catch-up puro (ninguém working com o pane aberto): 0, não a duração.
        let mut sess = Session::start(0, 1);
        sess.note_working(std::iter::empty::<&str>(), 0);
        assert_eq!(sess.summarize(300, 1).secs_working, 0);
    }

    #[test]
    fn summarize_ancora_total_e_janela_de_epoch() {
        let sess = Session::start(100, 2);
        let s = sess.summarize(250, 3);
        assert_eq!(s.xp_gained, 150);
        assert_eq!(s.xp_total, 250, "total é a âncora do histórico");
        assert!(
            s.ended_at >= s.started_at,
            "janela de epoch não pode fechar antes de abrir"
        );
    }

    #[test]
    fn xp_que_regride_satura_em_zero_mas_ancora_o_total_real() {
        // State regravado por outro processo pode fechar abaixo da abertura:
        // delta satura (nunca negativo), o total registra o que de fato há.
        let sess = Session::start(500, 3);
        let s = sess.summarize(200, 2);
        assert_eq!(s.xp_gained, 0);
        assert_eq!(s.xp_total, 200);
    }

    #[test]
    fn linha_com_level_up_bate_o_exemplo_da_issue() {
        let s = summary(2, 1_240, 6, 7, 47 * 60);
        assert_eq!(
            s.format_line(),
            "🐾 Sessão: 2 agentes · +1.240 XP · Nv 6 → 7 · 47 min"
        );
        assert!(
            s.format_line().contains(SUMMARY_NEEDLE),
            "toggle espera {SUMMARY_NEEDLE} no pane"
        );
    }

    #[test]
    fn singular_e_sem_subida_de_nivel() {
        let s = summary(1, 80, 6, 6, 5 * 60);
        assert_eq!(s.format_body(), "1 agente · +80 XP · Nv 6 · 5 min");
    }

    #[test]
    fn zero_agentes_zero_xp_omite_duracao_curta() {
        let s = summary(0, 0, 3, 3, 0);
        assert_eq!(s.format_body(), "0 agentes · +0 XP · Nv 3");
    }

    #[test]
    fn milhar_e_milhao() {
        assert_eq!(format_int(0), "0");
        assert_eq!(format_int(999), "999");
        assert_eq!(format_int(1_240), "1.240");
        assert_eq!(format_int(1_000_000), "1.000.000");
    }

    #[test]
    fn duracao_s_min_hora() {
        assert_eq!(format_duration(Duration::from_secs(12)), "12 s");
        assert_eq!(format_duration(Duration::from_secs(59)), "59 s");
        assert_eq!(format_duration(Duration::from_secs(60)), "1 min");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1 h");
        assert_eq!(
            format_duration(Duration::from_secs(3600 + 23 * 60)),
            "1 h 23 min"
        );
    }

    #[test]
    fn sessao_conta_panes_distintos_nao_pico() {
        let mut sess = Session::start(100, 2);
        sess.note_working(["w1:p1"], 1);
        sess.note_working(["w2:p2"], 1); // sequenciais — pico 1, mas 2 agentes
        let sum = sess.summarize(250, 3);
        assert_eq!(sum.agents, 2);
        assert_eq!(sum.xp_gained, 150);
        assert_eq!(sum.start_level, 2);
        assert_eq!(sum.end_level, 3);
    }

    #[test]
    fn sessao_pane_vazio_cai_no_pico() {
        // Só anônimo (nunca vimos id): o pico é o que temos.
        let mut sess = Session::start(0, 1);
        sess.note_working([""], 3);
        assert_eq!(sess.summarize(0, 1).agents, 3);
    }

    #[test]
    fn sessao_id_depois_do_anonimo_nao_infla() {
        // Pico 3 sem id + depois 1 id: unique é a verdade (subconta mistura
        // real; não infla quando o mesmo agente ganha pane_id).
        let mut sess = Session::start(0, 1);
        sess.note_working([""], 3);
        sess.note_working(["w1:p1"], 1);
        assert_eq!(sess.summarize(0, 1).agents, 1);
    }

    #[test]
    fn sessao_mesmo_pane_nao_duplica() {
        let mut sess = Session::start(0, 1);
        sess.note_working(["w1:p1"], 1);
        sess.note_working(["w1:p1"], 1);
        assert_eq!(sess.summarize(0, 1).agents, 1);
    }

    #[test]
    fn catchup_sem_agente_nao_diz_zero_agentes() {
        let s = summary(0, 150, 2, 3, 5 * 60);
        assert_eq!(s.format_body(), "recuperado: +150 XP · Nv 2 → 3 · 5 min");
        let line = s.format_line();
        assert!(line.starts_with(SUMMARY_NEEDLE));
        assert!(!line.contains("0 agentes"));
    }
}
