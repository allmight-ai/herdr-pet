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
    /// Mostra os primeiros N pets forjados a partir de um github id. [dev]
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
    /// Pós-install: grava atalho no config do Herdr + shim no PATH (idempotente).
    /// Rodado automaticamente no `[[build]]` e no `[[startup]]` do plugin.
    Setup {
        /// Saída silenciosa (startup do Herdr).
        #[arg(long)]
        quiet: bool,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd.unwrap_or(Cmd::Status) {
        Cmd::Init => match herdr_pet::anchor::ensure_locked_state() {
            Ok(s) => {
                println!("✓ Companion inicializado — âncora travada em {}", s.anchor);
                println!("  Pets chocados: {}.", s.hatched.len());
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
            println!("Pets forjados de github:{} (genesis v{})", id, GENESIS_VERSION);
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
            // Resolve o state (mutável) — guardamos XP nele. `persist` = salva em disco.
            let (mut state, persist) = match (herdr_pet::state::load(), id) {
                (Some(s), _) => (s, true),
                (None, Some(i)) => (herdr_pet::state::State::new(i), false), // dev transitório
                (None, None) => match herdr_pet::anchor::ensure_locked_state() {
                    Ok(s) => (s, true),
                    Err(e) => {
                        eprintln!(
                            "sem state e não consegui resolver o GitHub: {}\n(rode `herdr-pet init` ou passe --id N)",
                            e
                        );
                        std::process::exit(1);
                    }
                },
            };
            let gid = state.github_id;
            let idx = state.active_index;

            use std::io::Write;
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::Arc;
            use std::time::Instant;
            use herdr_pet::agent::all_agents_info;
            use herdr_pet::progression::{harmonic_milli, level_view, Accrual};
            use herdr_pet::render::{bar, BOLD, DIM, RESET};

            let running = Arc::new(AtomicBool::new(true));
            let r = running.clone();
            let _ = ctrlc::set_handler(move || {
                r.store(false, Ordering::SeqCst);
            });
            print!("\x1b[?1049h"); // alternate screen buffer
            let _ = std::io::stdout().flush();

            // Otimização: o pet é imutável pra (gid, idx) — forja UMA vez, cacheia.
            let pet = hatch(gid, idx);

            let mut frame = 0u32;
            let mut status = forced.unwrap_or(herdr_pet::AgentStatus::Idle);
            // Tarefas a exibir: com vários working, o loop rotaciona entre elas.
            let mut titles: Vec<String> = Vec::new();
            // nº de agentes trabalhando agora (multiplicador harmônico do XP live).
            let mut n_working: usize =
                if matches!(status, herdr_pet::AgentStatus::Working) { 1 } else { 0 };

            // Catch-up na abertura: trabalho de TODOS os agentes enquanto o pane esteve
            // fechado. Display agrega todos (acorda se algum working); XP agrega todos
            // (com decaimento). Só sem --mood.
            if forced.is_none() {
                let agents = all_agents_info();
                let (s, ts, working, pane_seqs) = agents_snapshot(&agents);
                status = s;
                titles = ts;
                n_working = working;
                state.apply_catchup(&pane_seqs);
            }

            // Acumulador de XP por tempo de trabalho acompanhado (dt real).
            let mut accrual = Accrual::new();
            let mut last_instant = Instant::now();

            // Save periódico (~30s) e só se o XP mudou — sem I/O de disco a cada tick.
            let mut last_save_frame = 0u32;
            let mut last_saved_xp = state.xp;
            const SAVE_EVERY_FRAMES: u32 = 36; // ~30s (ciclo ~0,8s)
            const TITLE_ROTATION_FRAMES: u32 = 5; // ~4s por tarefa quando há vários working

            // sig = o que determina redraw. Inclui nível + XP p/ redesenhar ao subir.
            let mut last_sig: Option<(herdr_pet::AgentStatus, u32, Option<String>, u8, u64)> = None;

            while running.load(Ordering::SeqCst) {
                // Poll a cada ~2,4s (3 frames): snapshot único — focado (display),
                // nº working (XP live) e pares (pane,seq) pra trackear sem dupla contagem.
                if forced.is_none() && frame % 3 == 0 {
                    let agents = all_agents_info();
                    let (s, ts, working, pane_seqs) = agents_snapshot(&agents);
                    status = s;
                    titles = ts;
                    n_working = working;
                    state.observe_seq(&pane_seqs); // trackeia sem creditar (o live cuida do acompanhado)
                }
                // XP live: ritmo base × H(n_working). Sem working → multiplicador 0 → nada.
                let now = Instant::now();
                let dt = now.duration_since(last_instant);
                last_instant = now;
                let mult = harmonic_milli(n_working);
                if mult > 0 {
                    state.xp += accrual.add_working(dt, mult);
                }

                // Tarefa exibida: com vários working, rotaciona entre elas (~4s cada);
                // com 0/1, mostra a única (ou nenhuma).
                let title: Option<String> = if titles.is_empty() {
                    None
                } else {
                    let idx = ((frame / TITLE_ROTATION_FRAMES) as usize) % titles.len();
                    Some(titles[idx].clone())
                };

                // Redraw só quando algo visível muda (status, fase da anim, tarefa, nível/XP).
                let lv = level_view(state.xp);
                let period = herdr_pet::render::animation_period(status, &pet);
                let sig = (status, frame % period, title.clone(), lv.level, lv.xp_into);
                if last_sig.as_ref() != Some(&sig) {
                    print!("\x1b[H\x1b[J"); // topo + limpa até o fim (sem scrollar)
                    println!(
                        "{}",
                        herdr_pet::render::render_casinha(&pet, frame, status, title.as_deref())
                    );
                    println!();
                    if lv.xp_span > 0 {
                        println!(
                            "{DIM}#{idx} · {BOLD}Nv {}{RESET}{DIM} · {RESET}{}{DIM} {}/{} XP · ⚙ {}{RESET}",
                            lv.level,
                            bar(lv.xp_into as u16, lv.xp_span as u16, 10),
                            lv.xp_into,
                            lv.xp_span,
                            n_working,
                        );
                    } else {
                        println!(
                            "{DIM}#{idx} · {BOLD}Nv 99 ★ máximo{RESET}{DIM} · ⚙ {} · Ctrl+C{RESET}",
                            n_working
                        );
                    }
                    let _ = std::io::stdout().flush();
                    last_sig = Some(sig);
                }

                // Save periódico (sem ddos de disco): ~a cada 30s, só se o XP mudou.
                if persist
                    && state.xp != last_saved_xp
                    && frame.wrapping_sub(last_save_frame) >= SAVE_EVERY_FRAMES
                {
                    if herdr_pet::state::save(&state).is_ok() {
                        last_saved_xp = state.xp;
                        last_save_frame = frame;
                    }
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

            // Save final na saída (Ctrl+C) — não perde o progresso da sessão.
            if persist && state.xp != last_saved_xp {
                let _ = herdr_pet::state::save(&state);
            }

            print!("\x1b[?1049l"); // restaura o buffer principal
            let _ = std::io::stdout().flush();
        }
        Cmd::Setup { quiet } => match herdr_pet::setup::ensure_setup() {
            Ok(report) => {
                if !quiet {
                    herdr_pet::setup::print_report(&report);
                }
                // Exit 0 even on soft failures (config missing etc.) so plugin startup
                // never blocks the Herdr server. Hard I/O errors still fail below.
                if matches!(
                    report.keybind,
                    herdr_pet::setup::KeybindStatus::Error
                ) || matches!(report.shim, herdr_pet::setup::ShimStatus::Error)
                {
                    std::process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("erro no setup: {e}");
                std::process::exit(1);
            }
        },
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
        Cmd::Status => match herdr_pet::state::load() {
            Some(s) => print_pet(&hatch(s.github_id, s.active_index)),
            None => println!(
                "herdr-pet — sem state ainda. Rode `herdr-pet init` ou abra o pane `watch` (auto-init)."
            ),
        },
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

/// Faz um resize do pane `pet` e devolve a altura resultante do pet (lê do layout da resposta).
fn resize_pet_height(bin: &str, pet: &str, dir: &str, amount: f64) -> Option<u64> {
    let r = std::process::Command::new(bin)
        .args([
            "pane", "resize", "--pane", pet, "--direction", dir, "--amount", &amount.to_string(),
        ])
        .output()
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).unwrap_or_default();
    v["result"]["resize"]["layout"]["panes"]
        .as_array()?
        .iter()
        .find(|p| p["pane_id"].as_str() == Some(pet))
        .and_then(|p| p["rect"]["height"].as_u64())
}

/// Acha o pane do pet (label "Pet") no workspace do pane focado, se existir.
fn pet_pane_in_workspace() -> Result<Option<String>, String> {
    let bin = herdr_bin();
    // Prefer API focus (hotkey/action context) over HERDR_WORKSPACE_ID — the env
    // can lag when `herdr-pet open` is called from another pane/workspace.
    let ws = focused_workspace_id().or_else(|| std::env::var("HERDR_WORKSPACE_ID").ok());
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
                        && match ws.as_deref() {
                            Some(ws) => p.get("workspace_id").and_then(|w| w.as_str()) == Some(ws),
                            None => true,
                        }
                })
                .and_then(|p| p["pane_id"].as_str().map(String::from))
        }))
}

fn focused_workspace_id() -> Option<String> {
    let bin = herdr_bin();
    let out = std::process::Command::new(&bin)
        .args(["pane", "current"])
        .output()
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    v["result"]["pane"]["workspace_id"]
        .as_str()
        .map(String::from)
}

/// Snapshot dos agentes detectados: **(status, tarefa)** do display agregado (vê todos —
/// acorda se algum working, com a tarefa de quem trabalha), nº working (multiplicador do
/// XP) e pares `(pane_id, seq)` pro catch-up/trackeio. Um único `herdr agent list` resolve tudo.
fn agents_snapshot(
    agents: &[herdr_pet::AgentInfo],
) -> (
    herdr_pet::AgentStatus,
    Vec<String>,
    usize,
    Vec<herdr_pet::state::PaneSeq>,
) {
    let (status, titles) = herdr_pet::agent::aggregate_display(agents);
    let working = agents
        .iter()
        .filter(|a| matches!(a.status, herdr_pet::AgentStatus::Working))
        .count();
    let pane_seqs = agents
        .iter()
        .map(|a| herdr_pet::state::PaneSeq {
            pane_id: a.pane_id.clone(),
            seq: a.state_change_seq,
        })
        .collect();
    (status, titles, working, pane_seqs)
}

/// **Toggle** do pet (pro hotkey): se já existe um pane do pet neste workspace, fecha;
/// senão abre como split pequeno dockado (~16 linhas) e refoca o pane original.
/// Leve — o `watch` só roda enquanto aberto.
fn open_pet_small() -> Result<(), String> {
    const PLUGIN_ID: &str = "allmight-ai.herdr-pet";
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

    // 2) ajeita o tamanho na banda [16,18]: encolhe se >18, cresce se <16.
    //    Abaixo de 16 o topo (nome) rola fora da viewport pequena.
    for _ in 0..20 {
        match resize_pet_height(&bin, &pet, "down", 0.02) {
            Some(h) if h > 18 => {}
            _ => break,
        }
    }
    for _ in 0..12 {
        match resize_pet_height(&bin, &pet, "up", 0.02) {
            Some(h) if h < 16 => {}
            _ => break,
        }
    }

    // 3) refoca o pane original (vizinho de cima do pet)
    let _ = std::process::Command::new(&bin)
        .args(["pane", "focus", "--pane", &pet, "--direction", "up"])
        .output();

    println!("✓ pet aberto ({pet}) — dockado embaixo. Ctrl+C fecha · redimensionar: prefix+r");
    Ok(())
}
