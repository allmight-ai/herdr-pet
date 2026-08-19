use clap::{Parser, Subcommand};
use herdr_pet::{hatch, GENESIS_VERSION};

/// `println!` à prova de pipe fechado: `herdr-pet status | head` saía **101**
/// porque `println!` PANICA quando a escrita falha (EPIPE — quem está do outro
/// lado do pipe fechou a leitura). Erro ignorado: quem cortou a saída já teve o
/// que queria, e o exit 0 preserva o contrato de CLI filtrável (`| head`, `|
/// grep`). Mesma técnica do caminho do `watch` (PTY morta não pula save).
macro_rules! outln {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, $($arg)*);
    }};
}

#[derive(Parser)]
#[command(
    name = "herdr-pet",
    version,
    about = "Companion V-Pet do Herdr — raridade forjada"
)]
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
    /// Histórico dos últimos N dias (default 7): XP, tempo e sessões por dia, e a sequência.
    Log {
        /// Quantos dias pra trás mostrar (contando hoje).
        #[arg(long, default_value_t = 7)]
        days: u32,
    },
    /// Pós-install: grava atalho no config do Herdr + shim no PATH (idempotente).
    /// Rodado automaticamente no `[[build]]` e no `[[startup]]` do plugin.
    Setup {
        /// Saída silenciosa (startup do Herdr).
        #[arg(long)]
        quiet: bool,
    },
}

/// O que o `watch` compara pra decidir se REDESENHA: status do agente, fase da
/// animação, tarefa exibida, nível, XP dentro do nível, nº de working e rodapé.
/// Igual ao frame anterior ⇒ nada visível mudou ⇒ pula o desenho.
type RedrawSig = (
    herdr_pet::AgentStatus,
    u32,
    Option<String>,
    u8,
    u64,
    usize,
    String,
);

fn main() {
    let cli = Cli::parse();
    match cli.cmd.unwrap_or(Cmd::Status) {
        Cmd::Init => match herdr_pet::anchor::ensure_locked_state() {
            Ok(s) => {
                outln!("✓ Companion inicializado — âncora travada em {}", s.anchor);
                outln!("  Pets chocados: {}.", s.hatched.len());
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
                outln!("{}", serde_json::to_string_pretty(&pet).unwrap());
            } else {
                print_pet(&pet);
            }
        }
        Cmd::Lineage { id, count } => {
            outln!("Pets forjados de github:{} (genesis v{})", id, GENESIS_VERSION);
            outln!("{:-<80}", "");
            for i in 0..count {
                let pet = hatch(id, i);
                outln!(
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
                // `--mood` é modo dev de display: não auto-inicializa state (criar
                // state é gravação) — sem state, exige `--id`.
                (None, None) if forced.is_some() => {
                    eprintln!(
                        "modo dev (--mood): sem state e sem --id\n(passe `--id N` — o modo dev não cria state)"
                    );
                    std::process::exit(1);
                }
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
            // Com `--mood` o state (se houver) é só leitura: exibe o pet, nunca grava.
            let mut persist = persist && forced.is_none();

            // Lock de verdade do state (fecha a limitação do toggle): com
            // persistência ativa, este watch tem que ser o ÚNICO dono do
            // `state.json` — dois donos carregam cópias e salvam por cima um
            // do outro. Se outro watch vivo já tem o lock (`plugin pane open`
            // chamado direto, corrida de toggle), o pane NÃO morre: vira
            // ESPELHO — mesmo caminho do dev `--id`/`--mood` (desenha tudo,
            // nunca grava), marcado no rodapé. O XP real fica com o dono.
            let mut mirror = false;
            // Promoção travada por state ilegível no disco (lock livre, sem
            // dono): o rodapé troca `⚠ espelho` por `⚠ state ilegível` —
            // culpar um dono que não existe é sinal mentiroso.
            let mut state_unreadable = false;
            let mut state_lock: Option<herdr_pet::state::StateLock> = None;
            if persist {
                match herdr_pet::state::acquire_state_lock() {
                    herdr_pet::state::LockOutcome::Acquired(l) => state_lock = Some(l),
                    herdr_pet::state::LockOutcome::Held { .. } => {
                        mirror = true;
                        persist = false;
                    }
                }
            }
            // "O que está no disco" na abertura — capturado ANTES do catch-up: o XP e
            // as baselines que o catch-up creditar nascem "sujos" (não salvos) e o
            // primeiro gate periódico/final os grava. Inicializar depois do catch-up
            // os marcaria como já salvos → nenhum save → disco nunca avança → o
            // catch-up se repaga a cada reabertura (replay infinito).
            let mut last_saved_xp = state.xp;
            let mut last_saved_seqs: std::collections::HashMap<String, u64> =
                state.last_seq_by_pane.clone();
            let gid = state.github_id;
            let idx = state.active_index;

            use std::io::Write;
            use std::sync::atomic::{AtomicBool, Ordering};
            use std::sync::Arc;
            use std::time::Instant;
            use herdr_pet::progression::{harmonic_milli, level_view, Accrual};
            use herdr_pet::render::{bar, BOLD, DIM, RESET};
            use herdr_pet::session::Session;

            let running = Arc::new(AtomicBool::new(true));
            let r = running.clone();
            let _ = ctrlc::set_handler(move || {
                r.store(false, Ordering::SeqCst);
            });
            // Handle de stdout com erro IGNORADO em todo o caminho do watch: pane
            // destruído sem Ctrl+C mata a PTY e `print!` PANICA quando a escrita
            // falha — o panic pularia o save final (perda de até ~30s de XP).
            // `let _ = write!` deixa o processo morrer em paz, save incluído.
            let mut out = std::io::stdout().lock();
            let _ = write!(out, "\x1b[?1049h"); // alternate screen buffer
            let _ = out.flush();

            // Otimização: o pet é imutável pra (gid, idx) — forja UMA vez, cacheia.
            let pet = hatch(gid, idx);

            let mut frame = 0u32;
            let mut status = forced.unwrap_or(herdr_pet::AgentStatus::Idle);
            // Tarefas a exibir: com vários working, o loop rotaciona entre elas.
            let mut titles: Vec<String> = Vec::new();
            // nº de agentes trabalhando agora (multiplicador harmônico do XP live).
            let mut n_working: usize =
                if matches!(status, herdr_pet::AgentStatus::Working) { 1 } else { 0 };
            let mut working_labels: Vec<String> = Vec::new();

            // Sessão começa no XP/nível de disco — o catch-up entra no delta do resumo.
            let mut session = Session::start(state.xp, state.level());
            // O mood forçado não entra no resumo como agente — não houve trabalho real.
            if forced.is_none() && matches!(status, herdr_pet::AgentStatus::Working) {
                session.note_working(std::iter::empty::<&str>(), n_working);
            }

            // Catch-up na abertura: trabalho de TODOS os agentes enquanto o pane esteve
            // fechado. Display agrega todos (acorda se algum working); XP agrega todos
            // (com decaimento). Só sem --mood.
            if forced.is_none() {
                let snap = refresh_agents(&mut state, SeqMode::Catchup);
                status = snap.status;
                titles = snap.titles;
                n_working = snap.n_working;
                working_labels = snap.working_labels;
                session.note_working(&snap.working_panes, snap.n_working);
            }

            // Acumulador de XP por tempo de trabalho acompanhado (dt real).
            let mut accrual = Accrual::new();
            let mut last_instant = Instant::now();

            // Save periódico (~30s), se o XP OU as baselines de seq mudaram desde o
            // último save (`last_saved_*`, capturados antes do catch-up) — sem I/O de
            // disco a cada tick. Baseline avançada pelo poll e não salva morre na
            // memória: o catch-up seguinte paga de novo o trecho já visto com o pane
            // aberto. Sem clonar mapa por frame: guardamos o mapa do momento do último
            // save (clone só ao salvar) e comparamos só quando o gate de frame passa
            // (~a cada 30s).
            let mut last_save_frame = 0u32;
            // Ritmo do heartbeat do lock: contador PRÓPRIO, não o do save — o
            // `last_save_frame` só anda quando há algo a salvar, e amarrar o
            // heartbeat nele bateria o mtime do lock a cada frame numa sessão
            // parada (~0,8s de I/O à toa).
            let mut last_beat_frame = 0u32;
            // true = o último save tentado falhou (marcador `⚠ save` no rodapé).
            let mut save_failing = false;
            const SAVE_EVERY_FRAMES: u32 = 36; // ~30s (ciclo ~0,8s)
            const TITLE_ROTATION_FRAMES: u32 = 5; // ~4s por tarefa quando há vários working

            // sig = o que determina redraw. Inclui nível + XP p/ redesenhar ao subir.
            let mut last_sig: Option<RedrawSig> = None;

            while running.load(Ordering::SeqCst) {
                // Poll a cada ~2,4s (3 frames): snapshot único — focado (display),
                // nº working (XP live) e pares (pane,seq) pra trackear sem dupla contagem.
                if forced.is_none() && frame.is_multiple_of(3) {
                    let snap = refresh_agents(&mut state, SeqMode::Track);
                    status = snap.status;
                    titles = snap.titles;
                    n_working = snap.n_working;
                    // Labels também: o rodapé `⚙ N nomes` é o conjunto ATUAL —
                    // sem isso o badge misturava N novo com nomes da abertura.
                    working_labels = snap.working_labels;
                    session.note_working(&snap.working_panes, snap.n_working);
                }
                // XP live: ritmo base × H(n_working). Sem working → multiplicador 0 → nada.
                // `--mood` zera o multiplicador: humor forçado não é trabalho real.
                let now = Instant::now();
                let dt = now.duration_since(last_instant);
                last_instant = now;
                let mult = if forced.is_some() { 0 } else { harmonic_milli(n_working) };
                if mult > 0 {
                    state.xp += accrual.add_working(dt, mult);
                    // Mesmo gate do XP: este dt foi trabalho acompanhado —
                    // vira `secs_working` na linha do diário no fecho.
                    session.note_working_span(dt);
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
                let badge = herdr_pet::agent::format_working_badge(n_working, &working_labels);
                // Sinais do rodapé, NÃO-SPAM: `· ⚠ save` acende enquanto o último
                // save tiver falhado e some no primeiro que passa; `· ⚠ espelho`
                // fica a sessão inteira do espelho. Sinal por transição (entram no
                // sig como string): liga/desliga ⇒ um redraw; condição persistente
                // não redesenha nada por frame. Nada de eprintln no loop — sujaria
                // o LCD.
                let mut footer = badge;
                if mirror {
                    // Rótulo honesto: `⚠ espelho` quando OUTRO pet é o dono;
                    // `⚠ state ilegível` quando o lock está livre e o que
                    // trava a promoção é o state no disco.
                    footer = if state_unreadable {
                        format!("{footer} · ⚠ state ilegível")
                    } else {
                        format!("{footer} · ⚠ espelho")
                    };
                }
                if save_failing {
                    footer = format!("{footer} · ⚠ save");
                }
                let sig = (
                    status,
                    frame % period,
                    title.clone(),
                    lv.level,
                    lv.xp_into,
                    n_working,
                    footer.clone(),
                );
                if last_sig.as_ref() != Some(&sig) {
                    let _ = write!(out, "\x1b[H\x1b[J"); // topo + limpa até o fim (sem scrollar)
                    let _ = writeln!(
                        out,
                        "{}",
                        herdr_pet::render::render_casinha(&pet, frame, status, title.as_deref())
                    );
                    let _ = writeln!(out);
                    if lv.xp_span > 0 {
                        let _ = writeln!(
                            out,
                            "{DIM}#{idx} · {BOLD}Nv {}{RESET}{DIM} · {RESET}{}{DIM} {}/{} XP · {footer}{RESET}",
                            lv.level,
                            bar(lv.xp_into as u16, lv.xp_span as u16, 10),
                            lv.xp_into,
                            lv.xp_span,
                        );
                    } else {
                        let _ = writeln!(
                            out,
                            "{DIM}#{idx} · {BOLD}Nv 99 ★ máximo{RESET}{DIM} · {footer} · Ctrl+C{RESET}",
                        );
                    }
                    let _ = out.flush();
                    last_sig = Some(sig);
                }

                // Heartbeat/promoção no ritmo do save (~30s): dono renova o
                // mtime do lock — prova de vida pra o próximo watch decidir se
                // um lock parado é sobra de crash ou dono vivo. Espelho tenta
                // PROMOVER: se o dono saiu (o lock cai no fecho dele), este
                // pane assume o state em vez de seguir desenhando sem ganhar
                // nada até o fim da sessão.
                if frame.wrapping_sub(last_beat_frame) >= SAVE_EVERY_FRAMES {
                    last_beat_frame = frame;
                    if mirror {
                        match herdr_pet::state::acquire_state_lock() {
                            herdr_pet::state::LockOutcome::Acquired(l) => {
                                // Promoção: reler o DISCO é obrigatório — o
                                // state em memória do espelho é foto da
                                // abertura dele, e salvar por cima apagaria o
                                // XP que o dono gravou de verdade. Rebase
                                // tudo no que veio do disco.
                                match herdr_pet::state::load_outcome() {
                                    herdr_pet::state::LoadOutcome::Loaded(disk) => {
                                        state = disk;
                                        // NOTA (chocadeira futura): o `pet`
                                        // desenhado não é reforgado aqui — hoje
                                        // nada muda `active_index` em runtime,
                                        // mas no dia em que mudar, a promoção
                                        // terá que rehatch(gid, state.active_index).
                                        last_saved_xp = state.xp;
                                        last_saved_seqs = state.last_seq_by_pane.clone();
                                        // Sessão RENASCIDA antes do catch-up: o
                                        // hiato (dono saiu → agora) entra no
                                        // delta como "recuperado", igual à
                                        // abertura — o `log` soma `xp_gained`
                                        // por dia, excluir aqui subcontaria o
                                        // dia inteiro. O renascimento zera
                                        // também tempo/panes: o trecho de
                                        // espelho já saiu na linha do dono.
                                        session.rebase(state.xp, state.level());
                                        // A fração de milli-XP junta no modo
                                        // espelho morre com ele — até 1 XP, mas
                                        // inflar é inflar.
                                        accrual = herdr_pet::progression::Accrual::new();
                                        let snap = refresh_agents(&mut state, SeqMode::Catchup);
                                        status = snap.status;
                                        titles = snap.titles;
                                        n_working = snap.n_working;
                                        working_labels = snap.working_labels;
                                        session.note_working(&snap.working_panes, snap.n_working);
                                        persist = true;
                                        mirror = false;
                                        state_unreadable = false;
                                        state_lock = Some(l);
                                    }
                                    // Sem state legível, não promove: salvar
                                    // por cima de ausente/ilegível destruiria
                                    // dados (ver `LoadOutcome`). Solta o lock
                                    // que acabou de nascer, acende o sinal
                                    // honesto e tenta de novo no próximo gate.
                                    _ => {
                                        state_unreadable = true;
                                        drop(l);
                                    }
                                }
                            }
                            // Dono ainda vivo: segue espelho (o rótulo volta a
                            // ser `⚠ espelho`, que agora é a verdade), retry
                            // no próximo gate.
                            herdr_pet::state::LockOutcome::Held { .. } => {
                                state_unreadable = false;
                            }
                        }
                    } else if let Some(l) = state_lock.as_ref() {
                        l.heartbeat();
                    }
                }

                // Save periódico (sem ddos de disco): ~a cada 30s, se o XP ou as
                // baselines mudaram desde o último save. Gate de frame primeiro —
                // a comparação do mapa roda ~a cada 30s, não a cada frame.
                if persist
                    && frame.wrapping_sub(last_save_frame) >= SAVE_EVERY_FRAMES
                    && (state.xp != last_saved_xp || state.last_seq_by_pane != last_saved_seqs)
                {
                    match herdr_pet::state::save(&state) {
                        Ok(()) => {
                            last_saved_xp = state.xp;
                            last_saved_seqs = state.last_seq_by_pane.clone();
                            last_save_frame = frame;
                            save_failing = false; // marcador some no primeiro save que passa
                        }
                        // Falha deixa de ser silenciosa: acende `⚠ save` no rodapé.
                        // `last_save_frame` anda TAMBÉM na falha — retry no mesmo
                        // ritmo ~30s, sem martelar disco a cada frame (~0,8s) se o
                        // estado persistir (ex.: fs read-only).
                        Err(_) => {
                            save_failing = true;
                            last_save_frame = frame;
                        }
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

            // Save final na saída (Ctrl+C / SIGHUP do toggle) — não perde o progresso
            // (XP ou baselines de seq ainda não salvos). Roda ANTES de qualquer
            // print do farewell e independe do stdout estar vivo (pane pode ter
            // sido destruído — os prints abaixo ignoram erro justamente por isso).
            let unsaved =
                state.xp != last_saved_xp || state.last_seq_by_pane != last_saved_seqs;
            if persist && unsaved && herdr_pet::state::save(&state).is_err() {
                save_failing = true;
            }

            let summary = session.summarize(state.xp, state.level());

            // Diário: a sessão vira história (linha no `sessions.jsonl`).
            // Acessório de verdade — se a gravação falhar, o pet não repara: o
            // state JÁ foi salvo acima, nada de panic, e o aviso segue o padrão
            // do `⚠ save` (uma linha discreta no farewell, LCD limpo o resto do
            // tempo). Espelho e dev (`--id`, `--mood`) não gravam: o trabalho
            // daquele período já vai no diário do dono do lock — gravar de novo
            // seria dupla contagem.
            let mut journal_failing = false;
            if persist {
                let entry = summary.to_entry(herdr_pet::journal::today_local());
                journal_failing = herdr_pet::journal::append(&entry).is_err();
            }

            // Última escrita feita (save final + diário): o lock sai AGORA,
            // antes do farewell. O farewell segura o processo por
            // FAREWELL_MS (~1,4 s) só pra desenhar, e o toggle "move" fecha
            // este pane e reabre o pet ~1,2 s depois da linha de resumo — se
            // o lock só caísse no fim do braço, o pet novo nasceria espelho
            // em TODO move (corrida medida no PARECER-1). Lock protege
            // escrita; não há mais escrita pela frente.
            drop(state_lock.take());

            let mut line = summary.format_line();
            if mirror {
                // O resumo do espelho mostra um XP que este pane NÃO gravou —
                // o sufixo impede a leitura de "ganhei e salvei". Mesma voz do
                // rodapé: sem dono no lock, o motivo é o state, não o espelho.
                line.push_str(if state_unreadable {
                    " · state ilegível"
                } else {
                    " · espelho"
                });
            }
            if !mirror {
                notify_session(&line);
            }

            // Último quadro ainda na tela alternativa — o pane precisa estar vivo
            // pra isso aparecer. O toggle manda Ctrl+C e só fecha depois.
            let goodbye = if summary.xp_gained > 0 {
                herdr_pet::AgentStatus::Done
            } else {
                status
            };
            let _ = write!(out, "\x1b[H\x1b[J");
            let _ = writeln!(
                out,
                "{}",
                herdr_pet::render::render_casinha(&pet, frame, goodbye, None)
            );
            let _ = writeln!(out);
            let _ = writeln!(out, "{BOLD}{line}{RESET}");
            if save_failing {
                // Última chance de avisar: o save final também falhou — o XP da
                // sessão não chegou ao disco (o rodapé `⚠ save` morreu com o pane).
                let _ = writeln!(
                    out,
                    "{DIM}⚠ save falhou — progresso não gravado no disco{RESET}"
                );
            }
            if journal_failing {
                // Mesma voz do ⚠ save: histórico é acessório, mas a sequência
                // pode perder um dia em silêncio — o usuário merece saber.
                let _ = writeln!(
                    out,
                    "{DIM}⚠ diário falhou — sessão não foi pro histórico{RESET}"
                );
            }
            let _ = out.flush();
            std::thread::sleep(std::time::Duration::from_millis(
                herdr_pet::session::FAREWELL_MS,
            ));

            let _ = write!(out, "\x1b[?1049l"); // restaura o buffer principal
            let _ = writeln!(out, "{line}");
            let _ = out.flush();
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
            outln!(
                "{}Galeria — um pet de cada tier (cor + sprite diferentes){}\n",
                herdr_pet::render::DIM,
                herdr_pet::render::RESET
            );
            for tier in order {
                if let Some(pet) = by_tier.get(&tier) {
                    outln!(
                        "{}",
                        herdr_pet::render::render_casinha(pet, 0, herdr_pet::AgentStatus::Idle, None)
                    );
                    outln!();
                }
            }
            if let Some(pet) = shiny {
                outln!("{}✨ Bônus: um SHINY{}\n", herdr_pet::render::BOLD, herdr_pet::render::RESET);
                outln!(
                    "{}",
                    herdr_pet::render::render_casinha(&pet, 0, herdr_pet::AgentStatus::Idle, None)
                );
                outln!();
            }
            // Easter egg: Primordial exclusivo do criador (shiny iridescente; animado no `watch`)
            let primordial = hatch(herdr_pet::forge::FREDERICO_ID, 0);
            outln!(
                "{}✦ Primordial — exclusivo do criador (shiny iridescente){}\n",
                herdr_pet::render::BOLD,
                herdr_pet::render::RESET
            );
            outln!(
                "{}",
                herdr_pet::render::render_casinha(&primordial, 0, herdr_pet::AgentStatus::Idle, None)
            );
        }
        Cmd::Log { days } => print_log(days),
        Cmd::Status => match herdr_pet::state::load_outcome() {
            herdr_pet::state::LoadOutcome::Loaded(s) => {
                print_pet(&hatch(s.github_id, s.active_index));
                print_progress(&s);
            }
            // O aviso detalhado (caminho + erro) já saiu no stderr do load.
            // Sem sugerir `init`: ele vai RECUSAR enquanto o acesso não for corrigido.
            herdr_pet::state::LoadOutcome::Unreadable => outln!(
                "herdr-pet — state presente mas ilegível (permissão?). Corrija o acesso ao arquivo indicado acima e rode de novo."
            ),
            // `Corrupt`: o conteúdo já foi preservado em `.corrupt` e o init pode
            // recriar com segurança — a orientação abaixo segue válida nos dois casos.
            herdr_pet::state::LoadOutcome::Missing
            | herdr_pet::state::LoadOutcome::Corrupt => outln!(
                "herdr-pet — sem state ainda. Rode `herdr-pet init` ou abra o pane `watch` (auto-init)."
            ),
        },
    }
}

fn print_pet(pet: &herdr_pet::Pet) {
    outln!("┌─ pet #{} ─────────────────────────────", pet.index);
    outln!("│ nome    : {}", pet.name);
    outln!(
        "│ espécie : {}{}",
        pet.species.name,
        if pet.shiny { "  ✦ shiny" } else { "" }
    );
    outln!("│ tier    : {}", pet.rarity.as_title());
    outln!("│ HP/SP   : {} / {}", pet.stats.hp_max, pet.stats.sp_max);
    outln!(
        "│ stats   : ATK {} · DEF {} · SpA {} · SpD {} · SPE {}",
        pet.stats.atk,
        pet.stats.def,
        pet.stats.sp_atk,
        pet.stats.sp_def,
        pet.stats.speed
    );
    outln!(
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
    outln!("│ âncora  : {}", pet.provenance.anchor);
    outln!("│ seed    : {}…", &pet.provenance.seed_hash[..12]);
    outln!("│ versão  : {}", pet.provenance.genesis_version);
    outln!("└──────────────────────────────────────────");
}

/// XP, nível e quem está working agora (inclui subagentes).
fn print_progress(state: &herdr_pet::state::State) {
    use herdr_pet::progression::level_view;
    use herdr_pet::render::{bar, BOLD, DIM, RESET};

    let lv = level_view(state.xp);
    let snap = herdr_pet::agent::snapshot(&herdr_pet::agent::all_agents_info());
    outln!("┌─ progresso ───────────────────────────");
    if lv.xp_span > 0 {
        outln!(
            "│ nível   : {BOLD}Nv {}{RESET}  {}  {}/{} XP",
            lv.level,
            bar(lv.xp_into as u16, lv.xp_span as u16, 10),
            lv.xp_into,
            lv.xp_span,
        );
    } else {
        outln!("│ nível   : {BOLD}Nv 99 ★ máximo{RESET}");
    }
    outln!("│ total   : {} XP", state.xp);
    // Sequência do diário, só quando há diário — sem inventar linha vazia.
    if let Some(st) = load_streak() {
        outln!("│ {}", streak_phrase(&st));
    }
    outln!(
        "│ agora   : {}",
        herdr_pet::agent::format_working_badge(snap.n_working, &snap.working_labels)
    );
    for title in snap.titles.iter().take(5) {
        outln!("{DIM}│           · {title}{RESET}");
    }
    outln!("└──────────────────────────────────────────");
}

/// `herdr-pet log [--days N]`: os últimos N dias com trabalho (XP, tempo,
/// sessões) e a sequência. I/O só na borda — janela, linha do dia e frase da
/// sequência são puras e testadas no fim do arquivo.
fn print_log(window: u32) {
    // `--days 0` é janela vazia (nem hoje entra) — clampa pra 1, o pedido mais
    // próximo que faz sentido; rejeitar no clap seria cerimônia à toa.
    let window = window.max(1);
    use herdr_pet::streaks;

    let today = herdr_pet::journal::today_local();
    let days = streaks::by_day(&herdr_pet::journal::load());
    let st = streaks::streak(&days, &today);
    // Janela em dias julianos (o contrato do streaks): diferença pura, sem
    // aritmética de calendário da nossa parte.
    let today_jd = streaks::parse_day(&today).map(|(y, m, d)| streaks::days_from_civil(y, m, d));

    if window == 1 {
        outln!("┌─ diário — último dia ───────────────");
    } else {
        outln!("┌─ diário — últimos {window} dias ──────────");
    }
    if days.is_empty() {
        outln!("│ sem histórico ainda — feche um pane do pet pra começar");
    } else {
        let mut shown = 0usize;
        for d in days.iter().rev() {
            // `by_day` vem cronológico; a linha ENTRA quando qualquer lado não
            // parseia (dia mal gravado, fuso exótico) — falha aberta, não
            // oculta: esconder trabalho que existe é pior que mostrar.
            let inside = match (today_jd, streaks::parse_day(&d.day)) {
                (Some(t), Some((y, m, dd))) => {
                    in_window(t - streaks::days_from_civil(y, m, dd), window)
                }
                _ => true,
            };
            if inside {
                outln!("│ {}", format_day_row(d));
                shown += 1;
            }
        }
        if shown == 0 {
            outln!("│ (sem sessões no período)");
        }
        outln!("│ {}", streak_phrase(&st));
    }
    outln!("└──────────────────────────────────────────");
}

/// Diferença em dias (hoje − dia) dentro da janela de `window` dias, contando
/// hoje? Sem piso inferior de propósito: data "no futuro" (fuso na meia-noite,
/// relógio adiantado) entra — a linha existe, ocultá-la mente por omissão.
fn in_window(diff: i64, window: u32) -> bool {
    diff < i64::from(window)
}

/// `2026-08-19 · +1.240 XP · 47 min · 2 sessões`. Tempo só quando houve
/// trabalho acompanhado: dia de catch-up puro mostra XP e sessões ("0 s" é
/// ruído, não informação).
fn format_day_row(d: &herdr_pet::streaks::Day) -> String {
    use herdr_pet::session::{format_duration, format_int};
    let mut parts = vec![d.day.clone(), format!("+{} XP", format_int(d.xp))];
    if d.secs_working > 0 {
        parts.push(format_duration(std::time::Duration::from_secs(
            d.secs_working,
        )));
    }
    parts.push(if d.sessions == 1 {
        "1 sessão".to_string()
    } else {
        format!("{} sessões", d.sessions)
    });
    parts.join(" · ")
}

/// `Sequência: 5 dias (recorde 12)` — singular no 1, recorde como número seco.
/// A linha que o `status` e o `log` compartilham.
fn streak_phrase(st: &herdr_pet::streaks::Streak) -> String {
    let atual = if st.current == 1 {
        "1 dia".to_string()
    } else {
        format!("{} dias", st.current)
    };
    format!("Sequência: {atual} (recorde {})", st.best)
}

/// Sequência do diário em disco (`None` se ainda não há diário — quem chama
/// decide o que fazer sem ele). Os subs (hoje local, agregação por dia) são do
/// contrato das fatias B e C.
fn load_streak() -> Option<herdr_pet::streaks::Streak> {
    let days = herdr_pet::streaks::by_day(&herdr_pet::journal::load());
    if days.is_empty() {
        return None;
    }
    let today = herdr_pet::journal::today_local();
    Some(herdr_pet::streaks::streak(&days, &today))
}

/// Caminho do CLI `herdr`: HERDR_BIN_PATH → `herdr` no PATH → `~/.local/bin/herdr`.
fn herdr_bin() -> String {
    if let Ok(b) = std::env::var("HERDR_BIN_PATH") {
        return b;
    }
    if std::process::Command::new("herdr")
        .arg("--version")
        .output()
        .is_ok()
    {
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
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| {
        format!(
            "pane current inesperado: {}",
            String::from_utf8_lossy(&out.stdout)
        )
    })?;
    v["result"]["pane"]["pane_id"]
        .as_str()
        .map(String::from)
        .ok_or("não veio o pane_id atual".to_string())
}

/// Faz um resize do pane `pet` e devolve a altura resultante do pet (lê do layout da resposta).
fn resize_pet_height(bin: &str, pet: &str, dir: &str, amount: f64) -> Option<u64> {
    let r = std::process::Command::new(bin)
        .args([
            "pane",
            "resize",
            "--pane",
            pet,
            "--direction",
            dir,
            "--amount",
            &amount.to_string(),
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

/// Acha os panes do pet (label "Pet") em QUALQUER workspace — devolve
/// `(pane_id, workspace_id)` de cada um que existir.
fn pet_panes() -> Result<Vec<(String, Option<String>)>, String> {
    let bin = herdr_bin();
    let out = std::process::Command::new(&bin)
        .args(["pane", "list"])
        .output()
        .map_err(|e| format!("herdr pane list: {e}"))?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap_or_default();
    Ok(v["result"]["panes"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|p| p.get("label").and_then(|l| l.as_str()) == Some("Pet"))
                .filter_map(|p| {
                    let pane_id = p["pane_id"].as_str()?.to_string();
                    let ws = p
                        .get("workspace_id")
                        .and_then(|w| w.as_str())
                        .map(String::from);
                    Some((pane_id, ws))
                })
                .collect()
        })
        .unwrap_or_default())
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

/// O que fazer com o seq no refresh: creditar catch-up (abertura) ou só avançar
/// a baseline sem XP (poll ao vivo — o accrual cuida do acompanhado).
enum SeqMode {
    Catchup,
    Track,
}

/// Um poll: `herdr agent list` → snapshot → aplica o seq no state. Catch-up e
/// poll compartilham este caminho; só o último passo (creditar vs. baseline) muda.
fn refresh_agents(state: &mut herdr_pet::state::State, mode: SeqMode) -> herdr_pet::AgentsSnapshot {
    let snap = herdr_pet::agent::snapshot(&herdr_pet::agent::all_agents_info());
    match mode {
        SeqMode::Catchup => {
            state.apply_catchup(&snap.pane_seqs);
        }
        SeqMode::Track => {
            state.record_seen_seq(&snap.pane_seqs);
        }
    }
    snap
}

/// Pede ao `watch` pra desenhar o resumo (Ctrl+C no PTY) e só destrói o pane
/// depois que a linha aparece — senão o toggle mata o PTY antes do print.
fn close_pet_with_farewell(bin: &str, pane: &str) {
    let sent = std::process::Command::new(bin)
        .args(["pane", "send-keys", pane, "ctrl+c"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if sent {
        // Gramática do herdr 0.8.0: `pane wait-output [OPTIONS] <PANE_ID>` SÓ
        // aceita o pane ANTES das flags — com o pane no fim o parse devolve
        // `unknown option: <needle>` (exit 2) e o wait nunca esperava nada
        // (o `let _` engolia; o farewell sobrevivia só pelo hold abaixo).
        let _ = std::process::Command::new(bin)
            .args([
                "pane",
                "wait-output",
                pane,
                "--match",
                herdr_pet::session::SUMMARY_NEEDLE,
                "--source",
                "visible",
                "--timeout",
                "2000",
            ])
            .output();
        // Um pouco menos que o hold do watch: fecha ainda com o quadro na tela.
        let hold = herdr_pet::session::FAREWELL_MS.saturating_sub(200);
        std::thread::sleep(std::time::Duration::from_millis(hold));
    }

    let _ = std::process::Command::new(bin)
        .args(["plugin", "pane", "close", pane])
        .output();
}

/// Toast no Herdr (sobrevive ao fechar o pane via toggle). Falha em silêncio se
/// o server não estiver no ar — a linha no terminal já saiu.
fn notify_session(line: &str) {
    let bin = herdr_bin();
    let _ = std::process::Command::new(&bin)
        .args(["notification", "show", line, "--sound", "done"])
        .output();
}

/// **Toggle/move** do pet (pro hotkey). Regra: NUNCA dois panes Pet ao mesmo
/// tempo — dois `watch` carregam cópias próprias do state e salvam o arquivo
/// inteiro por cima um do outro (last-writer-wins perde XP e regredi as
/// baselines de seq, que o próximo catch-up repaga). Então:
/// - pet aberto NO workspace focado → fecha (toggle de sempre);
/// - pet aberto em OUTRO workspace → "move": fecha o de lá (com farewell) e
///   reabre aqui embaixo do pane focado — o pet acompanha o usuário pra onde
///   ele foi trabalhar. Pré-condições do open (pane alvo) são resolvidas
///   ANTES de fechar: se não há pra onde reabrir, o pet fica onde está —
///   nunca fica sem pet nenhum por falha de API.
///
/// O segundo watch que escapa deste toggle (`herdr plugin pane open` chamado
/// DIRETO) não corrói mais nada: o lock do state faz o segundo watch entrar em
/// modo espelho — desenha, não salva (ver a tomada do lock no braço do `watch`).
/// Leve — o `watch` só roda enquanto aberto.
fn open_pet_small() -> Result<(), String> {
    let bin = herdr_bin();

    // Prefer API focus (hotkey/action context) over HERDR_WORKSPACE_ID — the env
    // can lag when `herdr-pet open` is called from another pane/workspace.
    let here = focused_workspace_id().or_else(|| std::env::var("HERDR_WORKSPACE_ID").ok());
    let pets = pet_panes()?;

    if !pets.is_empty() {
        // Pet no workspace atual (ou workspace indeterminável de qualquer lado —
        // sem como afirmar que é "outro") → comporta como toggle: fecha e pronto.
        // Só decide "move" quando ambos os lados são conhecidos e diferentes.
        let in_this_ws = pets.iter().any(|(_, ws)| match (&here, ws) {
            (Some(h), Some(w)) => h == w,
            _ => true,
        });
        if in_this_ws {
            for (existing, _) in &pets {
                close_pet_with_farewell(&bin, existing);
            }
            let ids: Vec<&str> = pets.iter().map(|(id, _)| id.as_str()).collect();
            outln!("✓ pet fechado ({}).", ids.join(", "));
            return Ok(());
        }
    }

    // pane alvo = o focado (via API — mais robusto que a env var HERDR_PANE_ID).
    // No braço move isso roda ANTES de fechar o pet existente: sem alvo pra
    // reabrir, o pet fica onde está (erro impresso) em vez de sumir dos dois.
    let target = match focused_pane() {
        Ok(t) => t,
        Err(e) if !pets.is_empty() => {
            outln!(
                "! não movi o pet: sem pane pra reabrir aqui ({e}) — ele segue aberto onde estava."
            );
            return Ok(());
        }
        Err(e) => return Err(e),
    };

    if !pets.is_empty() {
        // Move: agora que o alvo está garantido, fecha o pet do outro workspace
        // (com farewell) e reabre aqui embaixo.
        let from: Vec<String> = pets
            .iter()
            .map(|(id, ws)| match ws {
                Some(w) => format!("{id} ({w})"),
                None => id.clone(),
            })
            .collect();
        outln!(
            "✓ pet movido pra cá (fechando {} em outro workspace); reabrindo…",
            from.join(", ")
        );
        for (existing, _) in &pets {
            close_pet_with_farewell(&bin, existing);
        }
    }

    open_pet_split(&bin, &target)
}

/// Passos pós-decisão do `open`: cria o pane do pet dockado embaixo do
/// `target`, ajeita a altura pra banda [16,18] e refoca o pane original.
fn open_pet_split(bin: &str, target: &str) -> Result<(), String> {
    const PLUGIN_ID: &str = "allmight-ai.herdr-pet";

    // 1) abre o pet dockado abaixo do pane atual (split)
    let out = std::process::Command::new(bin)
        .args([
            "plugin",
            "pane",
            "open",
            "--plugin",
            PLUGIN_ID,
            "--entrypoint",
            "lcd",
            "--placement",
            "split",
            "--target-pane",
            target,
            "--direction",
            "down",
        ])
        .output()
        .map_err(|e| format!("não consegui rodar `herdr`: {e}"))?;
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(|_| {
        format!(
            "resposta inesperada: {}",
            String::from_utf8_lossy(&out.stdout)
        )
    })?;
    let pet = v["result"]["plugin_pane"]["pane"]["pane_id"]
        .as_str()
        .ok_or("não veio o pane_id do pet")?
        .to_string();

    // 2) ajeita o tamanho na banda [16,18]: encolhe se >18, cresce se <16.
    //    Abaixo de 16 o topo (nome) rola fora da viewport pequena.
    for _ in 0..20 {
        match resize_pet_height(bin, &pet, "down", 0.02) {
            Some(h) if h > 18 => {}
            _ => break,
        }
    }
    for _ in 0..24 {
        match resize_pet_height(bin, &pet, "up", 0.04) {
            Some(h) if h < 16 => {}
            _ => break,
        }
    }

    // 3) refoca o pane original (vizinho de cima do pet)
    let _ = std::process::Command::new(bin)
        .args(["pane", "focus", "--pane", &pet, "--direction", "up"])
        .output();

    outln!("✓ pet aberto ({pet}) — dockado embaixo. Ctrl+C fecha · redimensionar: prefix+r");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(day: &str, xp: u64, secs: u64, sessions: usize) -> herdr_pet::streaks::Day {
        herdr_pet::streaks::Day {
            day: day.to_string(),
            xp,
            secs_working: secs,
            sessions,
        }
    }

    #[test]
    fn janela_pega_os_ultimos_n_dias_contando_hoje() {
        assert!(in_window(0, 7), "hoje sempre entra");
        assert!(in_window(1, 7), "ontem entra");
        assert!(in_window(6, 7), "sexto dia atrás ainda é a janela");
        assert!(!in_window(7, 7), "sétimo dia atrás ficou fora");
        assert!(in_window(0, 1), "janela de 1 é só hoje");
        assert!(!in_window(1, 1));
    }

    #[test]
    fn janela_nao_esconde_data_no_futuro() {
        // Fuso/relógio podem gravar "amanhã": mostrar é honesto, ocultar mente.
        assert!(in_window(-1, 7));
    }

    #[test]
    fn linha_do_dia_formata_xp_tempo_e_sessoes() {
        let d = day("2026-08-19", 1_240, 47 * 60, 2);
        assert_eq!(
            format_day_row(&d),
            "2026-08-19 · +1.240 XP · 47 min · 2 sessões"
        );
    }

    #[test]
    fn linha_do_dia_omite_tempo_zero_e_singulariza_sessao() {
        // Catch-up puro (0 s acompanhados) não mostra "0 s".
        let d = day("2026-08-01", 300, 0, 1);
        assert_eq!(format_day_row(&d), "2026-08-01 · +300 XP · 1 sessão");
    }

    #[test]
    fn frase_da_sequencia_singular_e_recorde_seco() {
        let st = |current: u32, best: u32| herdr_pet::streaks::Streak {
            current,
            best,
            last_day: Some("2026-08-19".to_string()),
        };
        assert_eq!(streak_phrase(&st(5, 12)), "Sequência: 5 dias (recorde 12)");
        assert_eq!(streak_phrase(&st(1, 1)), "Sequência: 1 dia (recorde 1)");
        assert_eq!(streak_phrase(&st(0, 12)), "Sequência: 0 dias (recorde 12)");
    }
}
