//! herdr-pet — núcleo de forja e espelho do agente.
//!
//! Raridade forjada (não sorteada): derivada do ID numérico do GitHub.
//! Cada pet é `HMAC(root_seed, "pet:N")` (sub-seeds estilo BIP-32).
//! Apagar o state e re-rodar rederiva o mesmo pet.

pub mod agent;
pub mod anchor;
pub mod catalog;
pub mod crypto;
pub mod forge;
pub mod name;
pub mod pet;
pub mod render;
pub mod setup;
pub mod sprites;
pub mod state;

pub use agent::{AgentInfo, AgentStatus};
pub use catalog::{base_stats_for_tier, species_for_tier, RARITY_ORDER, RARITY_WEIGHTS, SHINY_DENOMINATOR};
pub use crypto::{gene, hmac_sha256, root_seed, APP_SALT, GENESIS_VERSION};
pub use name::pet_name;
pub use forge::{anchor_for, hatch};
pub use pet::{BaseStats, CombatStats, ForgeResult, IV, Pet, Provenance, Rarity, Species};
