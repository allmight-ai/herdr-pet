use clap::{Parser, Subcommand};
use herdr_pet::{hatch, GENESIS_VERSION};

#[derive(Parser)]
#[command(name = "herdr-pet", version, about = "Companion V-Pet do Herdr — raridade forjada")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Inicializa o companion: resolve seu GitHub, trava a âncora (lock-in) e choça o pet #0.
    Init,
    /// Forja e mostra o pet de um índice (default 0 = nascimento). [dev, sem state]
    Hatch {
        #[arg(long)]
        id: u64,
        #[arg(long, default_value_t = 0)]
        index: u32,
        #[arg(long)]
        json: bool,
    },
    /// Mostra os primeiros N pets da coleção (linha do tempo de renascimentos). [dev]
    Lineage {
        #[arg(long)]
        id: u64,
        #[arg(long, default_value_t = 5)]
        count: u32,
    },
    /// Casinha LCD ao vivo (loop animado). Usa o state, ou --id pra teste.
    Watch {
        #[arg(long)]
        id: Option<u64>,
    },
    /// Galeria: um pet de cada tier (+ shiny + Primordial) pra ver cores e sprites.
    Gallery,
    /// Estado do companion.
    Status,
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd.unwrap_or(Cmd::Status) {
        Cmd::Init => match herdr_pet::anchor::ensure_locked_state() {
            Ok(s) => {
                println!("✓ Companion inicializado — âncora travada em {}", s.anchor);
                println!("  Coleção: {} pet(s) chocada(s).", s.hatched.len());
                print_pet(&hatch(s.github_id, s.active_index));
            }
            Err(e) => {
                eprintln!("erro ao inicializar: {}", e);
                std::process::exit(1);
            }
        },
        Cmd::Hatch { id, index, json } => {
            let pet = hatch(id, index);
            if json {
                println!("{}", serde_json::to_string_pretty(&pet).unwrap());
            } else {
                print_pet(&pet);
            }
        }
        Cmd::Lineage { id, count } => {
            println!("Coleção forjada de github:{} (genesis v{})", id, GENESIS_VERSION);
            println!("{:-<80}", "");
            for i in 0..count {
                let pet = hatch(id, i);
                println!(
                    "  #{:<3} {:<16} {:<10} {:<8}{} HP {:>3} SP {:>3}  IV {}/{}",
                    pet.index,
                    pet.name,
                    pet.rarity.as_str(),
                    pet.species.name,
                    if pet.shiny { "✨" } else { " " },
                    pet.stats.hp_max,
                    pet.stats.sp_max,
                    pet.iv.total(),
                    186,
                );
            }
        }
        Cmd::Watch { id } => {
            let (gid, idx) = match (herdr_pet::state::load(), id) {
                (Some(s), _) => (s.github_id, s.active_index),
                (None, Some(i)) => (i, 0),
                (None, None) => {
                    // auto-init: resolve o GitHub e cria o state (pra o pane funcionar de cara)
                    match herdr_pet::anchor::ensure_locked_state() {
                        Ok(s) => (s.github_id, s.active_index),
                        Err(e) => {
                            eprintln!(
                                "sem state e não consegui resolver o GitHub: {}\n(rode `herdr-pet init` ou passe --id N)",
                                e
                            );
                            std::process::exit(1);
                        }
                    }
                }
            };
            use std::io::Write;
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::Arc;
            let running = Arc::new(AtomicBool::new(true));
            let r = running.clone();
            let _ = ctrlc::set_handler(move || {
                r.store(false, Ordering::SeqCst);
            });
            // alternate screen buffer: não suja nem scrolla o terminal principal
            print!("\x1b[?1049h");
            let _ = std::io::stdout().flush();
            let mut frame = 0u32;
            while running.load(Ordering::SeqCst) {
                let pet = hatch(gid, idx);
                print!("\x1b[H\x1b[J"); // topo + limpa até o fim (sem scrollar)
                println!("{}", herdr_pet::render::render_casinha(&pet, frame));
                println!();
                println!(
                    "{}github:{} · pet #{} · Ctrl+C para sair{}",
                    herdr_pet::render::DIM,
                    gid,
                    idx,
                    herdr_pet::render::RESET
                );
                let _ = std::io::stdout().flush();
                // sleep em passos pra responder rápido ao Ctrl+C
                for _ in 0..15 {
                    if !running.load(Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                frame = frame.wrapping_add(1);
            }
            print!("\x1b[?1049l"); // restaura o buffer principal
            let _ = std::io::stdout().flush();
        }
        Cmd::Gallery => {
            use herdr_pet::{Pet, Rarity};
            use std::collections::HashMap;
            let order = [
                Rarity::Common,
                Rarity::Uncommon,
                Rarity::Rare,
                Rarity::Epic,
                Rarity::Legendary,
            ];
            let mut by_tier: HashMap<Rarity, Pet> = HashMap::new();
            let mut shiny: Option<Pet> = None;
            let mut id = 0u64;
            while (by_tier.len() < 5 || shiny.is_none()) && id < 50_000 {
                let pet = hatch(id, 0);
                if pet.shiny && shiny.is_none() {
                    shiny = Some(pet.clone());
                }
                by_tier.entry(pet.rarity).or_insert(pet);
                id += 1;
            }
            println!(
                "{}Galeria — um pet de cada tier (cor + sprite diferentes){}\n",
                herdr_pet::render::DIM,
                herdr_pet::render::RESET
            );
            for tier in order {
                if let Some(pet) = by_tier.get(&tier) {
                    println!("{}", herdr_pet::render::render_casinha(pet, 0));
                    println!();
                }
            }
            if let Some(pet) = shiny {
                println!("{}✨ Bônus: um SHINY{}\n", herdr_pet::render::BOLD, herdr_pet::render::RESET);
                println!("{}", herdr_pet::render::render_casinha(&pet, 0));
                println!();
            }
            // Easter egg: Primordial exclusivo do criador (shiny iridescente; animado no `watch`)
            let primordial = hatch(herdr_pet::forge::FREDERICO_ID, 0);
            println!(
                "{}✦ Primordial — exclusivo do criador (shiny iridescente){}\n",
                herdr_pet::render::BOLD,
                herdr_pet::render::RESET
            );
            println!("{}", herdr_pet::render::render_casinha(&primordial, 0));
        }
        Cmd::Status => {
            println!("herdr-pet — companion V-Pet do Herdr");
            println!("genesis_version : {}", GENESIS_VERSION);
            println!("raridade        : forjada por âncora GitHub (não sorteada)");
            println!("subcomandos     : init | hatch | lineage | watch | gallery | status");
        }
    }
}

fn print_pet(pet: &herdr_pet::Pet) {
    println!("┌─ pet #{} ─────────────────────────────", pet.index);
    println!("│ nome    : {}", pet.name);
    println!(
        "│ espécie : {}{}",
        pet.species.name,
        if pet.shiny { "  ✦ shiny" } else { "" }
    );
    println!("│ tier    : {}", pet.rarity.as_str());
    println!("│ HP/SP   : {} / {}", pet.stats.hp_max, pet.stats.sp_max);
    println!(
        "│ stats   : ATK {} · DEF {} · SpA {} · SpD {} · SPE {}",
        pet.stats.atk, pet.stats.def, pet.stats.sp_atk, pet.stats.sp_def, pet.stats.speed
    );
    println!(
        "│ IV      : {}/{}/{}/{}/{}/{}  (hp/atk/def/spA/spD/spe, total {}/{})",
        pet.iv.hp,
        pet.iv.atk,
        pet.iv.def,
        pet.iv.sp_atk,
        pet.iv.sp_def,
        pet.iv.speed,
        pet.iv.total(),
        186,
    );
    println!("│ âncora  : {}", pet.provenance.anchor);
    println!("│ seed    : {}…", &pet.provenance.seed_hash[..12]);
    println!("│ versão  : {}", pet.provenance.genesis_version);
    println!("└──────────────────────────────────────────");
}
