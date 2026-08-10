use herdr_pet::state::{load_from, save_to, State};
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
    assert_eq!(loaded.last_state_change_seq, 0);
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
fn apply_catchup_primeira_observacao_nao_credita_historico() {
    // State novo/migrado (last_seq == 0): a 1ª observação trava a baseline,
    // sem conceder XP pelo trabalho histórico do agente. O pet cresce daqui p/ frente.
    let mut s = State::new(1);
    let gained = s.apply_catchup(57);
    assert_eq!(gained, 0);
    assert_eq!(s.xp, 0);
    assert_eq!(s.last_state_change_seq, 57); // baseline travada
}

#[test]
fn apply_catchup_credita_delta_e_atualiza_seq() {
    // Observações seguintes: XP pelo delta desde a última vez, e seq avança.
    let mut s = State::new(1);
    s.apply_catchup(100); // baseline
    let gained = s.apply_catchup(150); // delta de 50
    assert_eq!(gained, herdr_pet::progression::xp_for_catchup(50));
    assert_eq!(s.last_state_change_seq, 150);
}
