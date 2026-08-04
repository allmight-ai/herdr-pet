use clap::{Parser, Subcommand};
use herdr_pet::{hatch, Pet, GENESIS_VERSION};

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
                let pet = hatch(s.github_id, s.active_index);
                print_pet(&pet);
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
        Cmd::Status => {
            println!("herdr-pet — companion V-Pet do Herdr");
            println!("genesis_version : {}", GENESIS_VERSION);
            println!("raridade        : forjada por âncora GitHub (não sorteada)");
            println!("subcomandos     : init | hatch --id N | lineage --id N | status");
        }
    }
}

fn print_pet(pet: &Pet) {
    println!("┌─ pet #{} ─────────────────────────────", pet.index);
    println!("│ nome    : {}", pet.name);
    println!(
        "│ espécie : {}{}",
        pet.species.name,
        if pet.shiny { "  ✨ shiny" } else { "" }
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
