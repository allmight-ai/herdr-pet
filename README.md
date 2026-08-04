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

## Reutiliza

A teoria e parte do código do projeto **petterm** (`~/projects/petterm`) — V-Pet 1-bit LCD estilo
Tamagotchi, onde a cor do LCD é a raridade.

## Decisões de design fechadas (2026-08-04)

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
