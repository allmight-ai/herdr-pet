//! Gerador de nome próprio (apelido) determinístico por pet.
//!
//! O nome é **legível** (sílabas de um vocabulário "digital/geométrico") e **único
//! na prática**: um sufixo curto derivado do seed_hash é anexado. Como cada
//! `(âncora, índice)` produz um seed_hash diferente, o nome completo dificilmente
//! se repete. (Unicidade *absoluta* exigiria o hash inteiro; o sufixo de 8 chars
//! só colidiria com bilhões de pets — impossível na prática.)

use crate::crypto::gene;

const PREFIXES: &[&str] = &[
    "Vex", "Zor", "Kry", "Nyx", "Gli", "Blu", "Orl", "Wis", "Vol", "Qua",
    "Zep", "Mor", "Tor", "Lum", "Daz", "Cyr", "Fae", "Ryn", "Pix", "Hex",
];

const SUFFIXES: &[&str] = &[
    "ix", "ax", "on", "um", "eth", "ar", "in", "ox", "yl", "us",
    "or", "en", "is", "ai", "ek", "yn", "oth", "ir", "av", "uun",
];

fn readable(pet_seed: &[u8; 32]) -> String {
    let g = gene(pet_seed, "name");
    let pre = PREFIXES[g[0] as usize % PREFIXES.len()];
    let suf = SUFFIXES[g[1] as usize % SUFFIXES.len()];
    format!("{}{}", pre, suf)
}

/// Nome completo: parte legível + sufixo curto (8 chars) do seed_hash.
pub fn pet_name(pet_seed: &[u8; 32]) -> String {
    let tag = &hex::encode(pet_seed)[..8];
    format!("{}·{}", readable(pet_seed), tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::root_seed;

    #[test]
    fn readable_part_uses_vocab() {
        let s = root_seed("github:42");
        let n = readable(&gene(&s, "pet:0"));
        assert!(n.len() >= 4 && n.len() <= 7, "nome legível estranho: {}", n);
    }
}
