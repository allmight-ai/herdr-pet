# Changelog

Todas as mudanças notáveis deste projeto serão documentadas aqui.
O formato é baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/)
e o projeto adere ao [Versionamento Semântico](https://semver.org/lang/pt-BR/).

## [Unreleased]

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

[0.2.0]: https://github.com/allmight-ai/herdr-pet/releases/tag/v0.2.0
[0.1.0]: https://github.com/allmight-ai/herdr-pet/releases/tag/v0.1.0
