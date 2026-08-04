//! Tipos do pet: raridade, espécie, IV e proveniência.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}

impl Rarity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Epic => "epic",
            Rarity::Legendary => "legendary",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Species {
    pub id: &'static str,
    pub name: &'static str,
    pub tier: Rarity,
}

/// Individual Values (genes de força). 4 stats, cada um 0–15 (estilo Pokémon).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct IV {
    pub hp: u8,
    pub atk: u8,
    pub def: u8,
    pub spd: u8,
}

impl IV {
    /// Soma 0–60. Útil pra ranquear a "genética" do pet.
    pub fn total(&self) -> u8 {
        self.hp + self.atk + self.def + self.spd
    }

    /// Deriva os 4 stats de um gene nomeado (4 nibbles = 2 bytes).
    pub fn from_gene(seed: &[u8], name: &str) -> Self {
        let g = crate::crypto::gene(seed, name);
        IV {
            hp: g[0] & 0x0F,
            atk: (g[0] >> 4) & 0x0F,
            def: g[1] & 0x0F,
            spd: (g[1] >> 4) & 0x0F,
        }
    }
}

/// Proveniência criptográfica — o "recibo" forjado do pet (irá pro genesis gist).
#[derive(Debug, Clone, Serialize)]
pub struct Provenance {
    pub genesis_version: u32,
    pub origin: &'static str,
    pub anchor: String,
    pub index: u32,
    pub derivation: &'static str,
    pub seed_hash: String,
}

/// Identidade forjada de um pet. (State de gameplay — fome, humor, energia — vem
/// depois; aqui só o que é determinístico e imutável.)
#[derive(Debug, Clone, Serialize)]
pub struct Pet {
    pub index: u32,
    pub rarity: Rarity,
    pub shiny: bool,
    pub species: Species,
    pub iv: IV,
    pub provenance: Provenance,
}

/// Sinônimo legível pro resultado de `hatch()`.
pub type ForgeResult = Pet;
