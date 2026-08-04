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
    /// Forja e mostra o pet de um índice (default 0 = nascimento).
    Hatch {
        #[arg(long)]
        id: u64,
        #[arg(long, default_value_t = 0)]
        index: u32,
        #[arg(long)]
        json: bool,
    },
    /// Mostra os primeiros N pets da coleção (linha do tempo de renascimentos).
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
            println!("{:-<64}", "");
            for i in 0..count {
                let pet = hatch(id, i);
                println!(
                    "  #{:<3} {:<16} {:<10} {:<8}{} IV {:>2}/{:>2}/{:>2}/{:>2}",
                    pet.index,
                    pet.name,
                    pet.rarity.as_str(),
                    pet.species.name,
                    if pet.shiny { " ✨" } else { "  " },
                    pet.iv.hp,
                    pet.iv.atk,
                    pet.iv.def,
                    pet.iv.spd,
                );
            }
        }
        Cmd::Status => {
            println!("herdr-pet — companion V-Pet do Herdr");
            println!("genesis_version : {}", GENESIS_VERSION);
            println!("raridade        : forjada por âncora GitHub (não sorteada)");
            println!("subcomandos     : hatch --id N | lineage --id N | status");
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
    println!(
        "│ IV      : {}/{}/{}/{}  (hp/atk/def/spd, total {})",
        pet.iv.hp,
        pet.iv.atk,
        pet.iv.def,
        pet.iv.spd,
        pet.iv.total()
    );
    println!("│ âncora  : {}", pet.provenance.anchor);
    println!("│ seed    : {}…", &pet.provenance.seed_hash[..12]);
    println!("│ versão  : {}", pet.provenance.genesis_version);
    println!("└──────────────────────────────────────────");
}
