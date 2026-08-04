//! Tipos do pet: raridade, espécie, IV, stats de combate e proveniência.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
    Primordial, // acima de legendary; exclusivo (easter egg), não entra nos pesos
}

impl Rarity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Rarity::Common => "common",
            Rarity::Uncommon => "uncommon",
            Rarity::Rare => "rare",
            Rarity::Epic => "epic",
            Rarity::Legendary => "legendary",
            Rarity::Primordial => "primordial",
        }
    }

    /// Nome de exibição capitalizado (Common, …, Primordial). `as_str` é o id minúsculo.
    pub fn as_title(&self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::Epic => "Epic",
            Rarity::Legendary => "Legendary",
            Rarity::Primordial => "Primordial",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Species {
    pub id: &'static str,
    pub name: &'static str,
    pub tier: Rarity,
}

/// IV (genes de força), padrão Pokémon: 6 stats de 0–31.
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

    /// Deriva os 6 stats de um gene nomeado (`byte % 32` → 0–31, uniforme).
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

/// Base stats de uma espécie (por tier, por enquanto — calibrar por espécie depois).
/// O IV perfeito (31) atinge o topo: `stat_efetivo = base + IV`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BaseStats {
    pub hp: u16,
    pub atk: u16,
    pub def: u16,
    pub sp_atk: u16,
    pub sp_def: u16,
    pub speed: u16,
    pub sp: u16, // stamina base (recurso pra usar skills)
}

/// Stats de combate efetivos (capacidade máxima), forjados = `base + IV`.
/// HP/SP *atuais* + nível são state de gameplay (decai em batalha, regenera, sobe com XP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CombatStats {
    pub hp_max: u16,
    pub sp_max: u16,
    pub atk: u16,
    pub def: u16,
    pub sp_atk: u16,
    pub sp_def: u16,
    pub speed: u16,
}

impl CombatStats {
    /// `stat_efetivo = base + IV`. IV perfeito (31) → topo (`base + 31`).
    pub fn from(base: &BaseStats, iv: &IV, sp_iv: u8) -> Self {
        CombatStats {
            hp_max: base.hp + iv.hp as u16,
            atk: base.atk + iv.atk as u16,
            def: base.def + iv.def as u16,
            sp_atk: base.sp_atk + iv.sp_atk as u16,
            sp_def: base.sp_def + iv.sp_def as u16,
            speed: base.speed + iv.speed as u16,
            sp_max: base.sp + sp_iv as u16,
        }
    }
}

/// Proveniência criptográfica — o "recibo" forjado do pet (irá pro genesis log/gist).
#[derive(Debug, Clone, Serialize)]
pub struct Provenance {
    pub genesis_version: u32,
    pub origin: &'static str,
    pub anchor: String,
    pub index: u32,
    pub derivation: &'static str,
    pub seed_hash: String,
}

/// Identidade forjada de um pet. (HP/SP *atuais* + nível vêm com o state, depois.)
#[derive(Debug, Clone, Serialize)]
pub struct Pet {
    pub index: u32,
    pub name: String,
    pub rarity: Rarity,
    pub shiny: bool,
    pub species: Species,
    pub iv: IV,
    pub stats: CombatStats,
    pub provenance: Provenance,
}

/// Sinônimo legível pro resultado de `hatch()`.
pub type ForgeResult = Pet;
