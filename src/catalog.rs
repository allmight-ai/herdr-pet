//! Catálogo de espécies (pixel-mons inventados) por tier, pesos de nascimento e
//! base stats (para os stats de combate). Nomes de espécies são **provisórios**.

use crate::pet::{BaseStats, Rarity, Species};

/// Pesos de nascimento (somam 100). Common mais comum, Legendary raríssimo.
pub const RARITY_WEIGHTS: &[(Rarity, u32)] = &[
    (Rarity::Common, 60),
    (Rarity::Uncommon, 25),
    (Rarity::Rare, 10),
    (Rarity::Epic, 4),
    (Rarity::Legendary, 1),
];

/// Ordem canônica de exibição (do mais comum ao mais raro).
pub const RARITY_ORDER: &[Rarity] = &[
    Rarity::Common,
    Rarity::Uncommon,
    Rarity::Rare,
    Rarity::Epic,
    Rarity::Legendary,
];

/// Chance de shiny: 1 em `SHINY_DENOMINATOR`.
pub const SHINY_DENOMINATOR: u32 = 128;

/// Espécies disponíveis num tier.
pub fn species_for_tier(tier: Rarity) -> &'static [Species] {
    match tier {
        Rarity::Common => &[
            Species { id: "pix", name: "Pix", tier: Rarity::Common },
            Species { id: "bit", name: "Bit", tier: Rarity::Common },
            Species { id: "dot", name: "Dot", tier: Rarity::Common },
        ],
        Rarity::Uncommon => &[
            Species { id: "blok", name: "Blok", tier: Rarity::Uncommon },
            Species { id: "wav", name: "Wav", tier: Rarity::Uncommon },
            Species { id: "hex", name: "Hex", tier: Rarity::Uncommon },
        ],
        Rarity::Rare => &[
            Species { id: "glix", name: "Glix", tier: Rarity::Rare },
            Species { id: "prsm", name: "Prism", tier: Rarity::Rare },
            Species { id: "spir", name: "Spir", tier: Rarity::Rare },
        ],
        Rarity::Epic => &[
            Species { id: "frct", name: "Fract", tier: Rarity::Epic },
            Species { id: "knot", name: "Knot", tier: Rarity::Epic },
            Species { id: "vort", name: "Vort", tier: Rarity::Epic },
        ],
        Rarity::Legendary => &[
            Species { id: "aether", name: "Aether", tier: Rarity::Legendary },
            Species { id: "null", name: "Null", tier: Rarity::Legendary },
        ],
    }
}

/// Base stats por tier (provisório — calibrar por espécie depois).
/// `stat_efetivo = base + IV`; IV perfeito (31) atinge o topo (`base + 31`).
pub fn base_stats_for_tier(tier: Rarity) -> BaseStats {
    match tier {
        Rarity::Common => BaseStats { hp: 40, atk: 30, def: 30, sp_atk: 30, sp_def: 30, speed: 30, sp: 30 },
        Rarity::Uncommon => BaseStats { hp: 55, atk: 45, def: 45, sp_atk: 45, sp_def: 45, speed: 45, sp: 40 },
        Rarity::Rare => BaseStats { hp: 70, atk: 60, def: 60, sp_atk: 60, sp_def: 60, speed: 60, sp: 50 },
        Rarity::Epic => BaseStats { hp: 85, atk: 75, def: 75, sp_atk: 75, sp_def: 75, speed: 75, sp: 60 },
        Rarity::Legendary => BaseStats { hp: 100, atk: 90, def: 90, sp_atk: 90, sp_def: 90, speed: 90, sp: 75 },
    }
}
