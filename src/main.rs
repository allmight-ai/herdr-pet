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
        /// Força um mood (working/done/blocked/idle/unknown) — dev/teste; pula o poll do agente.
        #[arg(long)]
        mood: Option<String>,
    },
    /// Abre o pet como split pequeno sob demanda (pro hotkey). Dockado embaixo do pane
    /// atual (~16 linhas) e refoca o pane original. Leve: o watch só roda aberto.
    Open,
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
                    pet.rarity.as_title(),
                    pet.species.name,
                    if pet.shiny { "✨" } else { " " },
                    pet.stats.hp_max,
                    pet.stats.sp_max,
                    pet.iv.total(),
                    186,
                );
            }
        }
        Cmd::Watch { id, mood } => {
            let forced = mood.as_deref().map(herdr_pet::AgentStatus::from_herdr);
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
            // Otimização: o pet é imutável pra (gid, idx) — forja UMA vez, cacheia.
            // (antes re-forjava a cada frame, à toa.)
            let pet = hatch(gid, idx);
            let mut frame = 0u32;
            let mut status = forced.unwrap_or(herdr_pet::AgentStatus::Idle);
            let mut title: Option<String> = None;
            let mut last_sig: Option<(herdr_pet::AgentStatus, u32, Option<String>)> = None;
            while running.load(Ordering::SeqCst) {
                // Poll do agente a cada ~3s (3 ticks × ~0,8s) — status + tarefa. Só sem --mood.
                if forced.is_none() && frame % 3 == 0 {
                    if let Some(info) = herdr_pet::agent::focused_agent_info() {
                        status = info.status;
                        title = info.title;
                    }
                }
                // Redraw só quando algo visível muda: status, tarefa ou fase da animação.
                let period = herdr_pet::render::animation_period(status, &pet);
                let sig = (status, frame % period, title.clone());
                if last_sig.as_ref() != Some(&sig) {
                    print!("\x1b[H\x1b[J"); // topo + limpa até o fim (sem scrollar)
                    println!(
                        "{}",
                        herdr_pet::render::render_casinha(&pet, frame, status, title.as_deref())
                    );
                    println!();
                    println!(
                        "{}github:{} · pet #{} · Ctrl+C para sair{}",
                        herdr_pet::render::DIM,
                        gid,
                        idx,
                        herdr_pet::render::RESET
                    );
                    let _ = std::io::stdout().flush();
                    last_sig = Some(sig);
                }
                // sleep em passos pra responder rápido ao Ctrl+C (~0,8s/ciclo)
                for _ in 0..16 {
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
        Cmd::Open => match open_pet_small() {
            Ok(()) => {}
            Err(e) => {
                eprintln!("erro ao abrir o pet: {e}");
                std::process::exit(1);
            }
        },
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
                    println!(
                        "{}",
                        herdr_pet::render::render_casinha(pet, 0, herdr_pet::AgentStatus::Idle, None)
                    );
                    println!();
                }
            }
            if let Some(pet) = shiny {
                println!("{}✨ Bônus: um SHINY{}\n", herdr_pet::render::BOLD, herdr_pet::render::RESET);
                println!(
                    "{}",
                    herdr_pet::render::render_casinha(&pet, 0, herdr_pet::AgentStatus::Idle, None)
                );
                println!();
            }
            // Easter egg: Primordial exclusivo do criador (shiny iridescente; animado no `watch`)
            let primordial = hatch(herdr_pet::forge::FREDERICO_ID, 0);
            println!(
                "{}✦ Primordial — exclusivo do criador (shiny iridescente){}\n",
                herdr_pet::render::BOLD,
                herdr_pet::render::RESET
            );
            println!(
                "{}",
                herdr_pet::render::render_casinha(&primordial, 0, herdr_pet::AgentStatus::Idle, None)
            );
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
    println!("│ tier    : {}", pet.rarity.as_title());
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

/// Caminho do CLI `herdr`: HERDR_BIN_PATH → `herdr` no PATH → `~/.local/bin/herdr`.
fn herdr_bin() -> String {
    if let Ok(b) = std::env::var("HERDR_BIN_PATH") {
        return b;
    }
    if std::process::Command::new("herdr").arg("--version").output().is_ok() {
        return "herdr".to_string();
    }
    if let Ok(home) = std::env::var("HOME") {
        return format!("{home}/.local/bin/herdr");
    }
    "herdr".to_string()
}

/// O pane atualmente focado (via `herdr pane current`). Mais robusto que a env var.
fn focused_pane() -> Result<String, String> {
    let bin = herdr_bin();
    let out = std::process::Command::new(&bin)
        .args(["pane", "current"])
        .output()
        .map_err(|e| format!("herdr pane current: {e}"))?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|_| format!("pane current inesperado: {}", String::from_utf8_lossy(&out.stdout)))?;
    v["result"]["pane"]["pane_id"]
        .as_str()
        .map(String::from)
        .ok_or("não veio o pane_id atual".to_string())
}

/// Acha o pane do pet (label "Pet") no workspace atual, se existir.
fn pet_pane_in_workspace() -> Result<Option<String>, String> {
    let bin = herdr_bin();
    let ws = std::env::var("HERDR_WORKSPACE_ID").ok();
    let out = std::process::Command::new(&bin)
        .args(["pane", "list"])
        .output()
        .map_err(|e| format!("herdr pane list: {e}"))?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    Ok(v["result"]["panes"]
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|p| {
                    p.get("label").and_then(|l| l.as_str()) == Some("Pet")
                        && p.get("workspace_id").and_then(|w| w.as_str()) == ws.as_deref()
                })
                .and_then(|p| p["pane_id"].as_str().map(String::from))
        }))
}

/// **Toggle** do pet (pro hotkey): se já existe um pane do pet neste workspace, fecha;
/// senão abre como split pequeno dockado (~16 linhas) e refoca o pane original.
/// Leve — o `watch` só roda enquanto aberto.
fn open_pet_small() -> Result<(), String> {
    const PLUGIN_ID: &str = "fredericotmello.herdr-pet";
    let bin = herdr_bin();

    // Toggle: pet já existe neste workspace → fecha.
    if let Some(existing) = pet_pane_in_workspace()? {
        let _ = std::process::Command::new(&bin)
            .args(["plugin", "pane", "close", &existing])
            .output();
        println!("✓ pet fechado ({existing}).");
        return Ok(());
    }

    // pane alvo = o focado (via API — mais robusto que a env var HERDR_PANE_ID)
    let target = focused_pane()?;

    // 1) abre o pet dockado abaixo do pane atual (split)
    let out = std::process::Command::new(&bin)
        .args([
            "plugin", "pane", "open", "--plugin", PLUGIN_ID, "--entrypoint", "lcd",
            "--placement", "split", "--target-pane", &target, "--direction", "down",
        ])
        .output()
        .map_err(|e| format!("não consegui rodar `herdr`: {e}"))?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|_| format!("resposta inesperada: {}", String::from_utf8_lossy(&out.stdout)))?;
    let pet = v["result"]["plugin_pane"]["pane"]["pane_id"]
        .as_str()
        .ok_or("não veio o pane_id do pet")?
        .to_string();

    // 2) encolhe até ~16 linhas (resize down em passos pequenos)
    for _ in 0..12 {
        let r = std::process::Command::new(&bin)
            .args(["pane", "resize", "--pane", &pet, "--direction", "down", "--amount", "0.02"])
            .output()
            .map_err(|e| format!("herdr resize: {e}"))?;
        let rv: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap_or_default();
        let h = rv["result"]["resize"]["layout"]["panes"]
            .as_array()
            .and_then(|a| a.iter().find(|p| p["pane_id"].as_str() == Some(&pet)))
            .and_then(|p| p["rect"]["height"].as_u64());
        if matches!(h, Some(h) if h <= 17) {
            break;
        }
    }

    // 3) refoca o pane original (vizinho de cima do pet)
    let _ = std::process::Command::new(&bin)
        .args(["pane", "focus", "--pane", &pet, "--direction", "up"])
        .output();

    println!("✓ pet aberto ({pet}) — dockado embaixo, ~16 linhas. Ctrl+C no pane do pet fecha.");
    Ok(())
}
