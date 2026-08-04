//! Sprites 1-bit (pixel art em block chars Unicode) por espécie.
//!
//! Provisórios — formas geométricas simples, fáceis de refinar depois. Cada sprite
//! é um slice de linhas de igual largura (idealmente 9 chars).

pub fn sprite_for(species_id: &str) -> &'static [&'static str] {
    match species_id {
        "pix" => &[
            "    ▀▀▀    ",
            "   ▟███▙   ",
            "   ▜███▛   ",
            "    ▄▄▄    ",
        ],
        "bit" => &[
            "  ▄▄▄▄▄▄▄  ",
            "  ▟█████▙  ",
            "  ▜█████▛  ",
            "  ▀▀▀▀▀▀▀  ",
        ],
        "dot" => &[
            "    ▄▄▄    ",
            "   ▟   ▙   ",
            "   ▜   ▛   ",
            "    ▀▀▀    ",
        ],
        "blok" => &[
            "  ▄▄▄▄▄▄▄  ",
            "  ▟▀▀▀▀▀▙  ",
            "  ▜▄▄▄▄▄▛  ",
            "  ▀▀▀▀▀▀▀  ",
        ],
        "wav" => &[
            "   ▄ ▀ ▄   ",
            "  ▀ ▄ ▀ ▄  ",
            "   ▄ ▀ ▄   ",
            "  ▀ ▄ ▀ ▄  ",
        ],
        "hex" => &[
            "    ▄▄▄    ",
            "   ▟   ▙   ",
            "   ▜   ▛   ",
            "    ▀▀▀    ",
        ],
        "glix" => &[
            "  ▄▀ ▄▀ ▄  ",
            "  ▟▙▄▀▟▙   ",
            "  ▜▛▀▄▜▛   ",
            "  ▀ ▄▀ ▀▄  ",
        ],
        "prsm" => &[
            "     ◆     ",
            "    ▟▀▙    ",
            "    ▜▄▛    ",
            "     ◆     ",
        ],
        "spir" => &[
            "    ▄▄▄    ",
            "   ▟▀▀▙    ",
            "   ▜▄▄▛    ",
            "    ▀▀▀    ",
        ],
        "frct" => &[
            "    ▄▀▄    ",
            "   ▟▀▄▀▙   ",
            "   ▜▄▀▄▛   ",
            "    ▀▄▀    ",
        ],
        "knot" => &[
            "   ▄▀ ▀▄   ",
            "  ▟ ▀▀▀ ▙  ",
            "  ▜ ▄▄▄ ▛  ",
            "   ▀▄ ▄▀   ",
        ],
        "vort" => &[
            "    ◯◯     ",
            "   ▟◉◉◉▙   ",
            "   ▜◉◉◉▛   ",
            "    ◯◯     ",
        ],
        "aether" => &[
            "    ✦▄✦    ",
            "   ▟▀▀▀▙   ",
            "   ▜▄▄▄▛   ",
            "    ✧▀✧    ",
        ],
        "null" => &[
            "  ▌     ▐  ",
            "  ▌  ◌  ▐  ",
            "  ▌     ▐  ",
            "  ▀▀▀▀▀▀▀  ",
        ],
        _ => &[
            "    ???    ",
            "   ▟ ? ▙   ",
            "   ▜ ? ▛   ",
            "    ???    ",
        ],
    }
}
