//! Testes da progressão (XP → nível). Costura pública: `level_for_xp`,
//! `xp_to_reach`, `Accrual`, `xp_for_catchup`, `level_view`.
//!
//! Valores esperados vêm da curva acordada: custa `100 × nível` pra subir
//! de L pra L+1; total até o nível N = 50·N·(N−1); cap no 99 (~485.100 XP).
//! Ganho: ~1000 XP/hora de trabalho acompanhado (~1 ano até o 99).

use std::time::Duration;

use herdr_pet::progression::{
    harmonic_milli, harmonic_weighted_xp, level_for_xp, level_view, xp_for_catchup, xp_to_reach,
    Accrual,
};

// --- curva ---

#[test]
fn recem_nascido_eh_nivel_1() {
    // Promessa: um pet recém-nascido, com 0 XP, está no nível 1.
    assert_eq!(level_for_xp(0), 1);
}

#[test]
fn xp_para_alcancar_nivel_1_eh_zero() {
    assert_eq!(xp_to_reach(1), 0);
}

#[test]
fn ancoras_da_curva() {
    // Total pra alcançar o nível N = 50·N·(N−1), da curva acordada com o usuário.
    assert_eq!(xp_to_reach(2), 100);
    assert_eq!(xp_to_reach(3), 300);
    assert_eq!(xp_to_reach(6), 1_500);
    assert_eq!(xp_to_reach(10), 4_500);
    assert_eq!(xp_to_reach(99), 485_100);
}

#[test]
fn level_para_xp_nas_ancoras() {
    // Inverso: dado o XP total, qual o nível (limite inferior incluso).
    assert_eq!(level_for_xp(99), 1); // ainda não bateu 100
    assert_eq!(level_for_xp(100), 2);
    assert_eq!(level_for_xp(300), 3);
    assert_eq!(level_for_xp(4_500), 10);
    assert_eq!(level_for_xp(485_099), 98); // um abaixo do teto
    assert_eq!(level_for_xp(485_100), 99); // exatamente no teto
}

#[test]
fn saturacao_no_nivel_99() {
    // XP absurdo não passa de 99.
    assert_eq!(level_for_xp(u64::MAX), 99);
    assert_eq!(level_for_xp(1_000_000_000), 99);
}

#[test]
fn mais_xp_nunca_diminui_o_nivel() {
    // Monotônico: mais XP nunca dá nível menor.
    let mut anterior = level_for_xp(0);
    for xp in (1..=485_100).step_by(97) {
        let atual = level_for_xp(xp);
        assert!(
            atual >= anterior,
            "nível diminuiu em xp={xp}: {atual} < {anterior}"
        );
        anterior = atual;
    }
}

#[test]
fn cada_nivel_pede_mais_xp_que_o_anterior() {
    // Acelerante: o custo de cada transição cresce estritamente.
    let mut custo_anterior = 0u64;
    for nivel in 2..=99u8 {
        let custo = xp_to_reach(nivel) - xp_to_reach(nivel - 1);
        assert!(
            custo > custo_anterior,
            "transição parou de crescer no nível {nivel}"
        );
        custo_anterior = custo;
    }
}

// --- earning: acompanhado (Accrual) ---

#[test]
fn accrual_zero_por_zero_tempo() {
    let mut a = Accrual::new();
    assert_eq!(a.add_working(Duration::ZERO, 1000), 0);
}

#[test]
fn accrual_bate_mil_xp_por_hora_acompanhando() {
    // 1 agente (mult 1000) · 1 hora = 1000 XP.
    let mut a = Accrual::new();
    assert_eq!(a.add_working(Duration::from_secs(3600), 1000), 1000);
}

#[test]
fn accrual_respeita_multiplicador_de_largura() {
    // 2 agentes working → H(2)=1500 · 1 hora = 1500 XP (1,5× o base, não 2×).
    let mut a = Accrual::new();
    assert_eq!(a.add_working(Duration::from_secs(3600), 1500), 1500);
}

#[test]
fn accrual_acumula_sub_xp_sem_perder() {
    let mut a = Accrual::new();
    let mut total = 0u64;
    for _ in 0..45 {
        // 45 × 0,8s = 36s → a 1000 XP/hora = 10 XP
        total += a.add_working(Duration::from_millis(800), 1000);
    }
    assert_eq!(total, 10);
}

// --- earning: catch-up (pane fechado, via state_change_seq) ---

#[test]
fn catchup_cresce_e_teto() {
    assert_eq!(xp_for_catchup(0), 0);
    assert!(
        xp_for_catchup(5) < xp_for_catchup(50),
        "deve crescer com delta"
    );
    // Teto: deltas gigantes não inflacionam (granularidade do seq é incerta).
    assert_eq!(xp_for_catchup(10_000), xp_for_catchup(100_000));
}

// --- view: progresso dentro do nível ---

#[test]
fn level_view_no_nivel_1() {
    let v = level_view(0);
    assert_eq!((v.level, v.xp_into, v.xp_span), (1, 0, 100));
}

#[test]
fn level_view_ao_bater_nivel_2() {
    let v = level_view(100);
    assert_eq!((v.level, v.xp_into, v.xp_span), (2, 0, 200));
}

#[test]
fn level_view_no_meio_do_nivel() {
    let v = level_view(250); // nível 2: base 100, span 200 → 150 dentro
    assert_eq!((v.level, v.xp_into, v.xp_span), (2, 150, 200));
}

#[test]
fn level_view_no_topo() {
    assert_eq!((level_view(485_100).level, 0u64, 0u64), (99u8, 0, 0));
    let v = level_view(u64::MAX);
    assert_eq!((v.level, v.xp_into, v.xp_span), (99, 0, 0));
}

// --- curva de largura: decaimento harmônico por nº de agentes ---

#[test]
fn harmonic_um_agente_e_ritmo_cheio() {
    assert_eq!(harmonic_milli(0), 0);
    assert_eq!(harmonic_milli(1), 1000); // 1 agente = 1×, sem penalidade
}

#[test]
fn harmonic_cresce_sublinearmente() {
    // H(n)×1000: 1 + 1/2 + 1/3 + … — cresce, mas cada novo agente adiciona menos.
    assert_eq!(harmonic_milli(2), 1500);
    assert_eq!(harmonic_milli(3), 1833);
    assert_eq!(harmonic_milli(5), 2283);
}

#[test]
fn harmonic_satura_e_eh_monotono() {
    assert_eq!(harmonic_milli(100), harmonic_milli(8)); // teto da tabela
    let mut anterior = 0u64;
    for n in 0..=20usize {
        let v = harmonic_milli(n);
        assert!(v >= anterior, "deixou de ser monótono em n={n}");
        anterior = v;
    }
}

#[test]
fn harmonic_weighted_iguais_bate_h_sobre_n() {
    // 2×150: 150 + 75 = 225 = 300 × H(2)/2.
    let mut g = [150, 150];
    assert_eq!(harmonic_weighted_xp(&mut g), 225);
    let mut one = [80];
    assert_eq!(harmonic_weighted_xp(&mut one), 80);
    assert_eq!(harmonic_weighted_xp(&mut []), 0);
}

#[test]
fn harmonic_weighted_tick_nao_taxa_o_maior() {
    let mut g = [300, 3];
    assert_eq!(harmonic_weighted_xp(&mut g), 301);
}
