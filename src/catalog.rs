//! Catálogo de espécies (pixel-mons inventados) por tier, e os pesos de nascimento.
//!
//! Nomes são **provisórios** — tema "formas digitais geométricas", fáceis de
//! desenhar em LCD 1-bit. Ajustar livremente; o mapeamento tier→espécies é o que
//! importa pra a forja. Sprites vêm na Fase 4 (renderer).

use crate::pet::{Rarity, Species};

/// Pesos de nascimento (somam 100). Common mais comum, Legendary raríssimo.
/// Esta é a distribuição que cai sobre a **população** de usuários (1 pet forjado
/// por âncora GitHub).
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
