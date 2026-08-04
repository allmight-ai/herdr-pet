# herdr-pet

Companion (plugin do **Herdr**) em forma de V-Pet — uma **coleção forjada por mérito**.

> **Status: DESIGN — não implementado.** Este repositório existe para guardar o design e as
> decisões enquanto amadurecemos. Veja
> [`docs/ESBOCO-herdr-raridade.md`](docs/ESBOCO-herdr-raridade.md).

## A ideia em uma frase

Um V-Pet que vive numa pane do Herdr, cuja raridade é **forjada, não sorteada** — derivada
deterministicamente do **ID numérico da sua conta GitHub**. Você não nasce com todos os pets: cada
nova eclosão **custa aura** (mérito verificável via `gh api`), e o caminho de eclosões é **público e
imutável**. Anti-reroll **sem servidor**.

## Filosofia

**Visão B — "sua coleção é a sua carreira":** jogo de coleção/farm estilo Pokémon (buscar o melhor,
IV), mas sem reroll. Em vez de "1 pet = sua identidade" (Visão A), sua **coleção reflete o seu
trabalho** — e o mesmo esforço que alimenta o pet forja a aura que compra novas eclosões.

## Referência / base

A **teoria e o design visual** vêm do projeto **petterm** (`~/projects/petterm`) — V-Pet 1-bit LCD
estilo Tamagotchi, onde a cor do LCD é a raridade. O petterm é **Python**; este companion é
**reimplementado em Rust** (sem import de código Python — só referência de design).

## Stack

- **Rust** — lógica de forja (HMAC-SHA256, sub-seeds, derivação por índice) + renderer LCD.
- **Plugin nativo do Herdr** — manifest `herdr-plugin.toml` (`[[panes]]` / `[[actions]]` /
  `[[link_handlers]]`) apontando para o binário Rust. O Herdr executa o binário e passa o contexto
  por env vars (`HERDR_PLUGIN_STATE_DIR`, `HERDR_BIN_PATH`, …).

## Decisões de design fechadas (2026-08-04)

- **Implementação: Rust**, plugin nativo do Herdr (manifest TOML + binário).
- **Âncora** = ID numérico do GitHub (`gh api user --jq .id`) — imutável, fora da máquina.
- **DNA por sub-seed** (`HMAC(seed, feature_name)`, estilo BIP-32) — reserva ilimitada de genes.
- **`genesis_version`** + migration — o algoritmo é mutável; os nascimentos são imutáveis.
- **Lock-in na 1ª conta** + genesis gist público (commit-reveal) — transparência anti-reroll.
- **Renascimento = eclosão por índice** (`gene(seed, "pet:N")`) — unifica R2 (evolução) e coleção
  futura (farm) num só mecanismo.
- **Espécies = pixel-mons inventados** (originais, formas fáceis em 1-bit LCD).
- **Curva de raridade mantida: 60/25/10/4/1** (common → legendary), shiny 1/128.
- **Aura = moeda da eclosão** (peso central, não só cosmética).

Detalhes e raciocínio completo: [`docs/ESBOCO-herdr-raridade.md`](docs/ESBOCO-herdr-raridade.md).
