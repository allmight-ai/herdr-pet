use herdr_pet::{hatch, Rarity};
use std::collections::HashMap;

#[test]
fn idempotent_same_anchor_and_index() {
    // A promessa central: mesma âncora + mesmo índice => sempre o mesmo pet.
    // (Equivalente a "apagar o state e rodar de novo nasce o mesmo pet".)
    let a = hatch(12345678, 0);
    let b = hatch(12345678, 0);
    assert_eq!(a.species.id, b.species.id);
    assert_eq!(a.rarity, b.rarity);
    assert_eq!(a.shiny, b.shiny);
    assert_eq!(a.iv, b.iv);
    assert_eq!(a.provenance.seed_hash, b.provenance.seed_hash);
}

#[test]
fn different_indices_yield_different_pets() {
    // Índices diferentes → pets diferentes.
    let a = hatch(12345678, 0);
    let b = hatch(12345678, 1);
    assert_ne!(a.provenance.seed_hash, b.provenance.seed_hash);
    assert!(
        a.species.id != b.species.id || a.rarity != b.rarity || a.iv != b.iv || a.shiny != b.shiny,
        "pets de índices diferentes colidiram"
    );
}

#[test]
fn species_tier_matches_rolled_rarity() {
    for id in [1u64, 2, 3, 999, 12345678] {
        let pet = hatch(id, 0);
        assert_eq!(pet.species.tier, pet.rarity);
    }
}

#[test]
fn iv_stats_within_range() {
    for id in 0..500u64 {
        let pet = hatch(id, 0);
        for v in [
            pet.iv.hp,
            pet.iv.atk,
            pet.iv.def,
            pet.iv.sp_atk,
            pet.iv.sp_def,
            pet.iv.speed,
        ] {
            assert!(v <= 31, "iv stat fora do range 0..=31: {}", v);
        }
    }
}

#[test]
fn distribution_approximates_weights() {
    // A distribuição dos pesos (60/25/10/4/1) cai sobre a população de âncoras.
    let n = 6000u64;
    let mut counts: HashMap<Rarity, u64> = HashMap::new();
    for id in 0..n {
        let pet = hatch(id, 0);
        *counts.entry(pet.rarity).or_default() += 1;
    }
    let frac = |r: Rarity| *counts.get(&r).unwrap_or(&0) as f64 / n as f64;

    let common = frac(Rarity::Common);
    assert!(common > 0.54 && common < 0.66, "common = {:.3}", common);
    let rare = frac(Rarity::Rare);
    assert!(rare > 0.05 && rare < 0.15, "rare = {:.3}", rare);
    let legendary = frac(Rarity::Legendary);
    assert!(legendary < 0.03, "legendary = {:.3}", legendary);
}

#[test]
fn pet_name_is_deterministic() {
    assert_eq!(hatch(42, 0).name, hatch(42, 0).name);
}

#[test]
fn pet_names_are_unique_in_practice() {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    for id in 0..1000u64 {
        let pet = hatch(id, 0);
        assert!(
            seen.insert(pet.name.clone()),
            "nome duplicado: {}",
            pet.name
        );
    }
}

#[test]
fn combat_stats_are_base_plus_iv() {
    // stat_efetivo = base + IV. IV perfeito (31) atinge o topo (base + 31).
    use herdr_pet::base_stats_for_tier;
    for id in 0..50u64 {
        let pet = hatch(id, 0);
        let base = base_stats_for_tier(pet.rarity);
        assert_eq!(pet.stats.hp_max, base.hp + pet.iv.hp as u16);
        assert_eq!(pet.stats.atk, base.atk + pet.iv.atk as u16);
        assert_eq!(pet.stats.def, base.def + pet.iv.def as u16);
        assert_eq!(pet.stats.sp_atk, base.sp_atk + pet.iv.sp_atk as u16);
        assert_eq!(pet.stats.sp_def, base.sp_def + pet.iv.sp_def as u16);
        assert_eq!(pet.stats.speed, base.speed + pet.iv.speed as u16);
    }
}
