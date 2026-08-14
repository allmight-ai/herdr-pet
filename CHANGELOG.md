# Changelog

Todas as mudanças notáveis deste projeto serão documentadas aqui.
O formato é baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/)
e o projeto adere ao [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [0.4.0] - 2026-08-14

Release da caça de bugs orquestrada (PR #3, 27 commits): 4 caçadores de IA em
panes do Herdr + revisor adversarial, processo documentado em
`docs/caca-de-bugs-orquestrada.md`.

### Fixed
- **Anti-cheat (P0)**: `watch --mood` creditava ~1000 XP/h no state real sem
  nenhum agente; baseline de seq rebobinada pagava replay de catch-up (até
  6.000 XP/pane por reabertura — raiz: `serde(default)` confundia campo ausente
  com `0`/`""`); filho Grok casado por cwd contava uma vez por pai `working`.
- **Detecção**: encode do Claude não sanitizava `.`/`_` (projetos com ponto no
  path perdiam filhos); filhos GLM invisíveis (fallback disciplinado com janela
  de 120 s e roots separados); filho morto no meio da tool virava fantasma
  eterno (staleness por mtime, tools paralelas cobertas); rodapé `⚙ N`
  congelava os nomes da abertura.
- **Persistência**: save não atômico + corrupção tratada como ausência =
  reset silencioso de todo o XP (agora tmp+fsync+rename por pid, `.corrupt`
  preservado, state ilegível recusa recriação); `init` fora do pane criava
  state órfão no CWD (fork de âncora); dois `watch` disputavam o `state.json`
  (nunca dois panes Pet: toggle/move); baselines cresciam sem eviction.
- **Watch/CLI**: PTY morta panicava antes do save final (perdia até ~30 s de
  XP); `status | head` saía 101 (broken pipe); falha de save era 100% silenciosa
  (agora `⚠ save` no rodapé + retry a ~30 s); o `pane wait-output` do farewell
  **nunca funcionou** (argv na ordem que o herdr 0.8.0 rejeita) — o toggle
  fechava só pelo hold.
- **Progressão**: um tick idle de 1 seq taxava o worker em até 73 XP no
  catch-up (peso harmônico agora é por contribuinte, top-8, alinhado ao live);
  resumo da sessão dizia "0 agentes · +N XP" em catch-up puro (agora
  `recuperado: +N XP`) e superestimava agentes quando ids chegavam depois.

### Changed
- `watch --mood` virou modo dev **só-leitura**: nunca grava state nem ganha XP;
  sem state, exige `--id`.
- Resolução do state dir em 4 passos documentados; o dir XDG do plugin é o
  padrão de leitura **e** escrita fora do pane (`.herdr-pet-state/` do CWD só
  se já existir).
- Needle do farewell mais específico (`🐾 Sessão:`) — título de tarefa com a
  palavra "Sessão" não atalha mais o fechamento.

## [0.3.1] - 2026-08-14

### Fixed
- `⚙ N` inflava com subagente Claude já morto: o jsonl às vezes fecha em texto
  sem `end_turn`, e o filho era contado mesmo com o pai `done`/`idle`. Agora só
  entra filho de pai `working`, e texto final sem stop_reason conta como
  terminado.
- Rodapé cego: `⚙ N` agora lista quem está working (`⚙ 2 grok, claude`) e
  mostra `⚙ 0` quando ninguém trabalha — o humor e a conta saem do mesmo filtro
  (dormindo ⇒ zero).

### Changed
- **Subagentes de qualquer pai**: além do Claude/GLM, o pet lê os filhos do Grok
  (`active_sessions.json` + `sessions/.../subagents/`). Quem ainda roda entra no
  `⚙ N`, na rotação e no XP — o Herdr continua vendo só o processo pai.
- **`herdr-pet status`** mostra nível, barra de XP, total e quem está working agora
  (incluindo subagentes). Fora do pane do plugin, lê o state do Herdr
  (`~/.local/state/herdr/plugins/…`) — não o `.herdr-pet-state/` de dev.

## [0.3.0] - 2026-08-13

### Added
- **Subagentes do Claude Code** (e GLM): time/Task ainda rodando entra no `⚙ N`,
  na rotação de tarefas e no XP. O Herdr só lista o processo pai; cada subagente
  ativo conta como outro.
- **Resumo da sessão** ao fechar o pet (Ctrl+C ou toggle): agentes que trabalharam,
  XP ganho (incluindo catch-up da abertura), nível e duração. Ex.:
  `🐾 Sessão: 2 agentes · +1.240 XP · Nv 6 → 7 · 47 min`. A linha fica ~1,4 s na
  casinha (o toggle espera o quadro antes de destruir o pane) e também vai pra
  `herdr notification`.

## [0.2.0] - 2026-08-12

### Added
- **Progressão (XP e nível)** forjada pelo trabalho real do agente: curva `100 × nível`
  (acelerante), nível 1–99 derivado do XP total, ~1000 XP/h de trabalho acompanhado.
  `idle` não rende XP (anti-cheat: o sinal de trabalho vem do Herdr, não de arquivo local).
- **XP agrega todos os agentes** (qualquer projeto), não só o focado — com decaimento
  harmônico (1, ½, ⅓, …) que evita inflação por proliferação de agentes. Trabalho com o
  painel fechado é contabilizado na reabertura pelo `state_change_seq`.
- **Display agrega todos os agentes**: o pet acorda (`« treinando »`) se **qualquer**
  agente está `working`, mostrando a tarefa de quem trabalha — mesmo dockado numa sessão
  idle. Antes espelhava só o agente focado.
- **Rotação de tarefas**: com vários agentes `working` ao mesmo tempo, o pet alterna
  entre as tarefas deles (~4 s cada).
- **Setup automático** no install: grava o atalho no `config.toml` do Herdr + shim no
  `PATH` (`herdr-pet setup`, idempotente; roda no `[[build]]` e `[[startup]]`).
- **Git hooks** (`githooks/post-commit` + `post-merge`, via `core.hooksPath`) que
  recompilam o binário após cada commit/pull — o pet nunca roda versão desatualizada.
- **GitHub Action** de release: publica a Release automaticamente no push de uma tag `v*`,
  com notas extraídas deste CHANGELOG.

### Changed
- Rodapé da casinha: `⚙ N` indica N agentes trabalhando (antes o críptico `Nw`).
- README (PT/EN), `docs/design.md` e ADR 0002 refletem o display agregado e a progressão.

## [0.1.0] - 2026-08-04

### Added
- Companion V-Pet reativo: casinha LCD 1-bit que espelha o status do agente de código
  (working/idle/blocked/done) do Herdr.
- Forja determinística por GitHub anchor — espécie, raridade, shiny e nome (cosmético).
- Toggle on-demand via hotkey (`herdr-pet open` → split pequeno dockado).
- Render ANSI otimizado: pet cacheado por `(github_id, índice)` e redraw só quando algo
  visível muda.

[0.3.1]: https://github.com/allmight-ai/herdr-pet/releases/tag/v0.3.1
[0.3.0]: https://github.com/allmight-ai/herdr-pet/releases/tag/v0.3.0
[0.2.0]: https://github.com/allmight-ai/herdr-pet/releases/tag/v0.2.0
[0.1.0]: https://github.com/allmight-ai/herdr-pet/releases/tag/v0.1.0
