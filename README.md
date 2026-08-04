# herdr-pet

Companion (plugin do **Herdr**) — um V-Pet que **espelha o seu agente de código**: reage ao
status dele (trabalhando/idle/blocked/done) e mostra a tarefa atual, do lado, numa casinha
LCD 1-bit. Leve, sob demanda, em qualquer workspace.

> **Status: v1 implementado** (companion reativo). A ambição maior — coleção forjada por mérito
> (Visão B: gacha/breeding/aura/IVs) — é **futuro**, ver
> [`docs/ESBOCO-herdr-raridade.md`](docs/ESBOCO-herdr-raridade.md).
> Spec do v1: [`docs/SPEC-companion-basico.md`](docs/SPEC-companion-basico.md).

## O que ele faz (v1)
- **Espelha o agente**: lê o `agent_status` do Herdr (`working`/`idle`/`blocked`/`done`/`unknown`)
  e reage com animação + mood (treinando, dormindo, curioso, comemorando).
- **Mostra a tarefa atual** do agente (`» <título do terminal>`).
- **Identidade forjada**: espécie + raridade (cor) + shiny derivados deterministicamente do seu
  **ID numérico do GitHub** — **inroubáveis / anti-reroll** (re-derivável; a raridade nunca vive
  só no disco).
- **Leve**: pet cacheado, redraw só quando algo muda, e sob demanda (só roda enquanto o pane tá
  aberto).
- **Em qualquer workspace**: hotkey global abre/fecha (toggle).

## Como usar
```bash
cargo build --release
```
No Herdr, **`prefix+shift+p`** abre/fecha o pet (split pequeno dockado embaixo). Ou direto:
```bash
./target/release/herdr-pet watch               # ao vivo — espelha o agente
./target/release/herdr-pet watch --mood done   # pré-visualiza um mood (dev)
./target/release/herdr-pet open                # toggle do pane (o que o hotkey chama)
./target/release/herdr-pet gallery             # um pet de cada tier (+ shiny + Primordial)
```
O hotkey vive no `~/.config/herdr/config.toml` (`[[keys.command]]` → `herdr-pet open`).

## Stack
- **Rust** — forja (HMAC-SHA256 por âncora GitHub) + renderer LCD (stdout/ANSI, block chars).
- **Plugin nativo do Herdr** — manifest `herdr-plugin.toml`; lê o agente via socket API
  (`herdr agent list`); pane via `herdr plugin pane`.

## Origem
Teoria visual e design vêm do **petterm** (`~/projects/petterm`) — V-Pet 1-bit LCD estilo
Tamagotchi, onde a cor do LCD é a raridade. Reimplementado em Rust (referência de design, sem
importar código Python).

## Fora do v1 (futuro)
Progressão (XP/nível/rebirth estilo Ragnarok), combate, **coleção/gacha/breeding/aura via
`gh api`** — a Visão B original. Ainda não implementado; ver o
[`ESBOÇO`](docs/ESBOCO-herdr-raridade.md).

## Decisões (2026-08-04)
- **Implementação: Rust**, plugin nativo do Herdr.
- **Âncora** = ID numérico do GitHub — imutável, fora da máquina (anti-reroll da identidade).
- **Forja determinística** por sub-seeds (estilo BIP-32); `genesis_version` + migration.
- **v1 = companion reativo simples** (1 pet, status+task do agente) — não a Visão B de coleção.
