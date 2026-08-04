//! Gerador de nome próprio (apelido) determinístico por pet.
//!
//! Nome legível de **3 sílabas** (prefixo + meio + sufixo) + sufixo curto do
//! seed_hash (unicidade na prática). Com o vocabulário abaixo há ~48.000
//! combinações legíveis; o sufixo garante que dois pets nunca compartilhem o
//! nome completo (só colidiria com bilhões de pets).

use crate::crypto::gene;

const PREFIXES: &[&str] = &[
    "Vex", "Zor", "Kry", "Nyx", "Gli", "Blu", "Orl", "Wis", "Vol", "Qua",
    "Zep", "Mor", "Tor", "Lum", "Daz", "Cyr", "Fae", "Ryn", "Pix", "Hex",
    "Ael", "Bor", "Cyl", "Dex", "Eth", "Fyr", "Gry", "Hol", "Iox", "Jor",
    "Kha", "Lyr", "Myr", "Ory", "Pra", "Syl", "Tha", "Vor", "Xan", "Zen",
];

const MIDDLES: &[&str] = &[
    "a", "e", "i", "o", "u", "ae", "el", "il", "ol", "ul",
    "ar", "er", "ir", "or", "ur", "al", "em", "im", "om", "an",
    "en", "in", "on", "un", "ax", "ex", "ix", "ox", "la", "na",
];

const SUFFIXES: &[&str] = &[
    "ix", "ax", "ox", "on", "um", "eth", "ar", "in", "yl", "us",
    "or", "en", "is", "ai", "ek", "yn", "ir", "av", "im", "os",
    "ur", "eb", "id", "ob", "ad", "ub", "ed", "ald", "ild", "old",
    "uld", "arn", "orn", "urn", "ion", "ius", "uin", "oth", "yrn", "ael",
];

/// Quantas combinações legíveis existem (~48.000). Útil pra expor/documentar.
pub fn readable_space() -> usize {
    PREFIXES.len() * MIDDLES.len() * SUFFIXES.len()
}

fn readable(pet_seed: &[u8; 32]) -> String {
    let g = gene(pet_seed, "name");
    let pre = PREFIXES[g[0] as usize % PREFIXES.len()];
    let mid = MIDDLES[g[1] as usize % MIDDLES.len()];
    let suf = SUFFIXES[g[2] as usize % SUFFIXES.len()];
    format!("{}{}{}", pre, mid, suf)
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
        assert!(n.len() >= 5 && n.len() <= 12, "nome legível estranho: {}", n);
    }

    #[test]
    fn readable_space_is_large() {
        assert!(readable_space() >= 40_000, "espaço legível pequeno: {}", readable_space());
    }
}
