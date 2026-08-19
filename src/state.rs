//! State persistente do companion.
//!
//! Em `HERDR_PLUGIN_STATE_DIR` (pane do Herdr) ou no dir XDG do plugin
//! (`~/.local/state/herdr/plugins/allmight-ai.herdr-pet`) — padrão de leitura E
//! escrita fora do pane. `.herdr-pet-state/` no CWD só por compat com dev
//! antigo (se já existir); nunca é criado implicitamente. Guarda a âncora
//! (lock-in no primeiro GitHub ID), o índice ativo e os índices já chocados.
//! A raridade não vive só no disco — é rederivável da âncora.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::progression::{harmonic_weighted_xp, level_for_xp, xp_for_catchup};

/// Sinal de trabalho de um agente: o `state_change_seq` observado num dado pane.
/// Conceito do `CONTEXT.md` ("Sinal de trabalho"); é a chave do mapa `last_seq_by_pane`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneSeq {
    pub pane_id: String,
    pub seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub anchor: String,
    pub github_id: u64,
    pub active_index: u32,
    pub hatched: Vec<u32>,
    /// XP total do pet ativo. Ausente em states antigos → default 0.
    #[serde(default)]
    pub xp: u64,
    /// Último `state_change_seq` visto por pane (catch-up de trabalho não acompanhado,
    /// em qualquer projeto). Mapa pane_id → seq; ausente em states antigos → vazio.
    #[serde(default)]
    pub last_seq_by_pane: HashMap<String, u64>,
}

impl State {
    /// State inicial para um GitHub ID (pet #0 já nasceu).
    pub fn new(github_id: u64) -> Self {
        State {
            anchor: crate::forge::anchor_for(github_id),
            github_id,
            active_index: 0,
            hatched: vec![0],
            xp: 0,
            last_seq_by_pane: HashMap::new(),
        }
    }

    /// Marca um índice como chocada e ativa (idempotente).
    pub fn record_hatch(&mut self, index: u32) {
        if !self.hatched.contains(&index) {
            self.hatched.push(index);
        }
        self.active_index = index;
    }

    /// Nível atual do pet ativo (1..=99), derivado do XP total.
    pub fn level(&self) -> u8 {
        level_for_xp(self.xp)
    }

    /// Contabiliza trabalho de **todos** os agentes observados enquanto o pane esteve
    /// fechado. `agents`: `(pane_id, seq observado)` de cada agente. Primeira vista de
    /// um pane = baseline (sem creditar); nas seguintes, XP pelo delta. Vários agentes
    /// avançando sofrem o decaimento harmônico (anti-proliferação). Devolve o XP ganho.
    ///
    /// Também **expurga** do `last_seq_by_pane` as chaves ausentes da observação
    /// corrente (C12): pane morto não deixa slot eterno. (Hoje as entradas são
    /// só panes reais listados pelo Herdr — subagentes sintéticos vêm SEM
    /// `state_change_seq` desde a fase P0 e nunca entraram neste mapa; a
    /// eviction também poda sobras deixadas por versões antigas.) Observação
    /// vazia NÃO evicta — ver guarda no corpo. Só aqui; nota em `record_seen_seq`.
    pub fn apply_catchup(&mut self, agents: &[PaneSeq]) -> u64 {
        // Dedupe por pane (maior seq neste tick). Snapshot já descarta pane/seq
        // omitidos; se o mesmo pane vier 2×, o maior valor é o observado.
        let mut latest: HashMap<&str, u64> = HashMap::new();
        for ps in agents {
            latest
                .entry(&ps.pane_id)
                .and_modify(|e| *e = (*e).max(ps.seq))
                .or_insert(ps.seq);
        }
        // Ganhos contra o mapa como estava na entrada: um pane não enxerga o insert de outro.
        let mut gains = Vec::new();
        for (pane_id, observed) in &latest {
            let g = match self.last_seq_by_pane.get(*pane_id) {
                Some(&last) => xp_for_catchup(observed.saturating_sub(last)),
                None => 0, // primeira vista do pane: baseline, sem creditar histórico
            };
            if g > 0 {
                gains.push(g);
            }
        }
        for (pane_id, observed) in &latest {
            self.remember_seq(pane_id, *observed);
        }
        // Eviction (C12): o catch-up roda UMA vez na abertura e vê o conjunto
        // atual de panes — quem não apareceu (pane fechado/morto, sobra de
        // versão antiga) sai do mapa; cada sessão de watch poda pro universo
        // corrente. GUARDA: observação VAZIA não evicta nada — `herdr agent
        // list` falhar na abertura (server recarregando, binário fora do PATH —
        // justamente quando o pane sobe) chega aqui como `latest == {}`, e um
        // retain sem guarda apagaria o mapa INTEIRO, que o gate de save então
        // persistiria. Pane VIVO momentaneamente ausente perde a baseline e
        // volta como primeira-vista (sem crédito histórico) — no máximo
        // subconta, nunca infla.
        if !latest.is_empty() {
            self.last_seq_by_pane
                .retain(|k, _| latest.contains_key(k.as_str()));
        }
        // Largura: 1, ½, ⅓… no ganho (maior primeiro). Não H(n)/n sobre a soma —
        // um tick de 3 XP não pode comer 73 do worker (C7). Ver harmonic_weighted_xp.
        let granted = harmonic_weighted_xp(&mut gains);
        self.xp += granted;
        granted
    }

    /// Avança a baseline de cada agente **sem creditar XP** — usado no poll enquanto o
    /// pane está aberto, pra o próximo catch-up contar só o período fechado (sem dupla
    /// contagem). Dedupe pelo maior seq do slice. Se o observado for menor que o last
    /// (reset genuíno do Herdr), a baseline desce; o 0 espúrio da API omitida não
    /// chega — `agent::snapshot` descarta PaneSeq incompleto.
    ///
    /// NÃO evicta chaves ausentes (de propósito): o poll roda a cada ~2,4s e um
    /// pane vivo pode piscar fora do snapshot (agente entre transições) — apagar
    /// baseline por flicker trocaria a próxima observação por primeira-vista à
    /// toa. A eviction acontece uma vez por sessão, no `apply_catchup` da abertura.
    pub fn record_seen_seq(&mut self, agents: &[PaneSeq]) {
        let mut latest: HashMap<&str, u64> = HashMap::new();
        for ps in agents {
            latest
                .entry(&ps.pane_id)
                .and_modify(|e| *e = (*e).max(ps.seq))
                .or_insert(ps.seq);
        }
        for (pane_id, observed) in latest {
            self.remember_seq(pane_id, observed);
        }
    }

    /// Grava o seq observado. `observed < last` é reset genuíno (Herdr reiniciou
    /// ou o pane_id foi reusado): rebobina sem creditar neste tick — o ganho já
    /// saiu 0 via `saturating_sub`. Deltas seguintes partem do novo zero.
    ///
    /// Replay 200→0→200 de campo omitido continua morto: o 0 espúrio não passa
    /// do snapshot (`Option`, descarta ausente). Um 0 que chega aqui é real.
    fn remember_seq(&mut self, pane_id: &str, observed: u64) {
        self.last_seq_by_pane.insert(pane_id.to_string(), observed);
    }
}

/// Diretório do state, nesta ordem (núcleo puro em `resolve_state_dir`):
/// 1. `HERDR_PLUGIN_STATE_DIR` (pane do plugin — verdade do Herdr)
/// 2. dir XDG do plugin **com** `state.json` já presente
/// 3. `.herdr-pet-state/state.json` no CWD (compat com dev antigo — nunca criado)
/// 4. dir XDG do plugin — padrão pra LER E ESCREVER fora do pane (o save cria)
///
/// A regra nova do passo 4 fecha o C10: `herdr-pet init` num shell qualquer grava
/// no dir do plugin (o mesmo do pane), não num `.herdr-pet-state/` órfão do CWD —
/// um state só, âncora única. O passo 3 mantém o dev que JÁ tinha state local
/// funcionando; o passo 2 preserva a precedência antiga (install ganha de dev).
///
/// Limitação conhecida (aceita): fora do pane o caminho é **reconstruído** do env
/// do próprio processo (`XDG_STATE_HOME`). Se o shell e o server Herdr rodarem
/// com envs divergentes, `status`/`init` olham outro lugar que o pane — não há
/// pointer file; o caso comum (envs iguais) é coberto pelos passos acima.
pub fn state_dir() -> PathBuf {
    resolve_state_dir(
        std::env::var("HERDR_PLUGIN_STATE_DIR").ok().as_deref(),
        herdr_plugin_state_dir(),
        cwd_dev_state().as_deref(),
    )
}

/// `.herdr-pet-state/state.json` no CWD, se existir (compat dev antigo).
fn cwd_dev_state() -> Option<PathBuf> {
    let p = PathBuf::from(".herdr-pet-state/state.json");
    p.is_file().then_some(p)
}

/// Núcleo de `state_dir` (puro, testável sem mexer em env global). `cwd_dev_state`
/// sendo `Some` significa que o state de dev existe no CWD.
fn resolve_state_dir(
    plugin_env: Option<&str>,
    xdg_dir: Option<PathBuf>,
    cwd_dev_state: Option<&Path>,
) -> PathBuf {
    if let Some(d) = plugin_env {
        return PathBuf::from(d);
    }
    if let Some(x) = &xdg_dir {
        if x.join("state.json").is_file() {
            return x.clone();
        }
    }
    if cwd_dev_state.is_some() {
        return PathBuf::from(".herdr-pet-state");
    }
    xdg_dir.unwrap_or_else(|| PathBuf::from(".herdr-pet-state"))
}

/// `XDG_STATE_HOME/herdr/plugins/allmight-ai.herdr-pet` (padrão: `~/.local/state/...`).
pub fn herdr_plugin_state_dir() -> Option<PathBuf> {
    let base = match std::env::var("XDG_STATE_HOME") {
        Ok(d) => PathBuf::from(d),
        Err(_) => PathBuf::from(std::env::var("HOME").ok()?).join(".local/state"),
    };
    Some(base.join("herdr/plugins/allmight-ai.herdr-pet"))
}

pub fn state_path() -> PathBuf {
    state_dir().join("state.json")
}

/// Resultado de um load: distingue POR QUE não carregou, porque as consequências
/// mudam (C9 rodada 2).
#[derive(Debug)]
pub enum LoadOutcome {
    /// State carregado.
    Loaded(State),
    /// Arquivo ausente: o auto-init pode criar.
    Missing,
    /// Arquivo lido mas não parseia — o conteúdo JÁ foi preservado em
    /// `<path>.corrupt` antes de chegar aqui; recriar por cima é seguro.
    Corrupt,
    /// Arquivo presente cujos bytes NÃO puderam ser lidos (ex.: modo 000) — não
    /// há como preservar o que não se lê. Os dados presumivelmente estão intactos:
    /// **recriar por cima os destruiria** (`rename` funciona mesmo com o alvo
    /// ilegível). O chamador não deve salvar neste caminho.
    Unreadable,
}

/// Carrega de um caminho explícito distinguindo o motivo da falha (ver
/// `LoadOutcome`). Emite os avisos no stderr.
pub fn load_from_outcome(path: &Path) -> LoadOutcome {
    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return LoadOutcome::Missing,
        Err(e) => {
            // Presente mas ilegível SEM ler os bytes: nada a preservar, nada a
            // recriar por cima — avisa e sinaliza pro chamador não salvar.
            eprintln!(
                "herdr-pet: state {} não pôde ser lido ({e}) — NADA foi alterado; \
                 corrija o acesso ao arquivo antes de rodar de novo",
                path.display()
            );
            return LoadOutcome::Unreadable;
        }
    };
    match serde_json::from_slice(&data) {
        Ok(s) => LoadOutcome::Loaded(s),
        Err(e) => {
            let dest = preserve_corrupt(path, &data);
            eprintln!("{}", corrupt_warning(path, &dest, &e));
            LoadOutcome::Corrupt
        }
    }
}

/// Carrega de um caminho explícito (testável, sem depender de env var global).
///
/// Compat (`Option`): `Loaded` → `Some`; qualquer outro caso → `None`. Quem
/// precisa decidir entre recriar ou não usa `load_from_outcome`.
pub fn load_from(path: &Path) -> Option<State> {
    match load_from_outcome(path) {
        LoadOutcome::Loaded(s) => Some(s),
        _ => None,
    }
}

/// Copia o conteúdo ilegível pra `<path>.corrupt` (`.corrupt.1`, `.2`, … se já
/// houver — preservas **diferentes** empilham sem sobrescrever). Cópia idêntica
/// a uma preserva existente NÃO empilha de novo (rodar `status` três vezes
/// não gera três arquivos iguais). Devolve o destino usado.
fn preserve_corrupt(path: &Path, data: &[u8]) -> PathBuf {
    let mut n = 0;
    loop {
        let dest = corrupt_sibling(
            path,
            &if n == 0 {
                String::new()
            } else {
                format!(".{n}")
            },
        );
        match fs::read(&dest) {
            Ok(existing) if existing == data => return dest, // já preservado idêntico
            Ok(_) => {
                n += 1; // preserva diferente ocupa este slot — tenta o próximo
            }
            Err(_) => {
                // slot livre (ou incomparável): grava aqui
                let _ = fs::write(&dest, data);
                return dest;
            }
        }
    }
}

fn corrupt_sibling(path: &Path, suffix: &str) -> PathBuf {
    PathBuf::from(format!("{}.corrupt{suffix}", path.display()))
}

/// Mensagem de aviso (fatorada pra ser testável — o `eprintln` em si não é).
fn corrupt_warning(path: &Path, dest: &Path, err: &serde_json::Error) -> String {
    format!(
        "herdr-pet: state {} ilegível ({}); conteúdo preservado em {} — o state será recriado",
        path.display(),
        err,
        dest.display()
    )
}

/// Salva num caminho explícito (testável). Atômico (C9): tmp + fsync + rename —
/// crash no meio da gravação nunca deixa o `state.json` truncado; ou o arquivo
/// antigo permanece, ou o novo aparece inteiro.
pub fn save_to(path: &Path, state: &State) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(state).expect("state serializa");
    write_atomic(path, &data)
}

/// Gravação atômica: escreve num tmp no mesmo diretório, dá fsync e renomeia.
/// Morava em `setup.rs`; movida pra cá pra o state usar o mesmo mecanismo.
/// O tmp leva o PID no nome: dois processos `watch` salvando ao mesmo tempo
/// escrevem tmps DIFERENTES — tmp de nome fixo entregaria bytes intercalados
/// no rename (JSON inválido, pior que o `fs::write` antigo).
pub fn write_atomic(path: &Path, data: &[u8]) -> std::io::Result<()> {
    let tmp = tmp_sibling(path);
    if let Some(dir) = path.parent() {
        sweep_orphan_tmps(dir, &tmp);
    }
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)
}

/// Prazo a partir do qual um tmp vira lixo mesmo sem prova de que o dono morreu.
/// Uma gravação em voo vive milissegundos; dez minutos só sobram de um crash.
const TMP_ORPHAN_GRACE: Duration = Duration::from_secs(600);

/// Remove tmps órfãos de processos que morreram entre create e rename (o crash
/// deixava `*.tmp-herdr-pet-<pid>` no diretório pra sempre). Best-effort: erros
/// ignorados. Varredura acontece só ao salvar — não há varredura "de passagem"
/// em comandos só-leitura. Quem decide o que é lixo é `tmp_is_garbage`: tmp de
/// dono vivo NUNCA é varrido, senão a varredura de um processo faria o `rename`
/// do outro falhar (era o que acontecia — ver o teste de regressão).
fn sweep_orphan_tmps(dir: &Path, own_tmp: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // O marcador sem o sufixo de pid cobre também sobras do formato antigo.
        if !name.contains("tmp-herdr-pet") {
            continue;
        }
        let age = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| SystemTime::now().duration_since(t).ok());
        if tmp_is_garbage(
            p == own_tmp,
            age,
            tmp_owner_pid(name).and_then(crate::proc::pid_alive),
        ) {
            let _ = fs::remove_file(&p);
        }
    }
}

/// A regra da varredura, isolada pra ser testável (o `read_dir` em si não é).
/// O tmp deste processo nunca é lixo; um tmp que passou da carência é (cobre
/// formato antigo, dono impossível de checar e pid reciclado por outro programa);
/// e um tmp de dono comprovadamente morto é. Dono vivo + tmp recente = gravação
/// em voo de OUTRO processo — o pid no nome existe justamente pra dois `watch`
/// salvarem em paralelo, e apagar o tmp alheio desfazia essa garantia.
fn tmp_is_garbage(own: bool, age: Option<Duration>, owner_alive: Option<bool>) -> bool {
    if own {
        return false;
    }
    if age.is_some_and(|a| a >= TMP_ORPHAN_GRACE) {
        return true;
    }
    // Sem idade legível e sem veredito sobre o dono, poupa: deixar lixo custa um
    // arquivo, apagar tmp vivo custa um save.
    owner_alive == Some(false)
}

/// O pid declarado no fim do nome do tmp (`…tmp-herdr-pet-<pid>`). `None` no
/// formato antigo, que não carregava dono.
fn tmp_owner_pid(name: &str) -> Option<u32> {
    name.rsplit_once("tmp-herdr-pet-")?.1.parse().ok()
}

/// Nome do tmp do `write_atomic` pra este processo.
fn tmp_sibling(path: &Path) -> PathBuf {
    path.with_extension(format!("tmp-herdr-pet-{}", std::process::id()))
}

/// Carrega do state padrão.
pub fn load() -> Option<State> {
    load_from(&state_path())
}

/// Carrega do state padrão distinguindo o motivo da falha (ver `LoadOutcome`).
pub fn load_outcome() -> LoadOutcome {
    load_from_outcome(&state_path())
}

/// Salva no state padrão.
pub fn save(state: &State) -> std::io::Result<()> {
    save_to(&state_path(), state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tdir(tag: &str, with_state: bool) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("herdr-pet-resolve-{}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        if with_state {
            fs::write(d.join("state.json"), "{}").unwrap();
        }
        d
    }

    #[test]
    fn resolve_prefere_env_do_pane() {
        let d = resolve_state_dir(
            Some("/pane/plugin"),
            Some(tdir("a", true)),
            Some(Path::new(".herdr-pet-state/state.json")),
        );
        assert_eq!(d, PathBuf::from("/pane/plugin"));
    }

    #[test]
    fn resolve_xdg_com_state_vence_dev_do_cwd() {
        // Precedência antiga preservada: install (XDG com state) ganha do dev local.
        let xdg = tdir("b", true);
        let d = resolve_state_dir(
            None,
            Some(xdg.clone()),
            Some(Path::new(".herdr-pet-state/state.json")),
        );
        assert_eq!(d, xdg);
    }

    #[test]
    fn resolve_dev_antigo_do_cwd_quando_xdg_nao_tem_state() {
        let d = resolve_state_dir(
            None,
            Some(tdir("c", false)),
            Some(Path::new(".herdr-pet-state/state.json")),
        );
        assert_eq!(d, PathBuf::from(".herdr-pet-state"));
    }

    #[test]
    fn resolve_sem_nada_padrao_e_xdg_para_escrever() {
        // Regra nova (C10): sem env e sem state em lugar nenhum, o XDG é o padrão
        // de escrita — nunca um `.herdr-pet-state/` implícito no CWD arbitrário.
        let xdg = tdir("d", false);
        let d = resolve_state_dir(None, Some(xdg.clone()), None);
        assert_eq!(d, xdg);
    }

    #[test]
    fn tmp_do_write_atomic_leva_o_pid() {
        // Dois processos salvando ao mesmo tempo não podem compartilhar o tmp:
        // mesmo nome = bytes intercalados no rename.
        let t = tmp_sibling(Path::new("d/state.json"));
        let name = t.file_name().unwrap().to_str().unwrap();
        assert_eq!(name, format!("state.tmp-herdr-pet-{}", std::process::id()));
    }

    /// Envelhece o mtime de um arquivo — testa a carência sem dormir.
    fn age_file(p: &Path, secs: u64) {
        let f = fs::File::options().write(true).open(p).unwrap();
        let t = SystemTime::now() - Duration::from_secs(secs);
        f.set_times(fs::FileTimes::new().set_modified(t)).unwrap();
    }

    #[test]
    fn write_atomic_varre_tmps_orfaos_mas_poupa_os_vivos() {
        // Crash entre create e rename deixava tmp órfão pra sempre. O próximo
        // save remove o que passou da carência — inclusive sobras do formato
        // antigo (sem pid). Tmp recente de dono vivo e arquivos sem relação
        // alguma ficam intocados.
        let dir = std::env::temp_dir().join(format!("herdr-pet-sweep-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let orphan = dir.join(format!("state.tmp-herdr-pet-{}", u32::MAX)); // pid impossível
        let legacy = dir.join("state.tmp-herdr-pet"); // formato antigo
        let vivo = dir.join(format!("outro.tmp-herdr-pet-{}", std::process::id()));
        let keep = dir.join("state.json.corrupt");
        for f in [&orphan, &legacy, &vivo, &keep] {
            fs::write(f, b"...").unwrap();
        }
        age_file(&orphan, 3600);
        age_file(&legacy, 3600);

        let target = dir.join("state.json");
        let s = State::new(1);
        save_to(&target, &s).unwrap();

        assert!(!orphan.exists(), "tmp órfão vencido removido");
        assert!(!legacy.exists(), "sobra do formato antigo removida");
        assert!(vivo.exists(), "tmp recente de dono vivo é poupado");
        assert!(keep.exists(), "arquivo sem relação alguma é poupado");
        assert!(load_from(&target).is_some(), "o save em si funcionou");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn tmp_em_voo_de_outro_processo_nao_e_varrido() {
        // REGRESSÃO: a varredura apagava QUALQUER tmp que não fosse o seu. Dois
        // `watch` salvando ao mesmo tempo (o cenário que o pid no nome existe
        // pra cobrir) viravam `rename` NotFound — save perdido, XP junto.
        assert!(!tmp_is_garbage(
            false,
            Some(Duration::from_millis(5)),
            Some(true)
        ));
    }

    #[test]
    fn tmp_de_dono_morto_e_varrido_na_hora() {
        assert!(tmp_is_garbage(
            false,
            Some(Duration::from_millis(5)),
            Some(false)
        ));
    }

    #[test]
    fn tmp_vencido_e_varrido_mesmo_sem_veredito_do_dono() {
        // Formato antigo, dono impossível de checar ou pid reciclado: a carência é a
        // rede de segurança pra o lixo não ficar eterno.
        assert!(tmp_is_garbage(false, Some(TMP_ORPHAN_GRACE), None));
        assert!(!tmp_is_garbage(false, Some(Duration::from_secs(1)), None));
        // Sem idade e sem dono conhecido, poupa.
        assert!(!tmp_is_garbage(false, None, None));
    }

    #[test]
    fn tmp_do_proprio_processo_nunca_e_varrido() {
        assert!(!tmp_is_garbage(
            true,
            Some(Duration::from_secs(86_400)),
            Some(false)
        ));
    }

    #[test]
    fn dono_do_tmp_sai_do_nome() {
        assert_eq!(tmp_owner_pid("state.tmp-herdr-pet-42"), Some(42));
        assert_eq!(tmp_owner_pid("state.tmp-herdr-pet"), None, "formato antigo");
        assert_eq!(
            tmp_owner_pid("state.tmp-herdr-pet-abc"),
            None,
            "lixo no sufixo"
        );
    }
}
