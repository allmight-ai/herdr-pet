//! Primitivas determinísticas: HMAC-SHA256 e derivação por sub-seed.
//!
//! `gene(seed, name)` deriva 32 bytes determinísticos a partir de `(seed, nome)`.
//! Como cada gene depende só da semente + do seu próprio nome, **adicionar um gene
//! novo nunca altera um gene existente** (princípio BIP-32) — é o que torna a
//! "reserva" de genes ilimitada e gratuita. O algoritmo é mutável; os nascimentos
//! são imutáveis (ver `GENESIS_VERSION`).

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Versão do algoritmo de derivação. Bump **só** se mudar o MAPEAMENTO de um gene
/// existente. Adicionar genes novos é grátis (sub-seed por nome) e não exige bump.
pub const GENESIS_VERSION: u32 = 1;

/// Segredo do app (embarcado no binário). Mudar o salt re-deriva TODOS os pets de
/// forma diferente => bump global de `GENESIS_VERSION`. Não mudar levianamente.
pub const APP_SALT: &[u8] = b"herdr-pet/v1/anchor-derivation-salt";

/// HMAC-SHA256 com `key = seed` e `msg = data`.
pub fn hmac_sha256(seed: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(seed).expect("HMAC aceita chave de qualquer tamanho");
    mac.update(data);
    let bytes = mac.finalize().into_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    out
}

/// Deriva 32 bytes determinísticos para uma feature nomeada.
///
/// Exemplos: `gene(seed, "rarity")`, `gene(seed, "pet:3")`, `gene(seed, "iv")`.
/// Cada nome produz um espaço independente — nunca cola nem esgota.
pub fn gene(seed: &[u8], name: &str) -> [u8; 32] {
    hmac_sha256(seed, name.as_bytes())
}

/// Semente raiz do usuário a partir da âncora (`"github:<id>"`).
pub fn root_seed(anchor: &str) -> [u8; 32] {
    hmac_sha256(APP_SALT, anchor.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gene_is_deterministic() {
        let s = root_seed("github:42");
        assert_eq!(gene(&s, "rarity"), gene(&s, "rarity"));
    }

    #[test]
    fn different_names_yield_different_genes() {
        let s = root_seed("github:42");
        assert_ne!(gene(&s, "rarity"), gene(&s, "shiny"));
        assert_ne!(gene(&s, "iv"), gene(&s, "species"));
    }

    #[test]
    fn different_anchors_yield_different_roots() {
        assert_ne!(root_seed("github:1"), root_seed("github:2"));
    }
}
