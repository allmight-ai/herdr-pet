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
    // Ganhos iguais: Σ g/i = linear·H(2)/2 = o valor antigo (225).
    assert_eq!(gained, linear * 750 / 1000);
    assert!(gained < linear, "a curva tem que reduzir vs linear");
}

#[test]
fn apply_catchup_tick_pequeno_nao_taxa_o_worker() {
    // C7: worker delta 100 (300 XP) + idle delta 1 (3 XP) não pode virar 227.
    // Peso por ganho, maior primeiro: 300×1 + 3×½ = 301.
    let mut s = State::new(1);
    s.apply_catchup(&[
        PaneSeq {
            pane_id: "worker".into(),
            seq: 10,
        },
        PaneSeq {
            pane_id: "idle".into(),
            seq: 1,
        },
    ]);
    let gained = s.apply_catchup(&[
        PaneSeq {
            pane_id: "worker".into(),
            seq: 110,
        },
        PaneSeq {
            pane_id: "idle".into(),
            seq: 2,
        },
    ]);
    let worker = herdr_pet::progression::xp_for_catchup(100);
    let idle = herdr_pet::progression::xp_for_catchup(1);
    assert_eq!(worker, 300);
    assert_eq!(idle, 3);
    assert_eq!(gained, 301, "300 + 3/2, não 227 da taxa H(2)/n na soma");
    assert!(gained >= worker, "o worker não pode perder XP pro flicker");
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
fn record_seen_seq_mesmo_tick_fica_com_o_maior() {
    // Dois valores no mesmo slice: o maior é o observado deste tick.
    // O 0 *espúrio* (campo omitido) não chega aqui — snapshot descarta.
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
    assert_eq!(gained, 0, "seq já visto neste teto não gera XP");
    assert_eq!(s.xp, 0);
}

#[test]
fn apply_catchup_reset_genuino_rebobina_sem_creditar_nesse_tick() {
    // 0 real (Herdr reiniciou / pane_id reusado) rebobina a baseline.
    // Este tick: 0 XP. O próximo delta conta a partir do novo zero.
    // Replay de campo omitido (200→ausente→200) não acontece: o snapshot
    // não emite PaneSeq sem seq, então o 0 espúrio nunca chega aqui.
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
        0,
        "reset não credita neste tick"
    );
    assert_eq!(s.last_seq_by_pane.get("p"), Some(&0));
    let gained = s.apply_catchup(&[PaneSeq {
        pane_id: "p".into(),
        seq: 40,
    }]);
    assert_eq!(gained, herdr_pet::progression::xp_for_catchup(40));
    assert_eq!(s.last_seq_by_pane.get("p"), Some(&40));
}

// --- C9: corrupção preservada, save atômico ---

#[test]
fn state_truncado_vira_none_e_preserva_corrupt() {
    // Ilegível ⇒ None (auto-init pode recriar), MAS o conteúdo é preservado em
    // `<path>.corrupt` antes de qualquer coisa — nunca destruído em silêncio.
    let path = tmp("corrupt");
    let original =
        r#"{"anchor":"github:1","github_id":1,"active_index":0,"hatched":[0],"xp":48000,"last"#;
    std::fs::write(&path, original).unwrap();
    assert!(load_from(&path).is_none());
    let corrupt = PathBuf::from(format!("{}.corrupt", path.display()));
    assert!(corrupt.is_file(), "cópia .corrupt criada");
    assert_eq!(
        std::fs::read_to_string(&corrupt).unwrap(),
        original,
        "cópia .corrupt com o conteúdo original"
    );
    // Segunda carga do mesmo arquivo ilegível: empilha (.corrupt.1), não sobrescreve.
    assert!(load_from(&path).is_none());
    let second = PathBuf::from(format!("{}.corrupt.1", path.display()));
    assert!(
        second.is_file(),
        "segunda preserva empilha sem sobrescrever"
    );
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&corrupt);
    let _ = std::fs::remove_file(&second);
}

#[test]
fn save_to_nao_deixa_tmp_e_produz_arquivo_valido() {
    // Atômico (tmp+fsync+rename): depois do save não sobra tmp e o arquivo carrega.
    let path = tmp("atomic");
    let s = State::new(3);
    save_to(&path, &s).unwrap();
    assert!(
        !path.with_extension("tmp-herdr-pet").exists(),
        "sem tmp lixo"
    );
    assert_eq!(load_from(&path).unwrap().github_id, 3);
    let _ = std::fs::remove_file(&path);
}
