//! A forja: deriva deterministicamente a identidade de um pet a partir da âncora
//! (ID GitHub) e do índice (0 = nascimento; 1+ = renascimentos / novos pets da
//! coleção).

use crate::catalog::{species_for_tier, RARITY_WEIGHTS, SHINY_DENOMINATOR};
use crate::crypto::{gene, root_seed, GENESIS_VERSION};
use crate::pet::{IV, Pet, Provenance, Rarity, Species};

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
    // Inalcançável (r < total), mas o compilador exige um retorno.
    Rarity::Legendary
}

/// Escolhe a espécie dentro do tier sorteado (determinístico).
fn pick_species(tier: Rarity, entropy: u64) -> Species {
    let list = species_for_tier(tier);
    list[(entropy as usize) % list.len()]
}

/// Forja o pet de índice `index` para a âncora dada.
///
/// **Idempotente**: mesma âncora + mesmo índice => mesmo pet, sempre. A raridade
/// nunca esteve no disco — é sempre re-derivável. Índices diferentes = pets
/// diferentes (a base da coleção / renascimento).
pub fn hatch(github_id: u64, index: u32) -> Pet {
    let anchor = anchor_for(github_id);
    let root = root_seed(&anchor);
    let pet_seed = gene(&root, &format!("pet:{}", index));

    let rarity = roll_weighted(u64_of(&pet_seed, "rarity"));
    let shiny = u64_of(&pet_seed, "shiny") % SHINY_DENOMINATOR as u64 == 0;
    let species = pick_species(rarity, u64_of(&pet_seed, "species"));
    let iv = IV::from_gene(&pet_seed, "iv");

    Pet {
        index,
        rarity,
        shiny,
        species,
        iv,
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
