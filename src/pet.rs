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

/// Individual Values (genes de força), padrão Pokémon: 6 stats de 0–31.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct IV {
    pub hp: u8,
    pub atk: u8,
    pub def: u8,
    pub sp_atk: u8,
    pub sp_def: u8,
    pub speed: u8,
}

impl IV {
    /// Soma 0–186. Útil pra ranquear a "genética" do pet.
    pub fn total(&self) -> u16 {
        self.hp as u16
            + self.atk as u16
            + self.def as u16
            + self.sp_atk as u16
            + self.sp_def as u16
            + self.speed as u16
    }

    /// Deriva os 6 stats de um gene nomeado (`byte % 32` → 0–31, distribuição uniforme).
    pub fn from_gene(seed: &[u8], name: &str) -> Self {
        let g = crate::crypto::gene(seed, name);
        let s = |i: usize| g[i] % 32;
        IV {
            hp: s(0),
            atk: s(1),
            def: s(2),
            sp_atk: s(3),
            sp_def: s(4),
            speed: s(5),
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
    pub name: String,
    pub rarity: Rarity,
    pub shiny: bool,
    pub species: Species,
    pub iv: IV,
    pub provenance: Provenance,
}

/// Sinônimo legível pro resultado de `hatch()`.
pub type ForgeResult = Pet;
