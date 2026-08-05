//! Forja determinística: identidade do pet a partir da âncora (ID GitHub) e do índice.

use crate::catalog::{base_stats_for_tier, species_for_tier, RARITY_WEIGHTS, SHINY_DENOMINATOR};
use crate::crypto::{gene, root_seed, GENESIS_VERSION};
use crate::pet::{CombatStats, IV, Pet, Provenance, Rarity, Species};

/// GitHub ID do autor — easter egg do tier Primordial no pet #0.
pub const FREDERICO_ID: u64 = 76918723;

/// Forma canônica da âncora: `"github:<id>"`.
pub fn anchor_for(github_id: u64) -> String {
    format!("github:{}", github_id)
}

/// Deriva um u64 determinístico de um gene nomeado (8 bytes mais significativos).
fn u64_of(pet_seed: &[u8; 32], name: &str) -> u64 {
    let g = gene(pet_seed, name);
    u64::from_be_bytes(g[..8].try_into().unwrap())
}

/// Sorteio ponderado determinístico (NÃO usa RNG aleatório — é a âncora que decide).
fn roll_weighted(entropy: u64) -> Rarity {
    let total: u64 = RARITY_WEIGHTS.iter().map(|(_, w)| *w as u64).sum();
    let r = entropy % total;
    let mut acc = 0u64;
    for (tier, w) in RARITY_WEIGHTS {
        acc += *w as u64;
        if r < acc {
            return *tier;
        }
    }
    Rarity::Legendary
}

/// Escolhe a espécie dentro do tier sorteado (determinístico).
fn pick_species(tier: Rarity, entropy: u64) -> Species {
    let list = species_for_tier(tier);
    list[(entropy as usize) % list.len()]
}

/// Forja o pet de índice `index` para a âncora dada.
///
/// **Idempotente**: mesma âncora + mesmo índice => mesmo pet, sempre. Stats de
/// combate (HP/SP/atk/...) = `base + IV`, então IV perfeito (31) atinge o topo.
pub fn hatch(github_id: u64, index: u32) -> Pet {
    let anchor = anchor_for(github_id);
    let root = root_seed(&anchor);
    let pet_seed = gene(&root, &format!("pet:{}", index));

    // Easter egg: o pet #0 do criador é um Primordial shiny exclusivo (cor iridescente).
    let (rarity, shiny) = if github_id == FREDERICO_ID && index == 0 {
        (Rarity::Primordial, true)
    } else {
        (
            roll_weighted(u64_of(&pet_seed, "rarity")),
            u64_of(&pet_seed, "shiny") % SHINY_DENOMINATOR as u64 == 0,
        )
    };
    let species = pick_species(rarity, u64_of(&pet_seed, "species"));
    let iv = IV::from_gene(&pet_seed, "iv");
    let name = crate::name::pet_name(&pet_seed);

    // Stats de combate: base (por tier) + IV. IV perfeito (31) => topo.
    let base = base_stats_for_tier(rarity);
    let sp_iv = (u64_of(&pet_seed, "sp_iv") % 32) as u8;
    let stats = CombatStats::from(&base, &iv, sp_iv);

    Pet {
        index,
        name,
        rarity,
        shiny,
        species,
        iv,
        stats,
        provenance: Provenance {
            genesis_version: GENESIS_VERSION,
            origin: "herdr-pet",
            anchor,
            index,
            derivation: "hmac-sha256-subseed",
            seed_hash: hex::encode(pet_seed),
        },
    }
}
