use herdr_pet::state::{load_from, save_to, PaneSeq, State};
use std::path::PathBuf;

fn tmp(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("herdr-pet-test-{}-state.json", name));
    let _ = std::fs::remove_file(&p);
    p
}

#[test]
fn state_roundtrip() {
    let path = tmp("roundtrip");
    let s = State::new(12345678);
    save_to(&path, &s).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded.github_id, 12345678);
    assert_eq!(loaded.anchor, "github:12345678");
    assert_eq!(loaded.active_index, 0);
    assert_eq!(loaded.hatched, vec![0]);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn record_hatch_is_idempotent() {
    let mut s = State::new(1);
    s.record_hatch(1);
    s.record_hatch(1); // de novo — não duplica
    assert_eq!(s.hatched, vec![0, 1]);
    assert_eq!(s.active_index, 1);
}

#[test]
fn state_antigo_sem_xp_carrega_com_zero() {
    // State salvo ANTES de o campo `xp` existir ainda carrega; xp vem como 0.
    let path = tmp("old");
    std::fs::write(
        &path,
        r#"{"anchor":"github:1","github_id":1,"active_index":0,"hatched":[0]}"#,
    )
    .unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded.github_id, 1);
    assert_eq!(loaded.xp, 0);
    assert!(loaded.last_seq_by_pane.is_empty());
    assert_eq!(loaded.level(), 1); // 0 XP → nível 1
    let _ = std::fs::remove_file(&path);
}

#[test]
fn state_com_xp_faz_roundtrip_e_deriva_nivel() {
    let mut s = State::new(7);
    s.xp = 4_500; // nível 10 pela curva acordada
    let path = tmp("xp");
    save_to(&path, &s).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded.xp, 4_500);
    assert_eq!(loaded.level(), 10);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn apply_catchup_primeira_vista_nao_credita_historico() {
    // State novo: a 1ª vista de cada pane trava baseline, sem creditar histórico.
    let mut s = State::new(1);
    let gained = s.apply_catchup(&[PaneSeq {
        pane_id: "w1:p1".into(),
        seq: 57,
    }]);
    assert_eq!(gained, 0);
    assert_eq!(s.xp, 0);
    assert_eq!(s.last_seq_by_pane.get("w1:p1"), Some(&57));
}

#[test]
fn apply_catchup_credita_delta_de_um_agente() {
    let mut s = State::new(1);
    s.apply_catchup(&[PaneSeq {
        pane_id: "w1:p1".into(),
        seq: 100,
    }]); // baseline
    let gained = s.apply_catchup(&[PaneSeq {
        pane_id: "w1:p1".into(),
        seq: 150,
    }]); // delta 50
    assert_eq!(gained, herdr_pet::progression::xp_for_catchup(50));
    assert_eq!(s.last_seq_by_pane.get("w1:p1"), Some(&150));
}

#[test]
fn apply_catchup_multiplos_agentes_aplica_curva_harmonica() {
    // 2 agentes, cada um delta 50: linear = 2×xp_for_catchup(50); curva H(2)/2 = 0,75.
    let mut s = State::new(1);
    s.apply_catchup(&[
        PaneSeq {
            pane_id: "w1:p1".into(),
            seq: 100,
        },
        PaneSeq {
            pane_id: "w2:p2".into(),
            seq: 200,
        },
    ]);
    let gained = s.apply_catchup(&[
        PaneSeq {
            pane_id: "w1:p1".into(),
            seq: 150,
        },
        PaneSeq {
            pane_id: "w2:p2".into(),
            seq: 250,
        },
    ]);
    let linear = 2 * herdr_pet::progression::xp_for_catchup(50);
    assert_eq!(gained, linear * 750 / 1000);
    assert!(gained < linear, "a curva tem que reduzir vs linear");
}

#[test]
fn apply_catchup_pane_duplicado_nao_gera_xp_fantasma() {
    // Mesmo pane 2× (ex.: pane_id ausente → ""): conta UMA vez (maior delta), sem um
    // agente ler o insert do outro dentro da mesma chamada e gerar XP fantasma.
    let mut s = State::new(1);
    s.apply_catchup(&[PaneSeq {
        pane_id: "w1:p1".into(),
        seq: 100,
    }]); // baseline
    let gained = s.apply_catchup(&[
        PaneSeq {
            pane_id: "w1:p1".into(),
            seq: 150,
        },
        PaneSeq {
            pane_id: "w1:p1".into(),
            seq: 200,
        },
    ]);
    // Só o maior delta (200−100) conta; o segundo registro não dobra nem vira fantasma.
    assert_eq!(gained, herdr_pet::progression::xp_for_catchup(100));
}

#[test]
fn record_seen_seq_avanca_baseline_sem_creditar() {
    let mut s = State::new(1);
    s.xp = 100;
    s.record_seen_seq(&[PaneSeq {
        pane_id: "w1:p1".into(),
        seq: 150,
    }]);
    assert_eq!(s.xp, 100, "record_seen_seq não credita XP");
    assert_eq!(s.last_seq_by_pane.get("w1:p1"), Some(&150));
    // catch-up seguinte só conta o delta depois da baseline
    let gained = s.apply_catchup(&[PaneSeq {
        pane_id: "w1:p1".into(),
        seq: 160,
    }]);
    assert_eq!(gained, herdr_pet::progression::xp_for_catchup(10));
}

#[test]
fn record_seen_seq_duplicado_com_seq_zero_nao_rebobina() {
    // Mesmo pane 2× no poll (API omite o campo → seq 0 por último) não pode
    // baixar a baseline; o catch-up seguinte com o seq real não paga replay.
    let mut s = State::new(1);
    s.record_seen_seq(&[PaneSeq {
        pane_id: "w1:p1".into(),
        seq: 150,
    }]);
    s.record_seen_seq(&[
        PaneSeq {
            pane_id: "w1:p1".into(),
            seq: 150,
        },
        PaneSeq {
            pane_id: "w1:p1".into(),
            seq: 0,
        },
    ]);
    assert_eq!(s.last_seq_by_pane.get("w1:p1"), Some(&150));
    let gained = s.apply_catchup(&[PaneSeq {
        pane_id: "w1:p1".into(),
        seq: 150,
    }]);
    assert_eq!(gained, 0, "seq já visto não gera XP");
    assert_eq!(s.xp, 0);
}

#[test]
fn apply_catchup_200_0_200_nao_paga_replay() {
    // Campo omitido (seq 0) no meio não rebobina; reabrir no 200 não credita de novo.
    let mut s = State::new(1);
    assert_eq!(
        s.apply_catchup(&[PaneSeq {
            pane_id: "p".into(),
            seq: 200
        }]),
        0
    );
    assert_eq!(
        s.apply_catchup(&[PaneSeq {
            pane_id: "p".into(),
            seq: 0
        }]),
        0
    );
    assert_eq!(s.last_seq_by_pane.get("p"), Some(&200));
    let gained = s.apply_catchup(&[PaneSeq {
        pane_id: "p".into(),
        seq: 200,
    }]);
    assert_eq!(gained, 0, "replay 200→0→200 não pode creditar");
    assert_eq!(s.xp, 0);
}
