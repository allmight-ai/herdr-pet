//! Agregação do diário: sessões viram dias, dias viram sequência.
//!
//! Puro — nada de I/O e nada de relógio: `streak` recebe o "hoje" de fora, pra
//! o teste poder mentir sobre a data.

use std::collections::BTreeMap;

use crate::journal::Entry;

/// Um dia de trabalho, somando todas as sessões daquele dia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Day {
    pub day: String,
    pub xp: u64,
    pub secs_working: u64,
    pub sessions: usize,
}

/// Sequência de dias consecutivos com trabalho: a atual e o recorde.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Streak {
    pub current: u32,
    pub best: u32,
    pub last_day: Option<String>,
}

/// Um item por dia, em ordem cronológica. Sessões do mesmo dia são fundidas;
/// entrada com `day` ilegível é ignorada — linha corrompida do diário credita
/// menos, nunca mais (subcontar > inflar).
pub fn by_day(entries: &[Entry]) -> Vec<Day> {
    // Chave é o número civil do dia, não a string: o BTreeMap devolve a ordem
    // cronológica de graça e funde o dia mesmo se a string variar.
    let mut por_dia: BTreeMap<i64, Day> = BTreeMap::new();
    for e in entries {
        let Some((y, m, d)) = parse_day(&e.day) else {
            continue;
        };
        let dia = por_dia
            .entry(days_from_civil(y, m, d))
            .or_insert_with(|| Day {
                day: e.day.clone(),
                xp: 0,
                secs_working: 0,
                sessions: 0,
            });
        dia.xp += e.xp_gained;
        dia.secs_working += e.secs_working;
        dia.sessions += 1;
    }
    por_dia.into_values().collect()
}

/// Sequência atual (só conta se o último dia é hoje ou ontem — quebrou, zerou)
/// e o recorde histórico.
///
/// Dia com trabalho é dia com XP **ou** com segundos trabalhados; 0 XP e 0 s é
/// dia perdido: não vira `last_day` e é buraco no meio da série — quebra como
/// um dia sem registro nenhum. `today` ilegível também zera a atual: sem saber
/// que dia é, não dá pra afirmar que a sequência vive. A entrada não precisa
/// vir ordenada nem sem repetição: dia repetido é eco — conta uma vez, não
/// estende nem quebra.
pub fn streak(days: &[Day], today: &str) -> Streak {
    let mut trabalhados: Vec<(i64, &Day)> = days
        .iter()
        .filter(|d| d.xp > 0 || d.secs_working > 0)
        .filter_map(|d| {
            let (y, m, dia) = parse_day(&d.day)?;
            Some((days_from_civil(y, m, dia), d))
        })
        .collect();
    trabalhados.sort_unstable_by_key(|&(n, _)| n);

    let Some(&(ultimo_n, ultimo_dia)) = trabalhados.last() else {
        return Streak {
            current: 0,
            best: 0,
            last_day: None,
        };
    };

    // Uma passada só: ao final, `run` é a sequência que fecha no último dia —
    // exatamente a candidata a "atual". Dia repetido é eco de data que já
    // contou: pular custa nada e mantém o contrato de dias únicos sem exigir
    // que a entrada venha fundida (`by_day` funde, mas `streak` é pub).
    let mut best = 0u32;
    let mut run = 0u32;
    let mut anterior: Option<i64> = None;
    for &(n, _) in &trabalhados {
        if anterior == Some(n) {
            continue;
        }
        run = if anterior == Some(n - 1) { run + 1 } else { 1 };
        anterior = Some(n);
        best = best.max(run);
    }

    // …mas só é "atual" se o último dia com trabalho é hoje ou ontem. Buraco
    // maior (ou "hoje" que não parseia) zerou — recorde e último dia ficam.
    let hoje = parse_day(today).map(|(y, m, d)| days_from_civil(y, m, d));
    let atual = if hoje.is_some_and(|h| ultimo_n == h || ultimo_n == h - 1) {
        run
    } else {
        0
    };

    Streak {
        current: atual,
        best,
        last_day: Some(ultimo_dia.day.clone()),
    }
}

/// Dia juliano do calendário civil (algoritmo de Hinnant) — a diferença entre
/// dois deles diz se dois dias são consecutivos, sem depender de fuso.
/// Conta dias desde 1970-01-01 (antes disso, negativo). `d` parte de 1;
/// `parse_day` é o porteiro que garante entrada plausível.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    // jan/fev contam como dez/dez do ano anterior: o "ano" do algoritmo começa
    // em março, aí o 29/2 cai sempre no fim e não precisa de caso especial.
    let y = y - i64::from(m <= 2);
    // Era de 400 anos. O −399 nos anos negativos compensa a divisão do Rust
    // truncar pra zero, imitando o floor da referência.
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = (y - era * 400) as u64; // ano dentro da era, [0, 399]
    let mp = if m > 2 { m - 3 } else { m + 9 }; // mês do ano civil, março = 0
    let doy = (153 * u64::from(mp) + 2) / 5 + u64::from(d) - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe as i64 - 719468
}

/// `"2026-08-19"` → `(2026, 8, 19)`. `None` se não for uma data plausível.
/// Estrito: exige o formato canônico zero-padded (`"2026-8-19"` não passa) e
/// uma data que existe de verdade — `2025-02-29` e `2026-04-31` são rejeitados.
pub fn parse_day(s: &str) -> Option<(i64, u32, u32)> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let ano = digitos(&b[0..4])?;
    let mes = digitos(&b[5..7])?;
    let dia = digitos(&b[8..10])?;
    if !(1..=12).contains(&mes) || dia < 1 || dia > dias_no_mes(i64::from(ano), mes) {
        return None;
    }
    Some((i64::from(ano), mes, dia))
}

/// Ano bissexto no calendário gregoriano proleptico (regra 4/100/400).
fn bissexto(ano: i64) -> bool {
    ano % 4 == 0 && (ano % 100 != 0 || ano % 400 == 0)
}

/// Quantos dias o mês tem naquele ano (fevereiro depende do bissexto).
fn dias_no_mes(ano: i64, mes: u32) -> u32 {
    match mes {
        4 | 6 | 9 | 11 => 30,
        2 if bissexto(ano) => 29,
        2 => 28,
        _ => 31,
    }
}

/// Slice de bytes como número, `None` se algum não for dígito ASCII.
fn digitos(b: &[u8]) -> Option<u32> {
    let mut n = 0u32;
    for &c in b {
        if !c.is_ascii_digit() {
            return None;
        }
        n = n * 10 + u32::from(c - b'0');
    }
    Some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sessao(day: &str, xp: u64, secs: u64) -> Entry {
        Entry {
            day: day.to_string(),
            started_at: 0,
            ended_at: 0,
            xp_gained: xp,
            xp_total: xp,
            level: 1,
            agents: 1,
            secs_working: secs,
        }
    }

    fn dia(day: &str, xp: u64, secs: u64) -> Day {
        Day {
            day: day.to_string(),
            xp,
            secs_working: secs,
            sessions: 1,
        }
    }

    // --- days_from_civil ---

    #[test]
    fn epoca_1970_01_01_e_o_zero() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(1969, 12, 31), -1);
    }

    #[test]
    fn fevereiro_bissexto_tem_um_dia_a_mais() {
        // De 28/2 pra 1/3: pulo de 2 dias em ano bissexto, 1 em ano comum.
        assert_eq!(
            days_from_civil(2024, 3, 1) - days_from_civil(2024, 2, 28),
            2
        );
        assert_eq!(
            days_from_civil(2023, 3, 1) - days_from_civil(2023, 2, 28),
            1
        );
        // 2000 é bissexto de século (÷400); 1900 não é (÷100 sem ÷400).
        assert_eq!(
            days_from_civil(2000, 3, 1) - days_from_civil(2000, 2, 28),
            2
        );
        assert_eq!(
            days_from_civil(1900, 3, 1) - days_from_civil(1900, 2, 28),
            1
        );
    }

    #[test]
    fn datas_conhecidas_batem_com_o_epoch_unix() {
        // 2000-03-01T00:00:00Z = 951868800 s ÷ 86400 = 11017 dias.
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
        // 2024-01-01T00:00:00Z = 1704067200 s ÷ 86400 = 19723 dias.
        assert_eq!(days_from_civil(2024, 1, 1), 19723);
    }

    // --- parse_day ---

    #[test]
    fn parse_day_aceita_so_iso_canonico() {
        assert_eq!(parse_day("2026-08-19"), Some((2026, 8, 19)));
        assert_eq!(parse_day("2024-02-29"), Some((2024, 2, 29))); // bissexto real
    }

    #[test]
    fn parse_day_rejeita_impossivel_ou_malformado() {
        for s in [
            "2026-13-01",  // mês 13
            "2026-00-10",  // mês 0
            "2026-01-32",  // dia 32
            "2026-01-00",  // dia 0
            "2025-02-29",  // 29/2 em ano comum
            "2026-02-30",  // fevereiro não tem 30
            "2026-04-31",  // abril não tem 31
            "2026-8-19",   // sem zero à esquerda
            "2026-08-19 ", // espaço sobrando
            "19/08/2026",  // outro formato
            "abc",
            "",
        ] {
            assert_eq!(parse_day(s), None, "deveria rejeitar {s:?}");
        }
    }

    // --- by_day ---

    #[test]
    fn by_day_funde_mesmo_dia_e_ordena_cronologico() {
        let dias = by_day(&[
            sessao("2026-08-19", 50, 30),
            sessao("2026-08-18", 100, 60),
            sessao("2026-08-18", 25, 15),
            sessao("domingo", 999, 999), // lixo no diário: ignora, não infla
        ]);
        assert_eq!(
            dias,
            vec![
                Day {
                    day: "2026-08-18".into(),
                    xp: 125,
                    secs_working: 75,
                    sessions: 2
                },
                Day {
                    day: "2026-08-19".into(),
                    xp: 50,
                    secs_working: 30,
                    sessions: 1
                },
            ]
        );
    }

    // --- streak ---

    #[test]
    fn sequencia_de_3_terminando_ontem_ainda_conta() {
        let dias = vec![
            dia("2026-08-17", 100, 60),
            dia("2026-08-18", 50, 30),
            dia("2026-08-19", 10, 5),
        ];
        let s = streak(&dias, "2026-08-20"); // último dia foi ontem
        assert_eq!(s.current, 3);
        assert_eq!(s.best, 3);
        assert_eq!(s.last_day.as_deref(), Some("2026-08-19"));
    }

    #[test]
    fn sequencia_terminando_hoje_conta_do_mesmo_jeito() {
        let dias = vec![dia("2026-08-18", 50, 30), dia("2026-08-19", 10, 5)];
        assert_eq!(streak(&dias, "2026-08-19").current, 2);
    }

    #[test]
    fn ultimo_dia_anteontem_zera_atual_mas_guarda_recorde() {
        let dias = vec![
            dia("2026-08-17", 100, 60),
            dia("2026-08-18", 50, 30),
            dia("2026-08-19", 10, 5),
        ];
        let s = streak(&dias, "2026-08-21"); // buraco de 2 dias: quebrou
        assert_eq!(s.current, 0);
        assert_eq!(s.best, 3);
        assert_eq!(s.last_day.as_deref(), Some("2026-08-19"));
    }

    #[test]
    fn recorde_antigo_sobrevive_ao_declinio() {
        let dias = vec![
            dia("2026-08-01", 100, 60),
            dia("2026-08-02", 100, 60),
            dia("2026-08-03", 100, 60),
            // duas semanas de buraco
            dia("2026-08-18", 50, 30),
            dia("2026-08-19", 10, 5),
        ];
        let s = streak(&dias, "2026-08-19");
        assert_eq!(s.current, 2);
        assert_eq!(s.best, 3);
    }

    #[test]
    fn dia_zerado_nao_segura_sequencia() {
        // Dia vazio no MEIO é buraco: 17 e 19 não colam.
        let dias = vec![
            dia("2026-08-17", 100, 60),
            dia("2026-08-18", 0, 0),
            dia("2026-08-19", 10, 5),
        ];
        let s = streak(&dias, "2026-08-19");
        assert_eq!(s.current, 1);
        assert_eq!(s.best, 1);

        // Dia vazio no FIM não vira last_day: último dia COM trabalho é que é.
        let s2 = streak(
            &[dia("2026-08-18", 50, 30), dia("2026-08-19", 0, 0)],
            "2026-08-19",
        );
        assert_eq!(s2.last_day.as_deref(), Some("2026-08-18"));
        assert_eq!(s2.current, 1); // ontem trabalhou, hoje ainda não
    }

    #[test]
    fn dia_com_so_tempo_de_trabalho_conta() {
        // A regra exige os DOIS zerados pra descartar o dia: 0 XP com segundos
        // trabalhados ainda é dia com trabalho.
        let dias = vec![dia("2026-08-18", 0, 90), dia("2026-08-19", 0, 60)];
        assert_eq!(streak(&dias, "2026-08-19").current, 2);
    }

    #[test]
    fn sem_dias_nao_ha_sequencia_nem_recorde() {
        assert_eq!(
            streak(&[], "2026-08-19"),
            Streak {
                current: 0,
                best: 0,
                last_day: None
            }
        );
    }

    #[test]
    fn hoje_ilegivel_zera_atual_conservador() {
        // Sem saber que dia é hoje, não dá pra dizer se a sequência vive.
        let s = streak(&[dia("2026-08-19", 10, 5)], "sextou");
        assert_eq!(s.current, 0);
        assert_eq!(s.best, 1);
        assert_eq!(s.last_day.as_deref(), Some("2026-08-19"));
    }

    #[test]
    fn dia_duplicado_na_entrada_nao_subconta() {
        // streak é pub: pode chegar [d1, d2, d2, d3] sem passar por by_day.
        // O eco não estende nem quebra — os 3 dias distintos são 3 de sequência
        // (antes da correção o run reiniciava no repetido e devolvia 2).
        let dias = vec![
            dia("2026-08-17", 100, 60),
            dia("2026-08-18", 50, 30),
            dia("2026-08-18", 50, 30),
            dia("2026-08-19", 10, 5),
        ];
        let s = streak(&dias, "2026-08-20"); // último dia ontem: sequência viva
        assert_eq!(s.current, 3);
        assert_eq!(s.best, 3);
        assert_eq!(s.last_day.as_deref(), Some("2026-08-19"));
    }

    #[test]
    fn dia_no_futuro_nao_vira_sequencia_atual() {
        // Relógio maluco/diário adiantado: último "trabalho" é amanhã — não credita.
        let dias = vec![dia("2026-08-19", 10, 5), dia("2026-08-20", 10, 5)];
        let s = streak(&dias, "2026-08-19");
        assert_eq!(s.current, 0);
        assert_eq!(s.best, 2);
    }
}
